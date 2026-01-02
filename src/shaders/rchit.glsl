#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_EXT_ray_tracing_position_fetch : require

layout(binding = 0, set = 0) uniform accelerationStructureEXT tlas;
layout(location = 0) rayPayloadInEXT vec3 hit_value;
layout(location = 1) rayPayloadEXT float shadow_hit;
hitAttributeEXT vec2 attribs;

void main() {
    vec3 pos0 = gl_HitTriangleVertexPositionsEXT[0];
    vec3 pos1 = gl_HitTriangleVertexPositionsEXT[1];
    vec3 pos2 = gl_HitTriangleVertexPositionsEXT[2];

    vec3 geometricNormal = normalize(cross(pos1 - pos0, pos2 - pos0));
    bool isFrontFacing = (gl_HitKindEXT == gl_HitKindFrontFacingTriangleEXT);
    
    // Flip normal if we hit from the back
    if (!isFrontFacing) {
        geometricNormal = -geometricNormal;
    }
    vec3 normal = normalize(mat3(gl_ObjectToWorldEXT) * geometricNormal);

    vec3 barycentrics = vec3(1.0 - attribs.x - attribs.y, attribs.x, attribs.y);

    vec3 object_hit_position = pos0 * barycentrics.x + pos1 * barycentrics.y + pos2 * barycentrics.z;
    vec3 hit_position = (gl_ObjectToWorldEXT * vec4(object_hit_position, 1.0)).xyz;

    vec3 light_position = vec3(0.5, 2.0, 1.0);
    vec3 light_color = vec3(1.0, 1.0, 1.0) * 2.0;

    vec3 to_light = light_position - hit_position;
    vec3 to_light_dir = normalize(to_light);
    float to_light_distance = length(to_light);

    float light_influence = max(0.0, dot(to_light_dir, normal));
    

    shadow_hit = 0.0;

    vec3 shadow_ray_origin = hit_position + normal * 0.00001;

    if (light_influence > 0.0) {
        traceRayEXT(
            tlas,
            gl_RayFlagsTerminateOnFirstHitEXT | gl_RayFlagsSkipClosestHitShaderEXT,
            0xFF,
            0,
            0,
            1,
            shadow_ray_origin,
            0.00001,
            to_light_dir,
            to_light_distance,
            1
        );
    }
    

    float attenuation = 1.0 / pow(to_light_distance, 1.0);
    float combined_light = light_influence * attenuation * shadow_hit * 0.9 + 0.1;


    //Reflexione sukaa blyatt

    vec3 reflection_dir = gl_WorldRayDirectionEXT - 2.0 * dot(gl_WorldRayDirectionEXT, normal) * normal;
    
    hit_value = vec3(0.0);

    //traceRayEXT(tlas, gl_RayFlagsOpaqueEXT, 0xff, 0, 0, 0, shadow_ray_origin, 0.00001, reflection_dir, 10000.0, 0);


    vec3 base_color = vec3(1.0);

    hit_value = light_color * combined_light * base_color * 1.0 + hit_value * base_color * 0.0;
}