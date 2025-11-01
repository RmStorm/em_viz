use std::{cell::RefCell, rc::Rc};

use leptos_use::use_window;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlProgram, WebGlShader};

const RIBBON3D_VS: &str = include_str!("../static/shaders/ribbon3d.vert.glsl");
const RIBBON3D_FS: &str = include_str!("../static/shaders/ribbon3d.frag.glsl");
const SPHERE_VS: &str = include_str!("../static/shaders/sphere_impostor.vert.glsl");
const SPHERE_FS: &str = include_str!("../static/shaders/sphere_impostor.frag.glsl");

pub struct Renderer {
    canvas: HtmlCanvasElement,
    gl: GL,

    last_w: u32,
    last_h: u32,

    ribbon_prog: WebGlProgram,
    ribbon_vbo: web_sys::WebGlBuffer,
    ribbon_counts: Vec<i32>,
    u_rib_view: Option<web_sys::WebGlUniformLocation>,
    u_rib_proj: Option<web_sys::WebGlUniformLocation>,
    u_rib_viewport: Option<web_sys::WebGlUniformLocation>,
    u_rib_halfwidth: Option<web_sys::WebGlUniformLocation>,
    u_rib_alpha: Option<web_sys::WebGlUniformLocation>,

    sphere_prog: WebGlProgram,
    sphere_vbo: web_sys::WebGlBuffer,
    u_sph_view: Option<web_sys::WebGlUniformLocation>,
    u_sph_proj: Option<web_sys::WebGlUniformLocation>,
    u_sph_size: Option<web_sys::WebGlUniformLocation>,
}

impl Renderer {
    pub fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let gl: GL = canvas
            .get_context("webgl2")?
            .ok_or(JsValue::from_str("no webgl2"))?
            .dyn_into::<GL>()?;

        gl.enable(GL::DEPTH_TEST);
        gl.depth_func(GL::LEQUAL);
        gl.enable(GL::CULL_FACE);
        gl.cull_face(GL::BACK);

        // programs
        let ribbon_prog = create_program(&gl, RIBBON3D_VS, RIBBON3D_FS)?;
        let sphere_prog = create_program(&gl, SPHERE_VS, SPHERE_FS)?;

        let ribbon_vbo = gl.create_buffer().unwrap();
        let sphere_vbo = gl.create_buffer().unwrap();

        // uniforms
        let u_rib_view = gl.get_uniform_location(&ribbon_prog, "u_view");
        let u_rib_proj = gl.get_uniform_location(&ribbon_prog, "u_proj");
        let u_rib_viewport = gl.get_uniform_location(&ribbon_prog, "u_viewport");
        let u_rib_halfwidth = gl.get_uniform_location(&ribbon_prog, "u_halfWidthPx");
        let u_rib_alpha = gl.get_uniform_location(&ribbon_prog, "u_alpha");

        let u_sph_view = gl.get_uniform_location(&sphere_prog, "u_view");
        let u_sph_proj = gl.get_uniform_location(&sphere_prog, "u_proj");
        let u_sph_size = gl.get_uniform_location(&sphere_prog, "u_pointSizePx");

        Ok(Self {
            canvas,
            gl,
            last_w: 0,
            last_h: 0,
            ribbon_prog,
            ribbon_vbo,
            ribbon_counts: Vec::new(),
            u_rib_view,
            u_rib_proj,
            u_rib_viewport,
            u_rib_halfwidth,
            u_rib_alpha,
            sphere_prog,
            sphere_vbo,
            u_sph_view,
            u_sph_proj,
            u_sph_size,
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

    pub fn update_ribbons(&mut self, interleaved: &[Vec<f32>]) {
        let total: usize = interleaved.iter().map(|p| p.len()).sum();
        let mut flat = Vec::with_capacity(total);
        self.ribbon_counts.clear();

        for p in interleaved {
            self.ribbon_counts.push((p.len() / 8) as i32); // 8 floats per vertex
            flat.extend_from_slice(p);
        }

        self.gl
            .bind_buffer(GL::ARRAY_BUFFER, Some(&self.ribbon_vbo));
        self.gl.buffer_data_with_array_buffer_view(
            GL::ARRAY_BUFFER,
            &js_sys::Float32Array::from(flat.as_slice()),
            GL::DYNAMIC_DRAW,
        );
    }

    pub fn clear_color_depth(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.resize(); // <- make sure last_w/last_h are up to date
        self.gl.clear_color(r, g, b, a);
        self.gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);
    }
    fn draw_ribbons_inner(
        &self,
        view: &[f32; 16],
        proj: &[f32; 16],
        half_width_px: f32,
        alpha: f32,
        blend: (u32, u32), // (src, dst)
        depth_write: bool,
    ) {
        let gl = &self.gl;
        gl.use_program(Some(&self.ribbon_prog));

        if let Some(u) = &self.u_rib_view {
            gl.uniform_matrix4fv_with_f32_array(Some(u), false, view);
        }
        if let Some(u) = &self.u_rib_proj {
            gl.uniform_matrix4fv_with_f32_array(Some(u), false, proj);
        }

        let vw = self.last_w.max(1) as f32;
        let vh = self.last_h.max(1) as f32;
        if let Some(u) = &self.u_rib_viewport {
            gl.uniform2f(Some(u), vw, vh);
        }
        if let Some(u) = &self.u_rib_halfwidth {
            gl.uniform1f(Some(u), half_width_px);
        }
        if let Some(u) = &self.u_rib_alpha {
            gl.uniform1f(Some(u), alpha);
        }

        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.ribbon_vbo));
        let stride = 8 * 4;
        let a_center = gl.get_attrib_location(&self.ribbon_prog, "a_center") as u32;
        let a_tan = gl.get_attrib_location(&self.ribbon_prog, "a_tangent") as u32;
        let a_side = gl.get_attrib_location(&self.ribbon_prog, "a_side") as u32;
        let a_tone = gl.get_attrib_location(&self.ribbon_prog, "a_tone") as u32;

