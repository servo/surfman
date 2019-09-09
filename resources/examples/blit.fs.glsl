#version 330

// resources/examples/blit.fs.glsl

uniform sampler2D uSource;

in vec2 vTexCoord;

out vec4 oFragColor;

void main() {
    oFragColor = texture(uSource, vTexCoord);
}
