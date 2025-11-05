struct Uniforms {
  k_soft_h_max: vec4<f32>, // k, soft2, h, max_pts
  far_cut: vec4<f32>, // far_cut, 0, 0, 0
  _pad0: vec4<f32>, // padding
};
@group(0) @binding(0) var<uniform> P: Uniforms;

struct Charge { posq: vec4<f32> };    // xyz=pos, w=q
struct Charges { data: array<Charge> }

struct Seeds  { data: array<vec4<f32>> }      // xyz=seed, w=sign (+1/-1)
struct OutBuf { data: array<vec4<f32>> }      // packed vertex stream
struct DrawIndirect {
  vertex_count : u32,
  instance_count : u32,
  first_vertex : u32,
  first_instance : u32,
}
struct Counts { data: array<DrawIndirect> }   // indirect draw args per streamline

@group(0) @binding(0) var<uniform> U  : Uniforms;
@group(0) @binding(1) var<storage, read>  CH: Charges;
@group(0) @binding(2) var<storage, read>  SD: Seeds;
@group(0) @binding(3) var<storage, read_write> OUT: OutBuf;
@group(0) @binding(4) var<storage, read_write> CNT: Counts;

fn charges_len() -> u32 {
  return arrayLength(&CH.data);
}
fn seeds_len() -> u32 {
  return arrayLength(&SD.data);
}

fn sample_e(p: vec3<f32>) -> vec3<f32> {
  let k     = U.k_soft_h_max.x;
  let soft2 = U.k_soft_h_max.y;
  var e = vec3<f32>(0.0);
  let n = charges_len();
  var i: u32 = 0u;
  loop {
    if (i >= n) { break; }
    let c  = CH.data[i].posq;
    let d  = p - c.xyz;
    let r2 = dot(d, d) + soft2;
    let r  = sqrt(r2);
    e = e + (k * c.w / (r2 * r)) * d; // d / r^3
    i = i + 1u;
  }
  return e;
}

fn tone_from_mag(m: f32) -> f32 {
  return pow(m / (1.0 + m), 0.75);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let idx = gid.x;
  if (idx >= seeds_len()) { return; }

  // params
  let h       = U.k_soft_h_max.z;
  let max_pts = u32(U.k_soft_h_max.w + 0.5);
  let far_cut = U.far_cut;

  // per-stream state
  let seed = SD.data[idx];
  var p    = seed.xyz;
  var prev = p + vec3<f32>(0.0, 0.0, 1.0);
  let sign = seed.w;

  // layout math: each step emits TWO vertices; we store as 4 vec4s per step
  //   L: (center, -1) (tangent, tone)
  //   R: (center, +1) (tangent, tone)
  //
  // counts[i] is the number of vertices written for strip i
  let stride_vertices = max_pts * 2u; // vertices per streamline (max)
  let base_vertex     = idx * stride_vertices;

  var written: u32 = 0u; // vertices written so far for this strip

  var step: u32 = 0u;
  loop {
    if (step >= max_pts) { break; }

    // f(p): k1
    let e1 = sample_e(p);
    let m1 = length(e1);

    var k1: vec3<f32>;
    if (m1 > 1e-6) {
      k1 = normalize(e1) * sign;
    } else {
      k1 = vec3<f32>(0.0);
    }

    // k2
    let e2 = sample_e(p + 0.5 * h * k1);
    var k2: vec3<f32>;
    if (length(e2) > 1e-6) {
      k2 = normalize(e2) * sign;
    } else {
      k2 = vec3<f32>(0.0);
    }

    // k3
    let e3 = sample_e(p + 0.5 * h * k2);
    var k3: vec3<f32>;
    if (length(e3) > 1e-6) {
      k3 = normalize(e3) * sign;
    } else {
      k3 = vec3<f32>(0.0);
    }

    // k4
    let e4 = sample_e(p + h * k3);
    var k4: vec3<f32>;
    if (length(e4) > 1e-6) {
      k4 = normalize(e4) * sign;
    } else {
      k4 = vec3<f32>(0.0);
    }

    let dir = (k1 + 2.0 * k2 + 2.0 * k3 + k4) / 6.0;
    let p2  = p + h * dir;

    // bail on bad numbers
    // if (any(isNan(p2)) || any(isInf(p2))) { break; }

    let tone = tone_from_mag(m1);
    let tan  = normalize(p2 - prev);
    prev = p;
    p    = p2;

    // write two vertices (4 vec4s)
    let base_vec4 = (base_vertex + written) * 2u; // 2 vec4 per vertex
    // LHS
    OUT.data[base_vec4 + 0u] = vec4<f32>(p,  -1.0);
    OUT.data[base_vec4 + 1u] = vec4<f32>(tan, tone);
    // RHS
    OUT.data[base_vec4 + 2u] = vec4<f32>(p,   1.0);
    OUT.data[base_vec4 + 3u] = vec4<f32>(tan, tone);

    written = written + 2u;

    // early termination like CPU
    if (!(m1 >= 1e-6 && m1 <= 1e4)) { break; }
    // if (length(p) > far_cut) { break; }

    step = step + 1u;
  }

  let inst = select(0u, 1u, written > 0u);
  CNT.data[idx].vertex_count = written;
  CNT.data[idx].instance_count = inst;
  CNT.data[idx].first_vertex = base_vertex;
  CNT.data[idx].first_instance = 0u;
}
