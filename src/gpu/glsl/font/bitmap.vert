#version 450

layout(push_constant) uniform PushConstants {
    layout(offset = 0) mat4 view_proj;
    layout(offset = 64) vec2 dims_inv;
} push_constants;

layout(location = 0) in vec2 position_in;
layout(location = 1) in vec2 texcoord_in;

layout(location = 0) out vec2 texcoord_out;

void main() {
    texcoord_out = texcoord_in;

    gl_Position = push_constants.view_proj
        * vec4(position_in * push_constants.dims_inv, 0, 1);
}