#version 460
#extension GL_EXT_ray_tracing : enable
#extension GL_EXT_shader_image_load_formatted : enable

layout(binding = 0, set = 0) uniform accelerationStructureEXT tlas;
layout(binding = 1, set = 0) uniform image2D image;
layout(binding = 2, set = 0) uniform CameraProperties 
{
	mat4 view_inverse;
	mat4 proj_inverse;
} cam;

layout(location = 0) rayPayloadEXT vec3 hit_value;

void main() 
{


	
	const vec2 pixelCenter = vec2(gl_LaunchIDEXT.xy) + vec2(0.5);
	const vec2 inUV = pixelCenter/vec2(gl_LaunchSizeEXT.xy);


	vec2 d = inUV * 2.0 - 1.0;

	vec4 origin = cam.view_inverse * vec4(0,0,0,1);
	vec4 target = cam.proj_inverse * vec4(d.x, -d.y, 1, 1) ;
	vec4 direction = cam.view_inverse*vec4(normalize(target.xyz), 0) ;

	float t_min = 0.001;
	float t_max = 10000.0;

    hit_value = vec3(0.0);

    traceRayEXT(tlas, gl_RayFlagsOpaqueEXT, 0xff, 0, 0, 0, origin.xyz, t_min, direction.xyz, t_max, 0);

	imageStore(image, ivec2(gl_LaunchIDEXT.xy), vec4(hit_value, 0.0));
}