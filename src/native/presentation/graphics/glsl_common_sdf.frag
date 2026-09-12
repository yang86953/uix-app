// GLSL 圆角/阴影数学的唯一共享定义——Vulkan 与 OpenGL 两个 API 消费同一份。
//
// - Vulkan：compile_vulkan_rhi_shaders.py 在 #version 行之后注入本文件，
//   rhi.frag 的各 pipeline 分支不得再持有副本。
// - OpenGL：rhi_shaders.rs 的 SHAPE_FRAGMENT / SHADOW_FRAGMENT 经
//   include_str! 拼接在 `precision highp float;` 之后。
//
// 与 CPU `rasterizer/core.rs` 的 rounded_rect_sdf / shadow coverage 曲线逐式对应。

float rounded_rect_sdf(vec2 local, vec2 size, vec4 radius) {
    // 圆角轮廓是直角矩形与各角圆弧约束的交集：先取直边矩形距离，再对
    // 采样点落入切线方形的每个角取圆弧距离（圆外为正），取最大者。
    // 归一化只保证相邻切线方形不相交，对角方形可重叠（如 100x100 的
    // tl=br=80），重叠区须同时满足两角弧约束。`local` 为相对矩形左上角
    // 坐标，radius 分量顺序 tl/tr/br/bl。
    vec2 q = abs(local - size * 0.5) - size * 0.5;
    float combined = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0);
    if (radius.x > 0.0 && local.x < radius.x && local.y < radius.x) {
        combined = max(combined, distance(local, vec2(radius.x)) - radius.x);
    }
    if (radius.y > 0.0 && local.x > size.x - radius.y && local.y < radius.y) {
        combined = max(combined, distance(local, vec2(size.x - radius.y, radius.y)) - radius.y);
    }
    if (radius.z > 0.0 && local.x > size.x - radius.z && local.y > size.y - radius.z) {
        combined = max(combined, distance(local, size - radius.z) - radius.z);
    }
    if (radius.w > 0.0 && local.x < radius.w && local.y > size.y - radius.w) {
        combined = max(combined, distance(local, vec2(radius.w, size.y - radius.w)) - radius.w);
    }
    return combined;
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
