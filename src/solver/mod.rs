#[derive(Clone, Copy)]
pub struct Charge {
    pub u: f32,
    pub v: f32,
    pub q: f32,
}

pub struct Solver {
    pub w: usize,
    pub h: usize,
    pub field: Vec<f32>, // [Ex,Ey,Ex,Ey,...]
    pub charges: Vec<Charge>,
    soft2: f32,
    k: f32,
}

impl Solver {
    pub fn new(w: usize, h: usize) -> Self {
        let soft_px = 0.75f32;
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
    pub fn add(&mut self, u: f32, v: f32, q: f32) {
        self.charges.push(Charge { u, v, q });
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
                    let dx = fx - c.u;
                    let dy = fy - c.v; // now dy>0 means charge is above pixel
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
