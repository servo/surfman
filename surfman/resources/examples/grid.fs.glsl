// surfman/resources/examples/grid.fs.glsl

precision highp float;

uniform vec4 uGridlineColor;
uniform vec4 uBGColor;

in vec2 vTexCoord;

out vec4 oFragColor;

const float EPSILON = 0.0001;

void main() {
    vec2 dist = fwidth(vTexCoord);
    bool on = any(lessThanEqual(mod(vTexCoord + dist * 0.5 + EPSILON, 1.0) / dist, vec2(1.0)));
    oFragColor = on ? uGridlineColor : uBGColor;
}
