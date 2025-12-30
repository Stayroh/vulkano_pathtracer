#version 460
#extension GL_EXT_ray_tracing : require

layout(location = 1) rayPayloadInEXT float shadow_hit;

void main() {
    shadow_hit = 1.0; // No hit = light is visible
}