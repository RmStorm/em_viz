#version 300 es
precision highp float;
out vec4 outColor;
void main(){
  vec2 p = gl_PointCoord*2.0 - 1.0;
  float r2 = dot(p,p);
  if(r2 > 1.0) discard;
  float z = sqrt(max(0.0, 1.0 - r2));
  vec3 n = normalize(vec3(p, z));
  float l = dot(n, normalize(vec3(0.4,0.6,1.0)))*0.5 + 0.5;
  outColor = vec4(vec3(0.95)*l, 1.0);
}
