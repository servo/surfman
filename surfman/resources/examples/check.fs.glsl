// surfman/resources/examples/check.fs.glsl

precision highp float;

uniform vec2 uViewportOrigin;
uniform vec3 uRotation;
uniform vec4 uColorA;
uniform vec4 uColorB;

in vec2 vTexCoord;

out vec4 oFragColor;

const float PI = 3.14159;
const float RADIUS = 96.0;
const float RADIUS_SQ = RADIUS * RADIUS;
const vec3 CAMERA_POSITION = vec3(400.0, 300.0, -1000.0);

void main() {
    vec3 rayDirection = normalize(vec3(gl_FragCoord.xy + uViewportOrigin, 0.0) - CAMERA_POSITION);

    vec3 center = vec3(uViewportOrigin, 0.0) + vec3(RADIUS, RADIUS, 0.0);
    vec3 originToCenter = center - CAMERA_POSITION;
    float tCA = dot(originToCenter, rayDirection);

    float t = -1.0;
    if (tCA >= 0.0) {
        float d2 = dot(originToCenter, originToCenter) - tCA * tCA;
        if (d2 <= RADIUS_SQ) {
            float tHC = sqrt(RADIUS_SQ - d2);
            vec2 ts = vec2(tCA) + vec2(-tHC, tHC);
            ts = vec2(min(ts.x, ts.y), max(ts.x, ts.y));
            t = ts.x >= 0.0 ? ts.x : ts.y;
        }
    }

    if (t < 0.0) {
        oFragColor = vec4(0.0);
        return;
    }

    vec3 hitPosition = CAMERA_POSITION + rayDirection * vec3(t);
    vec3 normal = normalize(hitPosition - center);

    normal = mat3(vec3(cos(uRotation.y), 0.0, sin(uRotation.y)),
                  vec3(0.0, 1.0, 0.0),
                  vec3(-sin(uRotation.y), 0.0, cos(uRotation.y))) * normal;
    normal = mat3(vec3(1.0, 0.0, 0.0),
                  vec3(0.0, cos(uRotation.x), -sin(uRotation.x)),
                  vec3(0.0, sin(uRotation.x),  cos(uRotation.x))) * normal;
    normal = mat3(vec3(cos(uRotation.z), -sin(uRotation.z), 0.0),
                  vec3(sin(uRotation.z),  cos(uRotation.z), 0.0),
                  vec3(0.0, 0.0, 1.0)) * normal;

    vec2 uv = vec2((1.0 + atan(normal.z, normal.x) / PI) * 0.5, acos(normal.y) / PI) * vec2(12.0);

    ivec2 on = ivec2(greaterThanEqual(mod(uv, vec2(2.0)), vec2(1.0)));
    oFragColor = ((on.x ^ on.y) > 0) ? uColorA : uColorB;
}
