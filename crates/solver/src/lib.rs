use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Solver {
    width: usize,
    height: usize,
    // Interleaved field: [Ex, Ey, Ex, Ey, ...] length = w * h * 2
    field: Vec<f32>,
    charges: Vec<Charge>,
}

#[derive(Clone, Copy)]
struct Charge {
    x: f32, // normalized [0,1] across the domain
    y: f32, // normalized [0,1] across the domain
    q: f32, // charge strength (sign matters)
}

#[wasm_bindgen]
impl Solver {
    #[wasm_bindgen(constructor)]
    pub fn new(width: usize, height: usize) -> Solver {
        Solver {
            width,
            height,
            field: vec![0.0; width * height * 2],
            charges: Vec::new(),
        }
    }

    pub fn width(&self) -> usize { self.width }
    pub fn height(&self) -> usize { self.height }

    pub fn clear_charges(&mut self) { self.charges.clear(); }
    pub fn add_charge(&mut self, x: f32, y: f32, q: f32) {
        self.charges.push(Charge { x, y, q });
    }

    pub fn step(&mut self, _time: f32) {
        let w = self.width as i32;
        let h = self.height as i32;

        // ~3/4 pixel softening in normalized coords
        let soft_px: f32 = 0.75;
        let soft_x = soft_px / self.width as f32;
        let soft_y = soft_px / self.height as f32;
        let soft2 = soft_x*soft_x + soft_y*soft_y;

        let k: f32 = 1.0;

        for jy in 0..h {
            let fy = (jy as f32 + 0.5) / self.height as f32;
            for ix in 0..w {
                let fx = (ix as f32 + 0.5) / self.width as f32;

                let mut ex = 0.0f32;
                let mut ey = 0.0f32;

                for c in &self.charges {
                    let dx = fx - c.x;
                    let dy = fy - c.y;
                    let r2 = dx*dx + dy*dy + soft2;
                    let r = r2.sqrt();
                    let r3 = r2 * r;
                    let s = k * c.q / r3;
                    ex += s * dx;
                    ey += s * dy;
                }

                let base = ((jy as usize)*self.width + (ix as usize)) * 2;
                self.field[base]     = ex;
                self.field[base + 1] = ey;
            }
        }
    }

    pub fn field_ptr(&self) -> *const f32 { self.field.as_ptr() }
}
