use std::cell::RefCell;
use std::rc::Rc;

use leptos::prelude::*;
use leptos::{logging::log, *};
use leptos_use::{use_document, use_window};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram, WebGlShader,
};

async fn fetch_shader_content_by_id(id: &str) -> Option<String> {
    // using web_sys here; gloo_net is also fine (see alt below)
    let el = use_document().query_selector(id).ok().flatten()?;
    let script = el.dyn_into::<web_sys::HtmlScriptElement>().ok()?;
    let src = script.src();

    let resp =
        wasm_bindgen_futures::JsFuture::from(use_window().as_ref().unwrap().fetch_with_str(&src))
            .await
            .ok()?;
    let resp: web_sys::Response = resp.dyn_into().ok()?;
    let text_promise = resp.text().ok()?;
    let text_js = wasm_bindgen_futures::JsFuture::from(text_promise)
        .await
        .ok()?;
    text_js.as_string()
}

#[component]
pub fn App() -> impl IntoView {
    let shaders = LocalResource::new(|| async move {
        let fvs = fetch_shader_content_by_id("#field-vertex-shader").await;
        let ffs = fetch_shader_content_by_id("#field-fragment-shader").await;
        let lvs = fetch_shader_content_by_id("#line-vertex-shader").await;
        let lfs = fetch_shader_content_by_id("#line-fragment-shader").await;
        if let (Some(mvc), Some(mfc), Some(rvc), Some(rfc)) = (fvs, ffs, lvs, lfs) {
            return Some(((mvc, mfc), (rvc, rfc)));
        }
        None
    });
    view! {
        <Transition fallback=move || view!{ <p>Loading</p> }>
            {move || {
                match shaders.get() {
                    Some(Some(shader_code)) => {
                        view! {
                            <FieldCanvas field_shaders=shader_code.0 line_shaders=shader_code.1 />
                        }.into_any()
                    }
                    Some(None) => view!{ <p>Could not load required shader code</p> }.into_any(),
                    None => view!{ <p>Loading</p> }.into_any(),                }
            }}
        </Transition>
    }
}

// TODO: Replace all this fetching crap with inlining the shaders like so:
// const MAIN_VS: &str = include_str!("../static/shaders/quad.vert.glsl");

#[component]
fn FieldCanvas(field_shaders: (String, String), line_shaders: (String, String)) -> impl IntoView {
    view! {
        <main class="min-h-screen flex">
            <aside class="w-96 p-4 border-r border-zinc-800 bg-zinc-950 text-zinc-100">
                <h2 class="font-semibold mb-2">sidebar</h2>
                <p class="text-sm opacity-70">Leptos + WebGL2 + Tailwind</p>
            </aside>
            <section class="flex-1 relative bg-black">
                <CanvasGL class="w-full h-screen block" field_shaders=field_shaders line_shaders=line_shaders/>
            </section>
        </main>
    }
}

type RafClosure = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

#[component]
fn CanvasGL(
    class: &'static str,
    field_shaders: (String, String),
    line_shaders: (String, String),
) -> impl IntoView {
    let canvas_ref: NodeRef<html::Canvas> = NodeRef::new();

    canvas_ref.on_load(move |_| {
        // --- grab canvas & gl2
        let canvas: HtmlCanvasElement = canvas_ref.get().expect("canvas in the DOM");
        let gl: GL = canvas
            .get_context("webgl2")
            .unwrap()
            .ok_or_else(|| JsValue::from_str("no webgl2"))
            .unwrap()
            .dyn_into::<GL>()
            .unwrap();

        // --- DPR-aware resize
        let mut last_w: u32 = 0;
        let mut last_h: u32 = 0;
        let resize = |canvas: &HtmlCanvasElement, gl: &GL, last_w: &mut u32, last_h: &mut u32| {
            let dpr = leptos_use::use_window()
                .as_ref()
                .unwrap()
                .device_pixel_ratio();
            let w = (canvas.client_width() as f64 * dpr).round() as u32;
            let h = (canvas.client_height() as f64 * dpr).round() as u32;
            if w != *last_w || h != *last_h {
                *last_w = w;
                *last_h = h;
                canvas.set_width(w);
                canvas.set_height(h);
                gl.viewport(0, 0, w as i32, h as i32);
            }
        };

        // --- program (full-screen quad)
        // let prog = create_program(&gl, MAIN_VS, &field_shaders.1).expect("program");
        let prog = create_program(&gl, &field_shaders.0, &field_shaders.1).expect("program");
        gl.use_program(Some(&prog));

        // attribute: a_pos
        let a_pos_loc = gl.get_attrib_location(&prog, "a_pos") as u32;

        // fullscreen quad (two triangles) in clip space
        let quad: [f32; 12] = [
            -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0,
        ];

        let vbo = gl.create_buffer().unwrap();
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vbo));
        gl.buffer_data_with_array_buffer_view(
            GL::ARRAY_BUFFER,
            &js_sys::Float32Array::from(quad.as_slice()),
            GL::STATIC_DRAW,
        );
        gl.enable_vertex_attrib_array(a_pos_loc);
        gl.vertex_attrib_pointer_with_i32(a_pos_loc, 2, GL::FLOAT, false, 0, 0);

        // --- RG32F texture for (Ex,Ey)
        let tex = gl.create_texture().unwrap();
        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&tex));

        gl.get_extension("OES_texture_float_linear").unwrap();
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::LINEAR as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::LINEAR as i32);

        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);
        gl.pixel_storei(GL::UNPACK_ALIGNMENT, 1);

        // bind sampler to texture unit 0
        if let Some(loc) = gl.get_uniform_location(&prog, "u_exTex") {
            gl.uniform1i(Some(&loc), 0);
        };

        // solver + field buffer
        let mut solver = em::Solver::new(256, 256);
        solver.clear();
        solver.add(0.35, 0.5, 1.0);
        solver.add(0.65, 0.5, -1.0);

        // zero-copy upload helper
        let upload = |gl: &GL, w: i32, h: i32, data: &[f32]| {
            // SAFETY: view lives only for this call; no move of `data` during call.
            let view = unsafe { js_sys::Float32Array::view(data) };
            gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                GL::TEXTURE_2D,
                0,
                GL::RG32F as i32, // internalformat
                w,
                h,
                0,
                GL::RG,    // format
                GL::FLOAT, // type
                Some(&view),
            )
            .unwrap();
            // view dropped here
        };
        //
        // --- RAF loop
        let start = leptos_use::use_window()
            .as_ref()
            .unwrap()
            .performance()
            .unwrap()
            .now();
        let raf_cb: RafClosure = Rc::new(RefCell::new(None));
        let raf_cb2 = raf_cb.clone();

        *raf_cb2.borrow_mut() = Some(Closure::wrap(Box::new(move |t_ms: f64| {
            // size
            resize(&canvas, &gl, &mut last_w, &mut last_h);

            let t = ((t_ms - start) * 0.001) as f32;
            solver.step(t);
            // upload field → RG32F texture (no copy)
            upload(&gl, solver.w as i32, solver.h as i32, &solver.field);

            // draw
            gl.clear_color(0.02, 0.02, 0.05, 1.0);
            gl.clear(GL::COLOR_BUFFER_BIT);
            gl.draw_arrays(GL::TRIANGLES, 0, 6);

            // next frame
            leptos_use::use_window()
                .as_ref()
                .unwrap()
                .request_animation_frame(raf_cb.borrow().as_ref().unwrap().as_ref().unchecked_ref())
                .unwrap();
        }) as Box<dyn FnMut(f64)>));

        // kick it off
        leptos_use::use_window()
            .as_ref()
            .unwrap()
            .request_animation_frame(raf_cb2.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .unwrap();
    });

    view! { <canvas node_ref=canvas_ref class=class></canvas> }
}

