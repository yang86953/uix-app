//! OpenGL ES 3.0 薄 RHI 的固定 shader ABI。

// 位置 float2 与 viewport uniform 的通用顶点阶段。
pub(super) const SOLID_VERTEX: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_pos;
uniform vec2 u_viewport;
// 根据当前 render target 选择原生 surface 或 top-left texture 行序。
uniform float u_target_y_sign;
void main() {
    vec2 ndc = (a_pos / u_viewport) * 2.0 - 1.0;
    // 让目标身份成为唯一的 Y 方向事实。
    ndc.y *= u_target_y_sign;
    gl_Position = vec4(ndc, 0.0, 1.0);
}
"#;

// 实心 mesh 的 premultiplied-alpha 颜色输出。
pub(super) const SOLID_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform vec4 u_color;
out vec4 fragColor;
void main() {
    fragColor = u_color;
}
"#;

// 位置、纹理坐标和颜色组成的 float8 采样顶点阶段。
pub(super) const TEXTURED_VERTEX: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec2 a_uv;
layout(location = 2) in vec4 a_color;
uniform vec2 u_viewport;
// 根据当前 render target 选择原生 surface 或 top-left texture 行序。
uniform float u_target_y_sign;
out vec2 v_uv;
out vec4 v_color;
void main() {
    vec2 ndc = (a_pos / u_viewport) * 2.0 - 1.0;
    // 让目标身份成为唯一的 Y 方向事实。
    ndc.y *= u_target_y_sign;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_uv = a_uv;
    v_color = a_color;
}
"#;

// BGRA8 采样 quad 的 SrcOver 颜色输出。
pub(super) const TEXTURED_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_tex;
in vec2 v_uv;
in vec4 v_color;
out vec4 fragColor;
void main() {
    vec4 sample_color = texture(u_tex, v_uv).bgra;
    fragColor = vec4(sample_color.rgb * v_color.rgb, sample_color.a * v_color.a);
}
"#;

// R8 coverage quad 的像素量化与 premultiplied-alpha 输出。
pub(super) const COVERAGE_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_tex;
in vec2 v_uv;
in vec4 v_color;
out vec4 fragColor;
void main() {
    float coverage = floor(clamp(texture(u_tex, v_uv).r, 0.0, 1.0) * 255.0 + 0.5);
    vec4 color = floor(clamp(v_color, 0.0, 1.0) * 255.0 + 0.5);
    float alpha = floor(color.a * coverage / 255.0);
    vec3 premul = floor(color.rgb * color.a / 255.0);
    vec3 rgb = floor(premul * coverage / 255.0);
    fragColor = vec4(rgb, alpha) / 255.0;
}
"#;

// RGBA8 MSDF quad 的颜色和 viewport 阶段复用 TEXTURED_VERTEX。
pub(super) const MSDF_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_tex;
uniform vec2 u_tex_size;
uniform float u_range;
in vec2 v_uv;
in vec4 v_color;
out vec4 fragColor;
float median3(float red, float green, float blue) {
    return max(min(red, green), min(max(red, green), blue));
}
void main() {
    vec3 encoded = texture(u_tex, v_uv).rgb;
    float signed_distance = median3(encoded.r, encoded.g, encoded.b) - 0.5;
    vec2 derivative = max(fwidth(v_uv), vec2(1e-6));
    vec2 unit_range = u_range / max(u_tex_size, vec2(1.0));
    vec2 screen_texture_size = 1.0 / derivative;
    float screen_pixel_range = max(0.5 * dot(unit_range, screen_texture_size), 1.0);
    float coverage = clamp(0.5 - signed_distance * screen_pixel_range, 0.0, 1.0);
    vec4 color = floor(clamp(v_color, 0.0, 1.0) * 255.0 + 0.5);
    float alpha = floor(color.a * coverage * 255.0 + 0.5);
    vec3 premul = floor(color.rgb * color.a / 255.0);
    vec3 rgb = floor(premul * coverage * 255.0 + 0.5);
    fragColor = vec4(rgb, alpha) / 255.0;
}
"#;

