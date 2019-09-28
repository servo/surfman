// surfman/resources/examples/check.fs.glsl

precision highp float;

uniform vec3 uRotation;
uniform vec4 uColorA;
uniform vec4 uColorB;

in vec2 vTexCoord;

out vec4 oFragColor;

const float PI = 3.14159;

void main() {
    float t = -1.0;
    vec3 rayOrigin = vec3(0.0, 0.0, -10.0);
    vec3 rayDirection = normalize(vec3(vTexCoord, 0.0) - rayOrigin);

    vec3 originToCenter = vec3(0.0) - rayOrigin;
    float tCA = dot(originToCenter, rayDirection);
    if (tCA >= 0.0) {
        float d2 = dot(originToCenter, originToCenter) - tCA * tCA;
        if (d2 <= 1.0) {
            float tHC = sqrt(1.0 - d2);
            vec2 ts = vec2(tCA) + vec2(-tHC, tHC);
            ts = vec2(min(ts.x, ts.y), max(ts.x, ts.y));
            t = ts.x >= 0.0 ? ts.x : ts.y;
        }
    }

    if (t < 0.0) {
        oFragColor = vec4(0.0);
        return;
    }

    vec3 hitPosition = normalize(rayOrigin + rayDirection * vec3(t));

    hitPosition = mat3(vec3(cos(uRotation.y), 0.0, sin(uRotation.y)),
                       vec3(0.0, 1.0, 0.0),
                       vec3(-sin(uRotation.y), 0.0, cos(uRotation.y))) * hitPosition;
    hitPosition = mat3(vec3(1.0, 0.0, 0.0),
                       vec3(0.0, cos(uRotation.x), -sin(uRotation.x)),
                       vec3(0.0, sin(uRotation.x),  cos(uRotation.x))) * hitPosition;
    hitPosition = mat3(vec3(cos(uRotation.z), -sin(uRotation.z), 0.0),
                       vec3(sin(uRotation.z),  cos(uRotation.z), 0.0),
                       vec3(0.0, 0.0, 1.0)) * hitPosition;

    vec2 uv = vec2((1.0 + atan(hitPosition.z, hitPosition.x) / PI) * 0.5,
                   acos(hitPosition.y) / PI) * vec2(12.0);

    ivec2 on = ivec2(greaterThanEqual(mod(uv, vec2(2.0)), vec2(1.0)));
    oFragColor = ((on.x ^ on.y) > 0) ? uColorA : uColorB;
}