fn create_program(gl: &GL, vert_shader: &str, frag_shader: &str) -> Result<WebGlProgram, String> {
    let vert_shader = compile_shader(gl, GL::VERTEX_SHADER, vert_shader)?;
    let frag_shader = compile_shader(gl, GL::FRAGMENT_SHADER, frag_shader)?;
    let program = gl
        .create_program()
        .ok_or_else(|| String::from("Unable to create shader object"))?;
    gl.attach_shader(&program, &vert_shader);
    gl.attach_shader(&program, &frag_shader);
    gl.link_program(&program);
    if !gl
        .get_program_parameter(&program, GL::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Err(gl
            .get_program_info_log(&program)
            .unwrap_or_else(|| String::from("Unknown error creating program object")))?;
    }
    Ok(program)
}

fn compile_shader(gl: &GL, shader_type: u32, source: &str) -> Result<WebGlShader, String> {
    let shader = gl
        .create_shader(shader_type)
        .ok_or_else(|| String::from("Unable to create shader object"))?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    match gl
        .get_shader_parameter(&shader, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        true => Ok(shader),
        false => Err(gl
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| String::from("Unknown error creating shader"))),
    }
}

mod em {
    #[derive(Clone, Copy)]
    pub struct Charge {
        pub x: f32,
        pub y: f32,
        pub q: f32,
    }

    pub struct Solver {
        pub w: usize,
        pub h: usize,
        /// interleaved [Ex, Ey, Ex, Ey, ...]
        pub field: Vec<f32>,
        pub charges: Vec<Charge>,
        soft2: f32,
        k: f32,
    }

    impl Solver {
        pub fn new(w: usize, h: usize) -> Self {
            // ~3/4 pixel softening in normalized coords
            let soft_px: f32 = 0.75;
            let sx = soft_px / w as f32;
            let sy = soft_px / h as f32;
            Self {
                w,
                h,
                field: vec![0.0; w * h * 2],
                charges: vec![],
                soft2: sx * sx + sy * sy,
                k: 1.0,
            }
        }

        pub fn clear(&mut self) {
            self.charges.clear();
        }
        pub fn add(&mut self, x: f32, y: f32, q: f32) {
            self.charges.push(Charge { x, y, q });
        }

        pub fn step(&mut self, _t: f32) {
            let w = self.w as i32;
            let h = self.h as i32;
            for jy in 0..h {
                let fy = (jy as f32 + 0.5) / self.h as f32;
                for ix in 0..w {
                    let fx = (ix as f32 + 0.5) / self.w as f32;

                    let mut ex = 0.0f32;
                    let mut ey = 0.0f32;
                    for c in &self.charges {
                        let dx = fx - c.x;
                        let dy = fy - c.y;
                        let r2 = dx * dx + dy * dy + self.soft2;
                        let r = r2.sqrt();
                        let r3 = r2 * r;
                        let s = self.k * c.q / r3;
                        ex += s * dx;
                        ey += s * dy;
                    }

                    let base = ((jy as usize) * self.w + ix as usize) * 2;
                    self.field[base] = ex;
                    self.field[base + 1] = ey;
                }
            }
        }
    }
}
