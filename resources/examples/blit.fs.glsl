// resources/examples/blit.fs.glsl

precision highp float;

uniform SAMPLER_TYPE uSource;

in vec2 vTexCoord;

out vec4 oFragColor;

void main() {
    oFragColor = texture(uSource, vTexCoord);
}