        gl.enable_vertex_attrib_array(a_center);
        gl.enable_vertex_attrib_array(a_tan);
        gl.enable_vertex_attrib_array(a_side);
        gl.enable_vertex_attrib_array(a_tone);

        gl.enable(GL::BLEND);
        gl.blend_func(blend.0, blend.1);
        gl.disable(GL::CULL_FACE);
        gl.depth_mask(depth_write);

        let mut base_vertices = 0i32;
        for &count in &self.ribbon_counts {
            let base_bytes = base_vertices * stride;
            gl.vertex_attrib_pointer_with_i32(a_center, 3, GL::FLOAT, false, stride, base_bytes);
            let tangent_bytes = base_bytes + 3 * 4;
            gl.vertex_attrib_pointer_with_i32(a_tan, 3, GL::FLOAT, false, stride, tangent_bytes);
            let side_bytes = base_bytes + 6 * 4;
            gl.vertex_attrib_pointer_with_i32(a_side, 1, GL::FLOAT, false, stride, side_bytes);
            let tone_bytes = base_bytes + 7 * 4;
            gl.vertex_attrib_pointer_with_i32(a_tone, 1, GL::FLOAT, false, stride, tone_bytes);

            gl.draw_arrays(GL::TRIANGLE_STRIP, 0, count);
            base_vertices += count;
        }

        gl.depth_mask(true);
        gl.disable(GL::BLEND);
        gl.enable(GL::CULL_FACE);
    }

    pub fn draw_ribbons_beautified(&self, view: &[f32; 16], proj: &[f32; 16]) {
        // 1) Glow: thicker, additive, no depth write
        self.draw_ribbons_inner(
            view,
            proj,
            2.5,                // half-width ~25px (5px total)
            0.18,               // low alpha
            (GL::ONE, GL::ONE), // additive
            false,              // don't write depth
        );
        // 2) Main: thin, standard alpha, depth write ON
        self.draw_ribbons_inner(
            view,
            proj,
            2.0, // ~4px total
            1.0, // full alpha into shader’s AA
            (GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA),
            true,
        );
    }

    pub fn update_charge_points(&self, centers: &[[f32; 3]]) {
        let flat: Vec<f32> = centers.iter().flatten().copied().collect();
        self.gl
            .bind_buffer(GL::ARRAY_BUFFER, Some(&self.sphere_vbo));
        self.gl.buffer_data_with_array_buffer_view(
            GL::ARRAY_BUFFER,
            &js_sys::Float32Array::from(flat.as_slice()),
            GL::DYNAMIC_DRAW,
        );
    }

    pub fn draw_charges(&self, view: &[f32; 16], proj: &[f32; 16], point_size_px: f32) {
        self.gl.use_program(Some(&self.sphere_prog));
        if let Some(u) = &self.u_sph_view {
            self.gl
                .uniform_matrix4fv_with_f32_array(Some(u), false, view);
        }
        if let Some(u) = &self.u_sph_proj {
            self.gl
                .uniform_matrix4fv_with_f32_array(Some(u), false, proj);
        }
        if let Some(u) = &self.u_sph_size {
            self.gl.uniform1f(Some(u), point_size_px);
        }

        self.gl
            .bind_buffer(GL::ARRAY_BUFFER, Some(&self.sphere_vbo));
        let a_center = self.gl.get_attrib_location(&self.sphere_prog, "a_center") as u32;
        self.gl.enable_vertex_attrib_array(a_center);
        self.gl
            .vertex_attrib_pointer_with_i32(a_center, 3, GL::FLOAT, false, 3 * 4, 0);

        // WebGL2: gl_PointSize works without enabling PROGRAM_POINT_SIZE (doesn’t exist here)
        let count = (self
            .gl
            .get_buffer_parameter(GL::ARRAY_BUFFER, GL::BUFFER_SIZE)
            .as_f64()
            .unwrap_or(0.0) as i32)
            / (3 * 4);
        self.gl.draw_arrays(GL::POINTS, 0, count);
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
