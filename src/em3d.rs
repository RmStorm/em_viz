use glam::Vec3;

#[derive(Clone, Copy)]
pub struct Charge3D {
    pub pos: Vec3,
    pub q: f32,
    pub vel: Vec3, // NEW: velocity in world units / s (visual units)
}

pub fn sample_e3d(p: Vec3, charges: &[Charge3D], k: f32, soft2: f32) -> Vec3 {
    let mut e = Vec3::ZERO;
    for c in charges {
        let d = p - c.pos;
        let r2 = d.length_squared() + soft2;
        let r = r2.sqrt();
        e += (k * c.q / (r2 * r)) * d; // d / r^3
    }
    e
}

// Helper: field of a single charge (for B approximation)
fn sample_e_of_charge(p: Vec3, c: &Charge3D, k: f32, soft2: f32) -> Vec3 {
    let d = p - c.pos;
    let r2 = d.length_squared() + soft2;
    let r = r2.sqrt();
    (k * c.q / (r2 * r)) * d
}

/// Approx magnetic field (non-retarded, low-v): B(x) ≈ (1/c^2) Σ [ v_i × E_i(x) ]
pub fn sample_b3d(p: Vec3, charges: &[Charge3D], k: f32, soft2: f32, c_inv2: f32) -> Vec3 {
    let mut b = Vec3::ZERO;
    for c in charges {
        if c.vel.length_squared() < 1e-10 {
            continue;
        }
        let ei = sample_e_of_charge(p, c, k, soft2);
        b += c_inv2 * c.vel.cross(ei);
    }
    b
}
