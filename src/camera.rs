use glam::{Mat4, Vec3};
use wasm_bindgen::JsCast;

pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_y: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}
impl Camera {
    pub fn new(aspect: f32) -> Self {
        Self {
            eye: Vec3::new(0.0, 2.0, 4.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov_y: 45f32.to_radians(),
            aspect,
            near: 0.01,
            far: 50.0,
        }
    }
    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye, self.target, self.up)
    }
    pub fn proj(&self) -> Mat4 {
        Mat4::perspective_rh_gl(self.fov_y, self.aspect, self.near, self.far)
    }
    pub fn update_from_orbit(&mut self, o: &Orbit) {
        self.target = o.target;
        self.eye = orbit_eye(o.target, o.yaw, o.pitch, o.radius);
    }
}

#[derive(Clone, Copy)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub radius: f32,
    pub target: Vec3,
}

pub fn orbit_eye(target: Vec3, yaw: f32, pitch: f32, radius: f32) -> Vec3 {
    let cp = pitch.cos();
    let sp = pitch.sin();
    let cy = yaw.cos();
    let sy = yaw.sin();
    let dir = Vec3::new(cy * cp, sp, sy * cp);
    target + radius * dir
}

pub struct OrbitController {
    orbit: Orbit,
}
impl OrbitController {
    pub fn new() -> Self {
        Self {
            orbit: Orbit {
                yaw: 0.0,
                pitch: 0.35,
                radius: 4.5,
                target: Vec3::ZERO,
            },
        }
    }
    pub fn orbit(&self) -> Orbit {
        self.orbit
    }
    // pub fn set_aspect(&mut self, cam: &mut Camera, aspect: f32) {
    //     cam.aspect = aspect;
    //     cam.update_from_orbit(&self.orbit);
    // }

    pub fn attach(
        self,
        canvas: &web_sys::HtmlCanvasElement,
    ) -> std::rc::Rc<std::cell::RefCell<Self>> {
        use web_sys::{MouseEvent, PointerEvent, WheelEvent, window};
        let rc = std::rc::Rc::new(std::cell::RefCell::new(self));

        // prevent context menu
        {
            let c = canvas.clone();
            let on_ctx =
                wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |e: MouseEvent| {
                    e.prevent_default();
                });
            c.add_event_listener_with_callback("contextmenu", on_ctx.as_ref().unchecked_ref())
                .unwrap();
            on_ctx.forget();
        }

        // drag state lives outside to keep controller clean
        #[derive(Default)]
        struct Drag {
            active: bool,
            button: i16,
            last_x: f32,
            last_y: f32,
        }
        let drag = std::rc::Rc::new(std::cell::RefCell::new(Drag::default()));

        // pointer down
        {
            let drag = drag.clone();
            let c2 = canvas.clone();
            let on_down =
                wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |e: PointerEvent| {
                    let mut d = drag.borrow_mut();
                    d.active = true;
                    d.button = e.button();
                    d.last_x = e.client_x() as f32;
                    d.last_y = e.client_y() as f32;
                    let _ = c2.set_pointer_capture(e.pointer_id());
                    e.prevent_default();
                });
            canvas
                .add_event_listener_with_callback("pointerdown", on_down.as_ref().unchecked_ref())
                .unwrap();
            on_down.forget();
        }

        // pointer move
        {
            let drag = drag.clone();
            let rc2 = rc.clone();
            let on_move =
                wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |e: PointerEvent| {
                    let mut d = drag.borrow_mut();
                    if !d.active {
                        return;
                    }
                    let x = e.client_x() as f32;
                    let y = e.client_y() as f32;
                    let dx = x - d.last_x;
                    let dy = y - d.last_y;
                    d.last_x = x;
                    d.last_y = y;

                    let mut this = rc2.borrow_mut();
                    let o = &mut this.orbit;

                    let dpr = window().unwrap().device_pixel_ratio() as f32;
                    let rot = 0.005 / dpr;
                    let pan = 0.0015 * o.radius;

                    if d.button == 2 {
                        o.yaw += dx * rot;
                        o.pitch = (o.pitch + dy * rot).clamp(-1.45, 1.45);
                    } else if d.button == 1 {
                        // pan in camera plane
                        let cy = o.yaw.cos();
                        let sy = o.yaw.sin();
                        let cp = o.pitch.cos();
                        let sp = o.pitch.sin();
                        let fwd = glam::Vec3::new(cy * cp, sp, sy * cp);
                        let right = fwd.cross(glam::Vec3::Y).normalize_or_zero();
                        let up = right.cross(fwd).normalize_or_zero();
                        o.target += (dx * pan) * right + (dy * pan) * up;
                    }

                    e.prevent_default();
                });
            canvas
                .add_event_listener_with_callback("pointermove", on_move.as_ref().unchecked_ref())
                .unwrap();
            on_move.forget();
        }

        // pointer up/cancel
        {
            let drag = drag.clone();
            let on_up =
                wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |_e: PointerEvent| {
                    drag.borrow_mut().active = false;
                });
            let w = window().unwrap();
            w.add_event_listener_with_callback("pointerup", on_up.as_ref().unchecked_ref())
                .unwrap();
            w.add_event_listener_with_callback("pointercancel", on_up.as_ref().unchecked_ref())
                .unwrap();
            on_up.forget();
        }

        // wheel zoom
        {
            let rc2 = rc.clone();
            let on_wheel =
                wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |e: WheelEvent| {
                    let mut this = rc2.borrow_mut();
                    let z = 0.0015;
                    let factor = (1.0 + z * e.delta_y() as f32).max(0.1);
                    this.orbit.radius = (this.orbit.radius * factor).clamp(0.3, 50.0);
                    e.prevent_default();
                });
            canvas
                .add_event_listener_with_callback("wheel", on_wheel.as_ref().unchecked_ref())
                .unwrap();
            on_wheel.forget();
        }

        rc
    }
}
