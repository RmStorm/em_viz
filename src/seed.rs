use glam::Quat;
use glam::Vec3;

pub fn fibonacci_sphere(center: Vec3, radius: f32, n: usize) -> Vec<Vec3> {
    // Uniform-ish distribution on a sphere
    let mut out = Vec::with_capacity(n);
    let phi = std::f32::consts::PI * (3.0_f32.sqrt() - 1.0); // ~2.399963
    for i in 0..n {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32; // (-1..1)
        let r = (1.0 - y * y).sqrt();
        let theta = phi * i as f32;
        let x = r * theta.cos();
        let z = r * theta.sin();
        out.push(center + radius * Vec3::new(x, y, z));
    }
    out
}

fn orthonormal_basis(n: Vec3) -> (Vec3, Vec3) {
    // returns two orthonormal vectors spanning plane ⟂ n
    let a = if n.abs().x < 0.9 { Vec3::X } else { Vec3::Y };
    let t1 = n.cross(a).normalize_or_zero();
    let t2 = n.cross(t1).normalize_or_zero();
    (t1, t2)
}

/// Sample a single ring orthogonal to `normal`
pub fn sample_ring(center: Vec3, normal: Vec3, radius: f32, points: usize) -> Vec<Vec3> {
    if points == 0 {
        return vec![];
    }
    let n = normal.normalize_or_zero();
    if n.length_squared() < 1e-12 {
        return vec![];
    }
    let (t1, t2) = orthonormal_basis(n);
    let mut out = Vec::with_capacity(points);
    for i in 0..points {
        let a = (i as f32) * std::f32::consts::TAU / (points as f32);
        out.push(center + radius * (a.cos() * t1 + a.sin() * t2));
    }
    out
}

/// Multi-ring seeding around velocity direction (skip tiny v)
pub fn b_rings_for_charge(
    center: Vec3,
    vel: Vec3,
    base_radius: f32,
    rings: usize,
    pts_per_ring: usize,
) -> Vec<Vec3> {
    if vel.length_squared() < 1e-10 || rings == 0 {
        return vec![];
    }
    let mut out = Vec::new();
    for r in 0..rings {
        let rad = base_radius * (1.0 + 0.5 * r as f32);
        let mut ring = sample_ring(center, vel, rad, pts_per_ring);
        // optionally rotate successive rings a bit to stagger seeds
        let rot = Quat::from_axis_angle(vel.normalize_or_zero(), 0.5 * r as f32);
        for p in &mut ring {
            *p = center + rot.mul_vec3(*p - center);
        }
        out.extend(ring);
    }
    out
}
