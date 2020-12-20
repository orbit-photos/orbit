#version 140
uniform sampler2D tex;
uniform mat2 rot;
uniform vec2 shift1;
uniform vec2 shift2;
in vec2 v_tex_coords;
out vec4 f_color;
void main() {
    f_color = texture(tex, rot*(v_tex_coords+shift1) + shift2);
}
