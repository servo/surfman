// surfman/resources/examples/check.fs.glsl

precision highp float;

in vec2 vTexCoord;

out vec4 oFragColor;

void main() {
    ivec2 on = ivec2(greaterThanEqual(mod(vTexCoord, vec2(2.0)), vec2(1.0)));
    oFragColor = vec4(vec3(float(on.x ^ on.y)), 1.0);
}
