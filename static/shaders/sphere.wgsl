struct Uniforms {
  view: mat4x4<f32>,
  proj: mat4x4<f32>,
  view_px: vec4<f32>,      // xy = viewport (px), z = point_size_px, w = unused
};

@group(0) @binding(0) var<uniform> U: Uniforms;

struct VsIn {
  @location(0) quad: vec2<f32>,   // -0.5..+0.5
  @location(2) center: vec3<f32>,
};

struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) quad: vec2<f32>,
};

@vertex
fn vs(in_: VsIn) -> VsOut {
  var o: VsOut;

  let vpos  = U.view * vec4(in_.center, 1.0);
  let vclip = U.proj * vpos;

  let ndc_per_px = vec2<f32>(2.0) / U.view_px.xy; // 2px / viewport
  let px = U.view_px.z;                            // point size in px
  let px_off_ndc = in_.quad * px * ndc_per_px;

  let clip = vec4<f32>(
    vclip.x + px_off_ndc.x * vclip.w,
    vclip.y + px_off_ndc.y * vclip.w,
    vclip.z,
    vclip.w
  );

  o.pos = clip;
  o.quad = in_.quad * 2.0; // -1..+1 for impostor
  return o;
}

@fragment
fn fs(@location(0) quad: vec2<f32>) -> @location(0) vec4<f32> {
  let r2 = dot(quad, quad);
  if (r2 > 1.0) {
    discard;
  }

  let z = sqrt(max(0.0, 1.0 - r2));
  let n = normalize(vec3(quad, z));
  let l = normalize(vec3(0.4, 0.6, 1.0));
  let diff = dot(n, l) * 0.5 + 0.5;
  let col = vec3(0.95) * diff;
  return vec4(col, 1.0);
}
