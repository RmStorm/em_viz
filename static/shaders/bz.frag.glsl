#version 300 es
precision highp float;

in vec2 v_uv;
uniform sampler2D u_bTex;   // R32F: Bz in .r
uniform float u_scale;      // visual scale (e.g. 4.0)
uniform float u_alpha;      // overall opacity multiplier (0..1)
out vec4 outColor;

float softsign(float x) {
  return x / (1.0 + abs(x));
}

// Blue–white–red diverging map
vec3 bwr(float t) {
  vec3 blue = vec3(0.230, 0.299, 0.754);
  vec3 red  = vec3(0.706, 0.016, 0.150);
  return mix(blue, red, t);
}

void main() {
  // Flip Y to match solver orientation
  vec2 uv = vec2(v_uv.x, v_uv.y);
  float bz = texture(u_bTex, uv).r;

  // Map to [-1,1], compressing extremes
  float s = softsign(u_scale * bz);

  // Convert to [0,1] for color lookup
  float t = 0.5 + 0.5 * s;

  // |s| controls opacity — small fields become transparent
  float a = abs(s) * u_alpha;

  // Use a soft white→color blend so weak B is transparent
  vec3 col = mix(vec3(1.0), bwr(t), abs(s));

  outColor = vec4(col, a);
}
