#version 300 es
precision highp float;

layout(location=0) in vec3 a_center;
layout(location=1) in vec3 a_tangent;
layout(location=2) in float a_side;   // -1 or +1
layout(location=3) in float a_tone;   // 0..1

uniform mat4 u_view, u_proj;
uniform vec2 u_viewport;      // (width, height) in px
uniform float u_halfWidthPx;  // half ribbon width in pixels

out float v_tone;
out float v_side;             // interpolates across strip for AA

void main() {
  // View-space basis
  vec4 vpos = u_view * vec4(a_center, 1.0);
  vec3 Tv   = normalize((u_view * vec4(a_tangent, 0.0)).xyz);
  // View-space screen normal: perpendicular to tangent in screen (x,y), facing camera -Z
  vec3 Nv   = normalize(cross(Tv, vec3(0.0, 0.0, -1.0)));

  // Project center
  vec4 clip = u_proj * vpos;

  // Convert constant pixel offset → NDC offset; keep aspect (x by width, y by height)
  vec2 ndc_per_px = 2.0 / u_viewport;     // NDC units per pixel in x/y
  vec2 dir_screen = normalize(Nv.xy + 1e-8); // screen direction from view normal
  vec2 ndc_off = a_side * u_halfWidthPx * ndc_per_px * dir_screen;

  // Offset in clip space (scale by w to move in NDC)
  clip.xy += ndc_off * clip.w;

  v_tone = clamp(a_tone, 0.0, 1.0);
  v_side = a_side; // will interpolate from -1..+1 across strip
  gl_Position = clip;
}
