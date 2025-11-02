use crate::em3d::{Charge3D, sample_e3d};
use glam::Vec3;

fn rk4(p: Vec3, h: f32, sign: f32, charges: &[Charge3D], k: f32, soft2: f32) -> (Vec3, f32, Vec3) {
    let f = |x: Vec3| {
        let e = sample_e3d(x, charges, k, soft2);
        let m = e.length();
        let dir = if m > 1e-6 { e / m } else { Vec3::ZERO };
        (dir * sign, m, e)
    };
    let (k1, m1, e1) = f(p);
    let (k2, _, _) = f(p + 0.5 * h * k1);
    let (k3, _, _) = f(p + 0.5 * h * k2);
    let (k4, _, _) = f(p + h * k3);
    let dir = (k1 + 2.0 * k2 + 2.0 * k3 + k4) / 6.0;
    (p + h * dir, m1, e1)
}

/// Build ribbon vertex data: for each point we emit two verts (side=-1,+1).
/// Layout per-vertex: [px,py,pz, tx,ty,tz, side, t]
pub fn integrate_streamline_ribbon_signed(
    seed: Vec3,
    charges: &[Charge3D],
    k: f32,
    soft2: f32,
    h: f32,
    max_pts: usize,
    sign: f32, // +1 follow E, -1 go against E
) -> Vec<f32> {
    let mut out = Vec::with_capacity(max_pts * 2 * 8);
    let mut p = seed;
    let mut prev = p + Vec3::Z;

    for _ in 0..max_pts {
        let (p2, mag, _e) = rk4(p, h, sign, charges, k, soft2);
        if !p2.is_finite() {
            break;
        }
        let t = (mag / (1.0 + mag)).powf(0.75);
        let tan = (p2 - prev).normalize_or_zero();
        prev = p;
        p = p2;

        for &side in &[-1.0f32, 1.0] {
            out.extend_from_slice(&[p.x, p.y, p.z, tan.x, tan.y, tan.z, side, t]);
        }
        if !(1e-6..=1e4).contains(&mag) || p.length() > 250.0 {
            break;
        }
    }
    out
}
