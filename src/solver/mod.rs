#[derive(Clone, Copy, Debug)]
pub struct Charge {
    pub u: f32, // x in [0,1], left->right
    pub v: f32, // y in [0,1], bottom->top  (y-up!)
    pub q: f32,
    pub vu: f32, // velocity du/dt in UV/s
    pub vv: f32, // velocity dv/dt in UV/s
}

pub struct Solver {
    pub w: usize,
    pub h: usize,
    pub field: Vec<f32>,   // [Ex,Ey,Ex,Ey,...] total E
    pub b_field: Vec<f32>, // [Bz,...] scalar per pixel (accumulated over moving charges)
    pub charges: Vec<Charge>,
    single_e: Vec<f32>, // scratch: [Ex,Ey,...] for one charge
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
            b_field: vec![0.0; w * h],
            charges: vec![],
            single_e: vec![0.0; w * h * 2],
            soft2: sx * sx + sy * sy,
            k: 1.0,
        }
    }

    pub fn clear(&mut self) {
        self.charges.clear();
    }

    pub fn add(&mut self, u: f32, v: f32, q: f32) {
        self.charges.push(Charge {
            u,
            v,
            q,
            vu: 0.0,
            vv: 0.0,
        });
    }

    /// Compute total E field from all charges (y-up convention).
    pub fn step(&mut self, _t: f32) {
        let w = self.w as i32;
        let h = self.h as i32;
        for jy in 0..h {
            let fy = (jy as f32 + 0.5) / self.h as f32; // bottom->top
            for ix in 0..w {
                let fx = (ix as f32 + 0.5) / self.w as f32;

                let mut ex = 0.0f32;
                let mut ey = 0.0f32;
                for c in &self.charges {
                    let dx = fx - c.u;
                    let dy = fy - c.v;
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

    // --- change signature & body: no access to self.charges inside
    fn compute_single_e_for(&mut self, c: Charge) {
        let w = self.w as i32;
        let h = self.h as i32;
        for jy in 0..h {
            let fy = (jy as f32 + 0.5) / self.h as f32;
            for ix in 0..w {
                let fx = (ix as f32 + 0.5) / self.w as f32;

                let dx = fx - c.u;
                let dy = fy - c.v;
                let r2 = dx * dx + dy * dy + self.soft2;
                let r = r2.sqrt();
                let r3 = r2 * r;
                let s = self.k * c.q / r3;

                let base = ((jy as usize) * self.w + ix as usize) * 2;
                self.single_e[base] = s * dx;
                self.single_e[base + 1] = s * dy;
            }
        }
    }

    pub fn compute_b_from_velocities(&mut self, gain: f32) -> &[f32] {
        // clear B buffer
        for bz in &mut self.b_field {
            *bz = 0.0;
        }

        let npix = self.w * self.h;

        // iterate by index, copy the charge (Charge is Copy)
        for i in 0..self.charges.len() {
            let c = self.charges[i];
            if c.vu == 0.0 && c.vv == 0.0 {
                continue;
            }

            // compute E for that charge without touching self.charges
            self.compute_single_e_for(c);

            let (vu, vv) = (c.vu, c.vv);
            let e = &self.single_e;
            let b = &mut self.b_field;

            // Bz += gain * (vu * Ey - vv * Ex)
            for p in 0..npix {
                let ex = e[2 * p];
                let ey = e[2 * p + 1];
                b[p] += gain * (vu * ey - vv * ex);
            }
        }

        &self.b_field
    }

    /// Utility to set velocity for a charge (e.g., from drag)
    pub fn set_velocity(&mut self, idx: usize, vu: f32, vv: f32) {
        if let Some(c) = self.charges.get_mut(idx) {
            c.vu = vu;
            c.vv = vv;
        }
    }
}
