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
const float MATERIAL_ALBEDO = 16.0;

void main() {
    vec3 rayDirection = normalize(vec3(gl_FragCoord.xy + uViewportOrigin, 0.0) - CAMERA_POSITION);

    vec3 center = vec3(uViewportOrigin, 0.0) +
        vec3(SUBSCREEN_LENGTH, SUBSCREEN_LENGTH, 0.0) * vec3(0.5);
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

    // Hack
    vec3 texNormal = normal;
    texNormal = mat3(vec3(cos(uRotation.y), 0.0, sin(uRotation.y)),
                     vec3(0.0, 1.0, 0.0),
                     vec3(-sin(uRotation.y), 0.0, cos(uRotation.y))) * texNormal;
    texNormal = mat3(vec3(1.0, 0.0, 0.0),
                     vec3(0.0, cos(uRotation.x), -sin(uRotation.x)),
                     vec3(0.0, sin(uRotation.x),  cos(uRotation.x))) * texNormal;
    texNormal = mat3(vec3(cos(uRotation.z), -sin(uRotation.z), 0.0),
                     vec3(sin(uRotation.z),  cos(uRotation.z), 0.0),
                     vec3(0.0, 0.0, 1.0)) * texNormal;

    vec2 uv = vec2((1.0 + atan(texNormal.z, texNormal.x) / PI) * 0.5,
                   acos(texNormal.y) / PI) * vec2(12.0);

    ivec2 on = ivec2(greaterThanEqual(mod(uv, vec2(2.0)), vec2(1.0)));
    vec4 diffuse = ((on.x ^ on.y) > 0) ? uColorA : uColorB;

    vec3 lightDirection = normalize(LIGHT_POSITION - hitPosition);
    vec3 reflection = normalize(2.0 * dot(lightDirection, normal) * normal - lightDirection);
    vec3 viewer = normalize(CAMERA_POSITION - hitPosition);

    float intensity = LIGHT_AMBIENT * MATERIAL_AMBIENT +
        MATERIAL_DIFFUSE * dot(lightDirection, normal) * LIGHT_DIFFUSE +
        MATERIAL_SPECULAR * pow(dot(reflection, viewer), MATERIAL_ALBEDO) * LIGHT_SPECULAR;

    oFragColor = vec4(intensity * diffuse.rgb, diffuse.a);
}
