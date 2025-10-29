#version 300 es
precision highp float;
in vec2 a_pos;
out vec2 v_uv;
void main() {
  v_uv = 0.5 * (a_pos + 1.0);
  gl_Position = vec4(a_pos, 0.0, 1.0);
}
