// resources/examples/quad.vs.glsl

precision highp float;

uniform mat2 uTransform;
uniform vec2 uTranslation;

in vec2 aPosition;

out vec2 vTexCoord;

void main() {
    vTexCoord = aPosition;
    vec2 position = uTransform * mix(vec2(-1.0), vec2(1.0), aPosition) + uTranslation;
    gl_Position = vec4(position, 0.0, 1.0);
}
