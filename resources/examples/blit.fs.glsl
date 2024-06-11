// surfman/resources/examples/blit.fs.glsl

precision highp float;

#ifdef SAMPLER_RECT
uniform sampler2DRect uSource;
#else
uniform sampler2D uSource;
#endif

in vec2 vTexCoord;

out vec4 oFragColor;

void main() {
    vec2 texCoord = vTexCoord;
#ifdef SAMPLER_RECT
    texCoord /= vec2(dFdx(vTexCoord.x), dFdy(vTexCoord.y));
#endif
    oFragColor = texture(uSource, texCoord);
}
