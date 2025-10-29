use crate::solver::Solver;

pub struct Streamliner {
    seeds: u32,
    dirty: bool,
}
impl Streamliner {
    pub fn new() -> Self {
        Self {
            seeds: 16,
            dirty: true,
        }
    }
    pub fn set_seeds(&mut self, n: u32) {
        self.seeds = n;
        self.dirty = true;
    }
    pub fn dirty(&mut self) -> bool {
        let d = self.dirty;
        self.dirty = false;
        d
    }

    pub fn recompute(
        &self,
        s: &Solver,
        seeds_per_axis: u32,
        step_scale: f32,
        max_pts: usize,
    ) -> Vec<Vec<f32>> {
        let seeds = make_seeds(seeds_per_axis);
        let mut out = Vec::with_capacity(seeds.len());
        for (u, v) in seeds {
            out.push(integrate_streamline(s, u, v, step_scale, max_pts));
        }
        out
    }
}

fn make_seeds(n: u32) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity((n * n) as usize);
    for j in 0..n {
        for i in 0..n {
            let u = (i as f32 + 0.5) / n as f32;
            let v = (j as f32 + 0.5) / n as f32;
            out.push((u, v));
        }
    }
    out
}

// bilinear sample Ex,Ey at UV
fn sample_field(s: &Solver, u: f32, v: f32) -> (f32, f32) {
    let w = s.w as i32;
    let h = s.h as i32;
    let uu = u.clamp(0.0, 1.0);
    let vv = v.clamp(0.0, 1.0);
    let x = uu * (w - 1) as f32;
    let y = vv * (h - 1) as f32;
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let i00 = ((y0 as usize * s.w + x0 as usize) * 2) as usize;
    let i10 = ((y0 as usize * s.w + x1 as usize) * 2) as usize;
    let i01 = ((y1 as usize * s.w + x0 as usize) * 2) as usize;
    let i11 = ((y1 as usize * s.w + x1 as usize) * 2) as usize;

    let ex00 = s.field[i00];
    let ey00 = s.field[i00 + 1];
    let ex10 = s.field[i10];
    let ey10 = s.field[i10 + 1];
    let ex01 = s.field[i01];
    let ey01 = s.field[i01 + 1];
    let ex11 = s.field[i11];
    let ey11 = s.field[i11 + 1];

    let ex0 = ex00 * (1.0 - tx) + ex10 * tx;
    let ex1 = ex01 * (1.0 - tx) + ex11 * tx;
    let ey0 = ey00 * (1.0 - tx) + ey10 * tx;
    let ey1 = ey01 * (1.0 - tx) + ey11 * tx;

    (ex0 * (1.0 - ty) + ex1 * ty, ey0 * (1.0 - ty) + ey1 * ty)
}

fn norm_dir(ex: f32, ey: f32) -> (f32, f32, f32) {
    let m = (ex * ex + ey * ey).sqrt();
    if m < 1e-6 {
        (0.0, 0.0, m)
    } else {
        (ex / m, ey / m, m)
    }
}

fn rk4_step(s: &Solver, u: f32, v: f32, h: f32, sign: f32) -> (f32, f32, f32) {
    let f = |uu: f32, vv: f32| {
        let (ex, ey) = sample_field(s, uu, vv);
        let (dx, dy, mag) = norm_dir(ex, ey);
        (dx * sign, dy * sign, mag)
    };
    let (k1x, k1y, k1m) = f(u, v);
    let (k2x, k2y, _) = f(u + 0.5 * h * k1x, v + 0.5 * h * k1y);
    let (k3x, k3y, _) = f(u + 0.5 * h * k2x, v + 0.5 * h * k2y);
    let (k4x, k4y, _) = f(u + h * k3x, v + h * k3y);
    let dx = (k1x + 2.0 * k2x + 2.0 * k3x + k4x) / 6.0;
    let dy = (k1y + 2.0 * k2y + 2.0 * k3y + k4y) / 6.0;
    (u + h * dx, v + h * dy, k1m)
}

fn integrate_streamline(
    s: &Solver,
    seed_u: f32,
    seed_v: f32,
    step_scale: f32,
    max_pts: usize,
) -> Vec<f32> {
    let h = step_scale / (s.w.max(s.h) as f32);

    let walk = |sign: f32| {
        let mut pts: Vec<f32> = Vec::with_capacity(max_pts * 2);
        let (mut u, mut v) = (seed_u, seed_v);
        for _ in 0..max_pts {
            if u <= 0.0 || u >= 1.0 || v <= 0.0 || v >= 1.0 {
                break;
            }
            // push clip-space
            pts.push(u * 2.0 - 1.0);
            pts.push(1.0 - v * 2.0);
            let (uu, vv, mag) = rk4_step(s, u, v, h, sign);
            u = uu;
            v = vv;
            if mag < 1e-5 || mag > 1e3 {
                break;
            }
        }
        pts
    };

    let mut neg = walk(-1.0);
    let fwd = walk(1.0);

    // merge: reverse neg (skip seed dup) + fwd
    let mut merged = Vec::with_capacity(neg.len() + fwd.len());
    // remove last point (the seed) from neg if present
    if neg.len() >= 2 {
        neg.truncate(neg.len() - 2);
    }
    for i in (0..neg.len()).step_by(2).rev() {
        merged.push(neg[i]);
        merged.push(neg[i + 1]);
    }
    merged.extend_from_slice(&fwd);
    merged
}
