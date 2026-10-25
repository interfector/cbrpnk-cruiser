#version 100
attribute vec3 position;
attribute vec2 texcoord;

varying vec2 uv;

uniform mat4 Projection;
uniform mat4 Model;

void main() {
    uv = texcoord;
    gl_Position = Projection * Model * vec4(position, 1.0);
}
