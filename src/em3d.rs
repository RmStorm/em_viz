use glam::Vec3;

#[derive(Clone, Copy)]
pub struct Charge3D { pub pos: Vec3, pub q: f32 }

pub fn sample_e3d(p: Vec3, charges: &[Charge3D], k: f32, soft2: f32) -> Vec3 {
    let mut e = Vec3::ZERO;
    for c in charges {
        let d = p - c.pos;
        let r2 = d.length_squared() + soft2;
        let r  = r2.sqrt();
        e += (k * c.q / (r2 * r)) * d; // d / r^3
    }
    e
}
