struct U {
  view: mat4x4<f32>,
  proj: mat4x4<f32>,
  vp_hw_alpha: vec4<f32>,  // x = viewport.w, y = viewport.h, z = halfWidthPx, w = alpha
};
@group(0) @binding(0) var<uniform> UBO: U;

struct VIn {
  @location(0) center: vec3<f32>,
  @location(1) tangent: vec3<f32>,
  @location(2) side: f32,
  @location(3) tone: f32,
};

struct VOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) side: f32,
  @location(1) tone: f32,
};

@vertex
fn vs(v: VIn) -> VOut {
  var o: VOut;

  // World -> view/clip for center
  let pos_world = vec4<f32>(v.center, 1.0);
  let vpos      = UBO.view * pos_world;
  let clipc     = UBO.proj * vpos;

  // Take a tiny step along the tangent in world space
  let eps      = 0.01;
  let p2_world = vec4<f32>(v.center + eps * v.tangent, 1.0);
  let vpos2    = UBO.view * p2_world;
  let clip2    = UBO.proj * vpos2;

  // Convert both to NDC
  let ndc0 = clipc.xyz / clipc.w;
  let ndc1 = clip2.xyz / clip2.w;

  // Screen-space line direction (in NDC)
  var dir_line = ndc1.xy - ndc0.xy;
  if (length(dir_line) < 1e-6) {
      dir_line = vec2<f32>(0.0, 1.0);
  } else {
      dir_line = normalize(dir_line);
  }

  // Perpendicular in screen space
  let dir_screen = vec2<f32>(-dir_line.y, dir_line.x);

  // Pixels -> NDC
  let ndc_per_px = vec2<f32>(2.0) / UBO.vp_hw_alpha.xy;
  let ndc_off = v.side * UBO.vp_hw_alpha.z * ndc_per_px * dir_screen;

  // Apply offset in NDC and go back to clip
  let ndc = vec3<f32>(
      ndc0.x + ndc_off.x,
      ndc0.y + ndc_off.y,
      ndc0.z       // leave depth alone
  );

  let clip = vec4<f32>(ndc * clipc.w, clipc.w);

  o.pos  = clip;
  o.side = v.side;
  o.tone = clamp(v.tone, 0.0, 1.0);
  return o;
}


fn viridis(t: f32) -> vec3<f32> {
  let c0=vec3<f32>(0.267,0.005,0.329);
  let c1=vec3<f32>(0.283,0.141,0.458);
  let c2=vec3<f32>(0.254,0.266,0.530);
  let c3=vec3<f32>(0.207,0.372,0.553);
  let c4=vec3<f32>(0.164,0.471,0.558);
  let c5=vec3<f32>(0.993,0.906,0.144);
  let x = clamp(t,0.0,1.0)*5.0;
  let i = floor(x);
  let f = fract(x);
  return select(
    select(select(select(mix(c0,c1,f), mix(c1,c2,f), i<2.0), mix(c2,c3,f), i<3.0), mix(c3,c4,f), i<4.0),
    mix(c4,c5,f),
    i>=4.0
  );
}

@fragment
fn fs(@location(0) side: f32, @location(1) tone: f32) -> @location(0) vec4<f32> {
  let w = fwidth(side);
  let edge = 1.0 - smoothstep(1.0 - w, 1.0, abs(side));
  let col = viridis(tone);
  return vec4(col, edge * UBO.vp_hw_alpha.w);
}
