// surfman/resources/examples/check.fs.glsl

precision highp float;

uniform vec2 uViewportOrigin;
uniform vec3 uRotation;
uniform vec4 uColorA;
uniform vec4 uColorB;

in vec2 vTexCoord;

out vec4 oFragColor;

const float PI = 3.14159;

const float SUBSCREEN_LENGTH = 256.0;
const float RADIUS = 96.0;
const float RADIUS_SQ = RADIUS * RADIUS;
const vec3 CAMERA_POSITION = vec3(400.0, 300.0, -1000.0);

const vec3 LIGHT_POSITION = vec3(600.0, 450.0, -500.0);
const float LIGHT_AMBIENT = 1.0;
const float LIGHT_DIFFUSE = 1.0;
const float LIGHT_SPECULAR = 1.0;
const float MATERIAL_AMBIENT = 0.2;
const float MATERIAL_DIFFUSE = 0.7;
const float MATERIAL_SPECULAR = 0.1;

// Hardcoded albedo of 16.0. Works around precision issues.
float pow16(float n) {
    float n2 = n * n;
    float n4 = n2 * n2;
    float n8 = n4 * n4;
    return n8 * n8;
}

mat3 rotateZXY(vec3 theta) {
    vec3 tCos = cos(theta), tSin = sin(theta);
    vec4 zx = vec4(tSin.zz, tCos.zz) * vec4(tCos.x, tSin.xx, tCos.x);
    return mat3( tCos.y * tCos.z, -tCos.y * tSin.z,  tSin.y,
                 tSin.y * zx.z,   -tSin.y * zx.y,   -tCos.y * tSin.x,
                -tSin.y * zx.w,    tSin.y * zx.x,    tCos.y * tCos.x) +
           mat3(0.0,  0.0,  0.0,
                zx.x, zx.w, 0.0,
                zx.y, zx.z, 0.0);
}

void main() {
    vec3 rayDirection = normalize(vec3(gl_FragCoord.xy + uViewportOrigin, 0.0) - CAMERA_POSITION);

    vec3 center = vec3(uViewportOrigin, 0.0) + vec3(vec2(SUBSCREEN_LENGTH * 0.5), 0.0);
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

    // Hack: Just rotate the texture instead of rotating the sphere.
    vec3 texNormal = rotateZXY(uRotation) * normal;
    vec2 uv = vec2((1.0 + atan(texNormal.z, texNormal.x) / PI) * 0.5,
                   acos(texNormal.y) / PI) * vec2(12.0);

    ivec2 on = ivec2(greaterThanEqual(mod(uv, vec2(2.0)), vec2(1.0)));
    vec4 diffuse = ((on.x ^ on.y) > 0) ? uColorA : uColorB;

    vec3 lightDirection = normalize(LIGHT_POSITION - hitPosition);
    vec3 reflection = -reflect(lightDirection, normal);
    vec3 viewer = normalize(CAMERA_POSITION - hitPosition);

    float intensity = LIGHT_AMBIENT * MATERIAL_AMBIENT +
        MATERIAL_DIFFUSE * dot(lightDirection, normal) * LIGHT_DIFFUSE +
        MATERIAL_SPECULAR * pow16(dot(reflection, viewer)) * LIGHT_SPECULAR;

    oFragColor = vec4(intensity * diffuse.rgb, diffuse.a);
}