// 渐变和 shape/shadow 共用的单位 quad 顶点阶段。
pub(super) const RECT_VERTEX: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_pos;
uniform vec2 u_viewport;
// 根据当前 render target 选择原生 surface 或 top-left texture 行序。
uniform float u_target_y_sign;
uniform vec4 u_rect;
out vec2 v_local;
out vec2 v_rect_size;
void main() {
    vec2 pos = u_rect.xy + a_pos * u_rect.zw;
    vec2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    // 让目标身份成为唯一的 Y 方向事实。
    ndc.y *= u_target_y_sign;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_local = a_pos * u_rect.zw;
    v_rect_size = u_rect.zw;
}
"#;

// 渐变 shader 使用 96 字节 affine GradientConstants 语义。
pub(super) const GRADIENT_VERTEX: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_pos;
uniform vec2 u_viewport;
// 根据当前 render target 选择原生 surface 或 top-left texture 行序。
uniform float u_target_y_sign;
uniform vec4 u_quad_origin_edge_x;
uniform vec4 u_quad_edge_y;
out vec2 v_gradient_uv;
void main() {
    vec2 origin = u_quad_origin_edge_x.xy;
    vec2 edge_x = u_quad_origin_edge_x.zw;
    vec2 edge_y = u_quad_edge_y.xy;
    vec2 pos = origin + a_pos.x * edge_x + a_pos.y * edge_y;
    vec2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    // 让目标身份成为唯一的 Y 方向事实。
    ndc.y *= u_target_y_sign;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_gradient_uv = a_pos;
}
"#;

// 渐变 fragment 在归一化局部坐标中执行 linear/radial 采样。
pub(super) const GRADIENT_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform vec4 u_color_a;
uniform vec4 u_color_b;
uniform vec4 u_params;
in vec2 v_gradient_uv;
out vec4 fragColor;
void main() {
    float mode = u_params.x;
    if (mode < 0.5) {
        float direction = u_params.y;
        float t;
        if (direction < 0.5)
            t = v_gradient_uv.x;
        else if (direction < 1.5)
            t = v_gradient_uv.y;
        else if (direction < 2.5)
            t = (v_gradient_uv.x * u_params.z + v_gradient_uv.y * u_params.w)
                / max(u_params.z + u_params.w, 1e-6);
        else
            t = (v_gradient_uv.x * u_params.z - v_gradient_uv.y * u_params.w + u_params.w)
                / max(u_params.z + u_params.w, 1e-6);
        fragColor = mix(u_color_a, u_color_b, clamp(t, 0.0, 1.0));
    } else {
        vec2 center = vec2(0.5);
        float distance_to_center = distance(v_gradient_uv, center);
        float outer_radius = max(u_params.z, 1e-6);
        float inner_radius = u_params.y;
        if (distance_to_center > outer_radius)
            discard;
        float range = max(outer_radius - inner_radius, 1e-6);
        float t = clamp((distance_to_center - inner_radius) / range, 0.0, 1.0);
        vec4 color = mix(u_color_a, u_color_b, t);
        fragColor = vec4(color.rgb * color.a, color.a);
    }
}
"#;

