use std::{cell::RefCell, rc::Rc};

use leptos_use::use_window;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlProgram, WebGlShader};

const QUAD_VS: &str = include_str!("../../static/shaders/quad.vert.glsl");
const FIELD_FS: &str = include_str!("../../static/shaders/viridis.frag.glsl");
const LINE_VS: &str = include_str!("../../static/shaders/line.vert.glsl");
const LINE_FS: &str = include_str!("../../static/shaders/line.frag.glsl");

// type Raf = std::rc::Rc<std::cell::RefCell<Option<Closure<dyn FnMut(f64)>>>>;

pub struct Renderer {
    canvas: HtmlCanvasElement,
    gl: GL,
    // field
    field_prog: WebGlProgram,
    quad_vbo: web_sys::WebGlBuffer,
    field_tex: web_sys::WebGlTexture,
    // lines
    line_prog: WebGlProgram,
    line_vbo: web_sys::WebGlBuffer,
    line_counts: Vec<i32>,
    // sizing
    last_w: u32,
    last_h: u32,
}

impl Renderer {
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let gl: GL = canvas
            .get_context("webgl2")?
            .ok_or(JsValue::from_str("no webgl2"))?
            .dyn_into::<GL>()?;

        // programs
        let field_prog = create_program(&gl, QUAD_VS, FIELD_FS)?;
        let line_prog = create_program(&gl, LINE_VS, LINE_FS)?;

