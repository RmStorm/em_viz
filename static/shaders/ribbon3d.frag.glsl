#version 300 es
precision highp float;

in float v_tone;
in float v_side;
out vec4 outColor;

uniform float u_alpha;
uniform int u_palette; // 0 = Viridis (E), 1 = Plasma (B)

vec3 viridis(float t){
  const vec3 c0=vec3(0.267,0.005,0.329), c1=vec3(0.283,0.141,0.458),
             c2=vec3(0.254,0.266,0.530), c3=vec3(0.207,0.372,0.553),
             c4=vec3(0.164,0.471,0.558), c5=vec3(0.993,0.906,0.144);
  float x=clamp(t,0.0,1.0)*5.0, i=floor(x), f=x-i;
  if(i<1.0) return mix(c0,c1,f);
  else if(i<2.0) return mix(c1,c2,f);
  else if(i<3.0) return mix(c2,c3,f);
  else if(i<4.0) return mix(c3,c4,f);
  else return mix(c4,c5,f);
}

vec3 plasma(float t){
  // coarse 6-stop plasma
  const vec3 c0=vec3(0.050,0.030,0.527), c1=vec3(0.302,0.005,0.705),
             c2=vec3(0.553,0.053,0.741), c3=vec3(0.792,0.227,0.643),
             c4=vec3(0.956,0.427,0.397), c5=vec3(0.987,0.772,0.258);
  float x=clamp(t,0.0,1.0)*5.0, i=floor(x), f=x-i;
  if(i<1.0) return mix(c0,c1,f);
  else if(i<2.0) return mix(c1,c2,f);
  else if(i<3.0) return mix(c2,c3,f);
  else if(i<4.0) return mix(c3,c4,f);
  else return mix(c4,c5,f);
}

void main(){
  float w = fwidth(v_side);
  float edge = 1.0 - smoothstep(1.0 - w, 1.0, abs(v_side));
  vec3 col = (u_palette==0) ? viridis(v_tone) : plasma(v_tone);
  outColor = vec4(col, edge * u_alpha);
}