// 圆角矩形与描边的 SDF fragment 阶段。
pub(super) const SHAPE_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform vec4 u_color;
uniform vec4 u_radius;
uniform float u_stroke;
in vec2 v_local;
in vec2 v_rect_size;
out vec4 fragColor;
float rounded_rect_sdf(vec2 local, vec2 size, vec4 radius) {
    vec2 half_size = size * 0.5;
    vec2 q = local - half_size;
    float corner_radius;
    if (q.x < 0.0)
        corner_radius = (q.y < 0.0) ? radius.x : radius.w;
    else
        corner_radius = (q.y < 0.0) ? radius.y : radius.z;
    vec2 d = abs(q) - half_size + corner_radius;
    float outside = length(max(d, vec2(0.0)));
    float inside = min(max(d.x, d.y), 0.0);
    return outside + inside - corner_radius;
}
void main() {
    float mask;
    if (u_stroke > 0.0) {
        vec2 shape_local = v_local - vec2(1.0);
        float half_stroke = u_stroke;
        vec2 outer_size = v_rect_size + 2.0 * half_stroke;
        vec4 outer_radius = u_radius + half_stroke;
        vec2 inner_size = max(v_rect_size - 2.0 * half_stroke, 0.0);
        vec4 inner_radius = max(u_radius - half_stroke, 0.0);
        float outer_sd = rounded_rect_sdf(shape_local, outer_size, outer_radius);
        if (inner_size.x > 0.0 && inner_size.y > 0.0) {
            float inner_sd = rounded_rect_sdf(shape_local, inner_size, inner_radius);
            mask = clamp(0.5 - outer_sd, 0.0, 1.0) * clamp(0.5 + inner_sd, 0.0, 1.0);
        } else {
            mask = clamp(0.5 - outer_sd, 0.0, 1.0);
        }
    } else {
        mask = any(greaterThan(u_radius, vec4(0.0)))
            ? clamp(0.5 - rounded_rect_sdf(v_local, v_rect_size, u_radius), 0.0, 1.0)
            : 1.0;
    }
    if (mask <= 0.0)
        discard;
    vec4 color = floor(clamp(u_color, 0.0, 1.0) * 255.0 + 0.5);
    vec3 premul = floor(color.rgb * color.a / 255.0);
    fragColor = vec4(premul * mask, color.a * mask) / 255.0;
}
"#;

// 轴对齐原生扇形的分析抗锯齿 fragment 阶段。
pub(super) const SECTOR_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform vec4 u_color;
uniform vec4 u_angles;
in vec2 v_local;
in vec2 v_rect_size;
out vec4 fragColor;
const float TAU = 6.283185307179586;
void main() {
    vec2 unit = (v_local / max(v_rect_size, vec2(0.0001)) - 0.5) * 2.0;
    float radius = length(unit);
    float radial_width = max(fwidth(radius), 0.0001);
    float radial_mask = clamp((1.0 - radius) / radial_width + 0.5, 0.0, 1.0);
    float angular_mask = 1.0;
    if (u_angles.y < TAU - 0.0001 && radius > 0.0001) {
        float angle = atan(unit.y, unit.x);
        if (angle < 0.0)
            angle += TAU;
        float delta = mod(angle - u_angles.x + TAU, TAU);
        float edge = min(delta, u_angles.y - delta);
        float angular_width = max(fwidth(angle), 0.0001);
        angular_mask = clamp(edge / angular_width + 0.5, 0.0, 1.0);
        if (delta > u_angles.y)
            angular_mask = 0.0;
    }
    float mask = radial_mask * angular_mask;
    if (mask <= 0.0)
        discard;
    vec4 color = floor(clamp(u_color, 0.0, 1.0) * 255.0 + 0.5);
    vec3 premul = floor(color.rgb * color.a / 255.0);
    fragColor = vec4(premul * mask, color.a * mask) / 255.0;
}
"#;

