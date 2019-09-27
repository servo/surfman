// surfman/resources/examples/quad.vs.glsl

precision highp float;

uniform mat2 uTransform;
uniform vec2 uTranslation;
uniform mat2 uTexTransform;
uniform vec2 uTexTranslation;

in vec2 aPosition;

out vec2 vTexCoord;

void main() {
    vTexCoord = uTexTransform * aPosition + uTexTranslation;
    vec2 position = uTransform * aPosition + uTranslation;
    gl_Position = vec4(position, 0.0, 1.0);
}
