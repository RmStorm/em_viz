#version 300 es
precision highp float;
layout(location=0) in vec3 a_center;
uniform mat4 u_view, u_proj;
uniform float u_pointSizePx;
void main(){
  gl_Position = u_proj * u_view * vec4(a_center, 1.0);
  gl_PointSize = u_pointSizePx; // WebGL2: always on, no enable needed
}
