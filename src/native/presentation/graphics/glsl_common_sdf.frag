// GLSL 圆角/阴影数学的唯一共享定义——Vulkan 与 OpenGL 两个 API 消费同一份。
//
// - Vulkan：compile_vulkan_rhi_shaders.py 在 #version 行之后注入本文件，
//   rhi.frag 的各 pipeline 分支不得再持有副本。
// - OpenGL：rhi_shaders.rs 的 SHAPE_FRAGMENT / SHADOW_FRAGMENT 经
//   include_str! 拼接在 `precision highp float;` 之后。
//
// 与 CPU `rasterizer/core.rs` 的 rounded_rect_sdf / shadow coverage 曲线逐式对应。

float rounded_rect_sdf(vec2 local, vec2 size, vec4 radius) {
    vec2 half_size = size * 0.5;
    vec2 q = local - half_size;
    float corner_radius;
    if (q.x < 0.0) {
        corner_radius = q.y < 0.0 ? radius.x : radius.w;
    } else {
        corner_radius = q.y < 0.0 ? radius.y : radius.z;
    }
    vec2 distance = abs(q) - half_size + corner_radius;
    float outside = length(max(distance, vec2(0.0)));
    float inside = min(max(distance.x, distance.y), 0.0);
    return outside + inside - corner_radius;
}

float shadow_coverage(float signed_distance, float blur) {
    float t = clamp((blur - signed_distance) / (2.0 * blur), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

float shadow_coverage_ambient(float signed_distance, float blur) {
    float half_blur = blur * 0.5;
    float t = clamp((half_blur - signed_distance) / (blur + half_blur), 0.0, 1.0);
    float squared = t * t;
    return squared * squared * (5.0 - 4.0 * t);
}
