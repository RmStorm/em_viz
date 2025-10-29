#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_exTex;  // RG32F: Ex in .r, Ey in .g
out vec4 outColor;

vec3 viridis(float t){
  const vec3 c0 = vec3(0.267, 0.005, 0.329);
  const vec3 c1 = vec3(0.283, 0.141, 0.458);
  const vec3 c2 = vec3(0.254, 0.266, 0.530);
  const vec3 c3 = vec3(0.207, 0.372, 0.553);
  const vec3 c4 = vec3(0.164, 0.471, 0.558);
  const vec3 c5 = vec3(0.993, 0.906, 0.144);
  float x = clamp(t, 0.0, 1.0) * 5.0;
  float i = floor(x), f = x - i;
  if (i < 1.0) return mix(c0, c1, f);
  else if (i < 2.0) return mix(c1, c2, f);
  else if (i < 3.0) return mix(c2, c3, f);
  else if (i < 4.0) return mix(c3, c4, f);
  else return mix(c4, c5, f);
}

void main() {
  vec2 E = texture(u_exTex, v_uv).rg;
  float m = length(E);
  float t = pow(m / (1.0 + m), 0.75);
  outColor = vec4(viridis(t), 1.0);
}
// #version 300 es
// precision mediump float;
// in vec2 uv;
// uniform sampler2D exTex;
// out vec4 fragColor;

// vec3 viridis(float t){
//   const vec3 c0 = vec3(0.267, 0.005, 0.329);
//   const vec3 c1 = vec3(0.283, 0.141, 0.458);
//   const vec3 c2 = vec3(0.254, 0.266, 0.530);
//   const vec3 c3 = vec3(0.207, 0.372, 0.553);
//   const vec3 c4 = vec3(0.164, 0.471, 0.558);
//   const vec3 c5 = vec3(0.993, 0.906, 0.144);
//   float x = clamp(t, 0.0, 1.0) * 5.0;
//   float i = floor(x), f = x - i;
//   if (i < 1.0) return mix(c0, c1, f);
//   else if (i < 2.0) return mix(c1, c2, f);
//   else if (i < 3.0) return mix(c2, c3, f);
//   else if (i < 4.0) return mix(c3, c4, f);
//   else return mix(c4, c5, f);
// }

// void main(){
//   vec2 E = texture(exTex, uv).rg;    // fetch Ex,Ey
//   float m = length(E);               // |E|
//   float t = m / (1.0 + m);           // same tone-map
//   t = pow(t, 0.75);                  // display curve lift
//   fragColor = vec4(viridis(t), 1.0);
// }
