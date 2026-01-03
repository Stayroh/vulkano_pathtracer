#version 460
#extension GL_EXT_ray_tracing : require

struct Payload {
	vec3 color;
	uint depth;
};

layout(location = 0) rayPayloadInEXT Payload hit_value;

void main() {
    hit_value.color = vec3(0.0);
}