        // quad vbo
        let quad: [f32; 12] = [
            -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0,
        ];
        let quad_vbo = gl.create_buffer().unwrap();
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&quad_vbo));
        gl.buffer_data_with_array_buffer_view(
            GL::ARRAY_BUFFER,
            &js_sys::Float32Array::from(quad.as_slice()),
            GL::STATIC_DRAW,
        );

        // field texture
        let field_tex = gl.create_texture().unwrap();
        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&field_tex));
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);

        // bind sampler uniform
        gl.use_program(Some(&field_prog));
        if let Some(loc) = gl.get_uniform_location(&field_prog, "u_exTex") {
            gl.uniform1i(Some(&loc), 0);
        }

        // line vbo (dynamic)
        let line_vbo = gl.create_buffer().unwrap();

        Ok(Self {
            canvas,
            gl,
            field_prog,
            quad_vbo,
            field_tex,
            line_prog,
            line_vbo,
            line_counts: Vec::new(),
            last_w: 0,
            last_h: 0,
        })
    }

    fn resize(&mut self) {
        let dpr = use_window().as_ref().unwrap().device_pixel_ratio();
        let w = (self.canvas.client_width() as f64 * dpr).round() as u32;
        let h = (self.canvas.client_height() as f64 * dpr).round() as u32;
        if w != self.last_w || h != self.last_h {
            self.last_w = w;
            self.last_h = h;
            self.canvas.set_width(w);
            self.canvas.set_height(h);
            self.gl.viewport(0, 0, w as i32, h as i32);
        }
    }

    pub fn update_field_texture(&self, w: i32, h: i32, data: &[f32]) {
        let view = unsafe { js_sys::Float32Array::view(data) };
        self.gl.active_texture(GL::TEXTURE0);
        self.gl.bind_texture(GL::TEXTURE_2D, Some(&self.field_tex));
        self.gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
                GL::TEXTURE_2D,
                0,
                GL::RG32F as i32,
                w,
                h,
                0,
                GL::RG,
                GL::FLOAT,
                Some(&view),
            )
            .unwrap();
    }

    pub fn update_lines(&mut self, polylines: &[Vec<f32>]) {
        // flatten to one buffer; store per-line counts
        let total_floats: usize = polylines.iter().map(|p| p.len()).sum();
        let mut flat = Vec::with_capacity(total_floats);
        self.line_counts.clear();
        for p in polylines {
            self.line_counts.push((p.len() / 2) as i32);
            flat.extend_from_slice(p);
        }
        self.gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.line_vbo));
        self.gl.buffer_data_with_array_buffer_view(
            GL::ARRAY_BUFFER,
            &js_sys::Float32Array::from(flat.as_slice()),
            GL::DYNAMIC_DRAW,
        );
    }

    pub fn draw(&mut self) {
        self.resize();

        // draw field quad
        self.gl.use_program(Some(&self.field_prog));
        self.gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.quad_vbo));
        let a_pos = self.gl.get_attrib_location(&self.field_prog, "a_pos") as u32;
        self.gl.enable_vertex_attrib_array(a_pos);
        self.gl
            .vertex_attrib_pointer_with_i32(a_pos, 2, GL::FLOAT, false, 0, 0);

        self.gl.clear_color(0.02, 0.02, 0.05, 1.0);
        self.gl.clear(GL::COLOR_BUFFER_BIT);
        self.gl.draw_arrays(GL::TRIANGLES, 0, 6);
    }

    pub fn draw_lines(&self) {
        if self.line_counts.is_empty() {
            return;
        }
        self.gl.use_program(Some(&self.line_prog));
        self.gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.line_vbo));
        let a_pos = self.gl.get_attrib_location(&self.line_prog, "pos") as u32;
        self.gl.enable_vertex_attrib_array(a_pos);
        self.gl
            .vertex_attrib_pointer_with_i32(a_pos, 2, GL::FLOAT, false, 0, 0);

        // color uniform (white)
        if let Some(loc) = self.gl.get_uniform_location(&self.line_prog, "uColor") {
            self.gl.uniform4f(Some(&loc), 1.0, 1.0, 1.0, 0.9);
        }

        // draw each strip in sequence (offset advances by previous counts)
        let mut base_vertex = 0;
        for &count in &self.line_counts {
            // WebGL2 doesn't support baseVertex drawArrays; split is simplest:
            // We rebind a subrange via stride/offset? In WebGL2 we can use bufferSubData,
            // but simplest: drawArrays starting at base via vertexAttribPointer? Not possible.
            // So instead: upload each strip separately OR draw all at once after gl.bufferData.
            // Easiest compromise: draw all lines as separate calls, updating buffer per line.
            // If performance matters later, pack with element arrays. For now:
            // (We uploaded as one big buffer, so we draw slices by rebinding a new pointer offset.)
            self.gl
                .vertex_attrib_pointer_with_i32(a_pos, 2, GL::FLOAT, false, 0, base_vertex * 4);
            self.gl.draw_arrays(GL::LINE_STRIP, 0, count);
            base_vertex += count * 2;
        }
    }

    pub fn start_raf_rc<F>(ren: Rc<RefCell<Renderer>>, mut frame: F)
    where
        F: 'static + FnMut(&Rc<RefCell<Renderer>>, f32),
    {
        let start = use_window().as_ref().unwrap().performance().unwrap().now();
        let raf = Rc::new(RefCell::new(None::<Closure<dyn FnMut(f64)>>));
        let raf2 = raf.clone();
        let ren_clone = ren.clone();

        *raf2.borrow_mut() = Some(Closure::wrap(Box::new(move |t_ms: f64| {
            let t = ((t_ms - start) * 0.001) as f32;
            frame(&ren_clone, t);
            use_window()
                .as_ref()
                .unwrap()
                .request_animation_frame(raf.borrow().as_ref().unwrap().as_ref().unchecked_ref())
                .unwrap();
        }) as Box<dyn FnMut(f64)>));

        use_window()
            .as_ref()
            .unwrap()
            .request_animation_frame(raf2.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .unwrap();
    }
}

// --- small GL helpers

fn create_program(gl: &GL, vs: &str, fs: &str) -> Result<WebGlProgram, JsValue> {
    let v = compile_shader(gl, GL::VERTEX_SHADER, vs)?;
    let f = compile_shader(gl, GL::FRAGMENT_SHADER, fs)?;
    let p = gl.create_program().ok_or(JsValue::from_str("no program"))?;
    gl.attach_shader(&p, &v);
    gl.attach_shader(&p, &f);
    gl.link_program(&p);
    let ok = gl
        .get_program_parameter(&p, GL::LINK_STATUS)
        .as_bool()
        .unwrap_or(false);
    if !ok {
        return Err(JsValue::from_str(
            &gl.get_program_info_log(&p).unwrap_or_default(),
        ));
    }
    Ok(p)
}
fn compile_shader(gl: &GL, ty: u32, src: &str) -> Result<WebGlShader, JsValue> {
    let sh = gl.create_shader(ty).ok_or(JsValue::from_str("no shader"))?;
    gl.shader_source(&sh, src);
    gl.compile_shader(&sh);
    let ok = gl
        .get_shader_parameter(&sh, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false);
    if !ok {
        return Err(JsValue::from_str(
            &gl.get_shader_info_log(&sh).unwrap_or_default(),
        ));
    }
    Ok(sh)
}
