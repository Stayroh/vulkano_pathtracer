#version 460
#extension GL_EXT_ray_tracing : require
#extension GL_EXT_ray_tracing_position_fetch : require

struct Payload {
	vec3 color;
	uint depth;
};

layout(binding = 0, set = 0) uniform accelerationStructureEXT tlas;
layout(location = 0) rayPayloadInEXT Payload hit_value;
layout(location = 1) rayPayloadEXT float shadow_hit;
layout(push_constant) uniform PushConstants {
    uint max_ray_recursion_depth;
    float time;
} pc;
hitAttributeEXT vec2 attribs;

struct Light {
    vec3 position;
    vec3 color;
    float intensity;
};

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

    bool is_mirror = dot(normal, vec3(0.0, 1.0, 0.0)) > 0.99;

    vec3 diffuse_color = vec3(0.0);
    vec3 glossy_color = vec3(0.0);

    if (false) {
        if (hit_value.depth < pc.max_ray_recursion_depth) {

            //BLAY>TT MATHEMATICN FAILUREE
            vec3 tangent_x = normalize(cross(normal, vec3(1.0, 0.0, 0.0)));
            vec3 tangent_y = normalize(cross(tangent_x, normal));
            float scale = 50.0;
            float offset = pc.time * 10.0;
            vec3 normal_offset = tangent_x * sin(hit_position.x * scale + offset) + tangent_y * cos(hit_position.z * scale + offset);
            vec3 modified_normal = normalize(normal + normal_offset * 0.005);

            //Reflexione sukaa blyatt
            vec3 reflection_dir = gl_WorldRayDirectionEXT - 2.0 * dot(gl_WorldRayDirectionEXT, modified_normal) * modified_normal;
            vec3 reflection_ray_origin = hit_position + normal * 0.00001;

            hit_value.color = vec3(0.0);
            hit_value.depth += 1;

            traceRayEXT(tlas, gl_RayFlagsOpaqueEXT, 0xff, 0, 0, 0, reflection_ray_origin, 0.00001, reflection_dir, 10000.0, 0);

            glossy_color = hit_value.color;
        }
    } 
    if (true) {
        Light lights[2] = Light[](
            Light(vec3(3.0, 1.0, 0.0), vec3(1.0, 0.1, 0.1), 5.0),
            Light(vec3(-3.0, 1.0, 0.0), vec3(0.1, 0.1, 1.0), 5.0)
        );

        vec3 total_light = vec3(0.0);

        vec3 global_light_offset = vec3(0.0);

        float sin_t = sin(pc.time);
        float cos_t = cos(pc.time);

        vec3 x_head = vec3(cos_t, 0.0, sin_t);
        vec3 y_head = vec3(0.0, 1.0, 0.0);
        vec3 z_head = vec3(-sin_t, 0.0, cos_t);

        for (int i = 0; i < 2; i++) {
            Light light = lights[i];
            vec3 light_position = light.position;
            light_position = x_head * light_position.x + y_head * light_position.y + z_head * light_position.z + global_light_offset;


            vec3 light_color = light.color * light.intensity;

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

            total_light += combined_light * light_color;
        }

        vec3 base_color = vec3(1.0);


        diffuse_color = total_light * base_color * 1.0;
    }

    hit_value.color = diffuse_color + glossy_color;
}