// surfman/resources/examples/grid.fs.glsl

precision highp float;

uniform vec4 uGridlineColor;
uniform vec4 uBGColor;
uniform float uRadius;
uniform vec2 uSpherePosition;
uniform vec3 uCameraPosition;
uniform vec3 uLightPosition;

in vec2 vTexCoord;

out vec4 oFragColor;

const float DEPTH = 200.0;
const int GRID_SPACING = 50;
const float SHADOW_ATTENUATION = 0.7;

// FIXME(pcwalton): Move to an include file.
bool raytraceSphere(vec3 rayOrigin,
                    vec3 rayDirection,
                    vec3 center,
                    float radius,
                    out vec3 outHitPosition,
                    out vec3 outHitNormal) {
    vec3 originToCenter = center - rayOrigin;
    float tCA = dot(originToCenter, rayDirection);
    if (tCA < 0.0)
        return false;

    float d2 = dot(originToCenter, originToCenter) - tCA * tCA;
    float radiusSq = radius * radius;
    if (d2 > radiusSq)
        return false;

    float tHC = sqrt(radiusSq - d2);
    vec2 ts = vec2(tCA) + vec2(-tHC, tHC);
    ts = vec2(min(ts.x, ts.y), max(ts.x, ts.y));

    float t = ts.x >= 0.0 ? ts.x : ts.y;
    if (t < 0.0)
        return false;

    vec3 hitPosition = rayOrigin + rayDirection * vec3(t);
    outHitPosition = hitPosition;
    outHitNormal = normalize(hitPosition - center);
    return true;
}

void main() {
    vec3 rayOrigin = vec3(gl_FragCoord.xy, DEPTH);
    vec3 rayDirection = normalize(uLightPosition - rayOrigin);
    vec3 origin = vec3(uSpherePosition, 0.0);

    vec3 hitPosition, hitNormal;
    bool hit = raytraceSphere(rayOrigin, rayDirection, origin, uRadius, hitPosition, hitNormal);

    bool onGrid = any(equal(ivec2(gl_FragCoord.xy) % ivec2(GRID_SPACING), ivec2(0)));

    vec4 color = onGrid ? uGridlineColor : uBGColor;
    if (hit)
        color.rgb *= vec3(SHADOW_ATTENUATION);

    oFragColor = color;
}
