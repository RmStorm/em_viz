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