// 阴影 shader 的顶点阶段，使用真实仿射四边形并保留一像素抗锯齿边界。
pub(super) const SHADOW_VERTEX: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_pos;
uniform vec2 u_viewport;
// 根据当前 render target 选择原生 surface 或 top-left texture 行序。
uniform float u_target_y_sign;
uniform vec4 u_rect;
uniform vec4 u_params;
uniform vec4 u_size;
out vec2 v_local;
out vec2 v_rect_size;
void main() {
    vec2 blur = max(u_params.zw, vec2(0.0));
    vec2 body_size = max(u_size.xy, vec2(0.0001));
    vec2 expanded_size = body_size + 2.0 * blur;
    vec2 axis_x = u_rect.zw / max(expanded_size.x, 0.0001);
    vec2 axis_y = u_params.xy / max(expanded_size.y, 0.0001);
    vec2 draw_xy = u_rect.xy - axis_x - axis_y;
    vec2 draw_edge_x = u_rect.zw + axis_x * 2.0;
    vec2 draw_edge_y = u_params.xy + axis_y * 2.0;
    vec2 pos = draw_xy + a_pos.x * draw_edge_x + a_pos.y * draw_edge_y;
    vec2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    // 让目标身份成为唯一的 Y 方向事实。
    ndc.y *= u_target_y_sign;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_local = a_pos * (expanded_size + vec2(2.0));
    v_rect_size = body_size;
}
"#;

// 阴影 shader 的 SDF fragment 阶段，保持 straight-alpha blend ABI。
pub(super) const SHADOW_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform vec4 u_color;
uniform vec4 u_radius;
uniform vec4 u_params;
uniform vec4 u_size;
in vec2 v_local;
in vec2 v_rect_size;
out vec4 fragColor;
float rounded_rect_sdf(vec2 local, vec2 size, vec4 radius) {
    vec2 half_size = size * 0.5;
    vec2 q = local - half_size;
    float corner_radius;
    if (q.x < 0.0)
        corner_radius = (q.y < 0.0) ? radius.x : radius.w;
    else
        corner_radius = (q.y < 0.0) ? radius.y : radius.z;
    vec2 d = abs(q) - half_size + corner_radius;
    float outside = length(max(d, vec2(0.0)));
    float inside = min(max(d.x, d.y), 0.0);
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
void main() {
    vec2 blur = max(u_params.zw, vec2(0.0));
    float blur_radius = max(blur.x, blur.y);
    vec2 shape_local = v_local - blur - vec2(1.0);
    float signed_distance = rounded_rect_sdf(shape_local, v_rect_size, u_radius);
    float coverage;
    if (blur_radius > 0.5) {
        coverage = u_size.z > 0.5
            ? shadow_coverage_ambient(signed_distance, blur_radius)
            : shadow_coverage(signed_distance, blur_radius);
    } else {
        coverage = clamp(0.5 - signed_distance, 0.0, 1.0);
    }
    if (coverage <= 0.0)
        discard;
    fragColor = vec4(u_color.rgb, u_color.a * coverage);
}
"#;

// Blur pass 的 NDC 区域顶点阶段。
pub(super) const BLUR_VERTEX: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_pos;
uniform vec4 u_sizes;
uniform vec4 u_region;
out vec2 v_uv;
void main() {
    // blur 顶点已是 top-left NDC；纹理目标需把顶部写到 GL 第零行。
    gl_Position = vec4(a_pos.x, -a_pos.y, 0.0, 1.0);
    vec2 unit = vec2(a_pos.x * 0.5 + 0.5, 0.5 - a_pos.y * 0.5);
    v_uv = (u_region.xy + unit * u_region.zw) / u_sizes.zw;
}
"#;

// Blur pass 的 64 tap 高斯采样 fragment 阶段。
pub(super) const BLUR_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_tex;
uniform vec4 u_sizes;
uniform vec4 u_dir_taps;
uniform vec4 u_weights[16];
in vec2 v_uv;
out vec4 fragColor;
void main() {
    vec2 step_size = u_dir_taps.xy / u_sizes.zw;
    int radius = int(u_dir_taps.z);
    vec4 color = vec4(0.0);
    for (int index = 0; index < 64; ++index) {
        float weight = u_weights[index / 4][index % 4];
        if (weight <= 0.0)
            break;
        vec2 offset = step_size * float(index - radius);
        color += weight * texture(u_tex, v_uv + offset);
    }
    fragColor = color;
}
"#;
