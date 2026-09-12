//! OpenGL ES 3.0 薄 RHI 的固定 shader ABI。

// 位置 float2 与 viewport uniform 的通用顶点阶段。
pub(super) const SOLID_VERTEX: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_pos;
layout(location = 1) in float a_coverage;
uniform vec2 u_viewport;
// 根据当前 render target 选择原生 surface 或 top-left texture 行序。
uniform float u_target_y_sign;
out float v_coverage;
void main() {
    vec2 ndc = (a_pos / u_viewport) * 2.0 - 1.0;
    // 让目标身份成为唯一的 Y 方向事实。
    ndc.y *= u_target_y_sign;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_coverage = a_coverage;
}
"#;

// 实心 mesh 直出共享契约声明的 straight-alpha 颜色。
pub(super) const SOLID_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform vec4 u_color;
in float v_coverage;
out vec4 fragColor;
void main() {
    fragColor = vec4(u_color.rgb, u_color.a * clamp(v_coverage, 0.0, 1.0));
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

// 已由 Adapter 规范化为逻辑 RGBA 的颜色纹理 SrcOver 输出。
pub(super) const TEXTURED_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_tex;
uniform vec2 u_viewport;
uniform float u_corner_radius;
// 客户端阴影环逐角补画峰值：缺口按 fragment 所在角取值。
uniform float u_shadow_alpha_tl;
uniform float u_shadow_alpha_tr;
uniform float u_shadow_alpha_bl;
uniform float u_shadow_alpha_br;
// 客户端阴影环补画的物理衰减距离，同时也是补画启用事实。
uniform float u_shadow_range;
in vec2 v_uv;
in vec4 v_color;
out vec4 fragColor;
void main() {
    vec4 sample_color = texture(u_tex, v_uv);
    float coverage = 1.0;
    float fill_alpha = 0.0;
    if (u_corner_radius > 0.0) {
        vec2 half_size = u_viewport * 0.5;
        vec2 centered = abs(gl_FragCoord.xy - half_size);
        vec2 distance = centered - half_size + u_corner_radius;
        float signed_distance = length(max(distance, vec2(0.0)))
            + min(max(distance.x, distance.y), 0.0) - u_corner_radius;
        coverage = clamp(0.5 - signed_distance / max(fwidth(signed_distance), 0.0001), 0.0, 1.0);
        if (u_shadow_range > 0.0) {
            // 圆角缺口用与客户端阴影环一致的二次衰减延伸外圈阴影；
            // v_uv 保持内容左上原点约定，按所在象限取逐角峰值。
            float horizontal = v_uv.y < 0.5
                ? mix(u_shadow_alpha_tl, u_shadow_alpha_tr, step(0.5, v_uv.x))
                : mix(u_shadow_alpha_bl, u_shadow_alpha_br, step(0.5, v_uv.x));
            float falloff = clamp(1.0 - signed_distance / u_shadow_range, 0.0, 1.0);
            fill_alpha = horizontal * falloff * falloff;
        }
    }
    vec4 color = vec4(sample_color.rgb * v_color.rgb, sample_color.a * v_color.a);
    // 缺口阴影是预乘黑色，只向输出 alpha 贡献补画事实。
    fragColor = vec4(color.rgb * coverage, color.a * coverage + fill_alpha * (1.0 - coverage));
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
    // 共享 R8Unorm 与 nearest 契约已保证 coverage 位于单位域。
    float coverage = floor(texture(u_tex, v_uv).r * 255.0 + 0.5);
    // 共享 FramePlan 顶点契约已保证 tint 位于单位域。
    vec4 color = floor(v_color * 255.0 + 0.5);
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
    // 先把解析覆盖率量化为字节，固定与 D3D11 相同的舍入顺序。
    float coverage_byte = floor(coverage * 255.0 + 0.5);
    // 共享 FramePlan 顶点契约已保证 MSDF tint 位于单位域。
    vec4 color = floor(v_color * 255.0 + 0.5);
    // 颜色已经位于字节域，alpha 只按量化覆盖率缩放一次。
    float alpha = floor(color.a * coverage_byte / 255.0);
    vec3 premul = floor(color.rgb * color.a / 255.0);
    // 预乘颜色沿用同一字节覆盖率，禁止再次乘入 255。
    vec3 rgb = floor(premul * coverage_byte / 255.0);
    fragColor = vec4(rgb, alpha) / 255.0;
}
"#;

// Shape 独占的单位 quad 顶点阶段，只消费共享层冻结的绘制边界。
pub(super) const SHAPE_VERTEX: &str = r#"#version 300 es
// 使用高精度浮点保证物理像素边界稳定。
precision highp float;
// 读取共享单位四边形的顶点坐标。
layout(location = 0) in vec2 a_pos;
// 读取当前绘制目标的物理尺寸。
uniform vec2 u_viewport;
// 根据当前 render target 选择原生 surface 或 top-left texture 行序。
uniform float u_target_y_sign;
// 读取未外扩 Shape 的原始矩形。
uniform vec4 u_rect;
// 共享 GPU Raster Module 已经计算完成的实际绘制边界。
uniform vec4 u_draw_rect;
// 输出实际绘制边界内的局部坐标。
out vec2 v_local;
// 输出原始 Shape 尺寸供片元 SDF 使用。
out vec2 v_rect_size;
// 把共享绘制边界转换为目标裁剪空间。
void main() {
    // 平台 shader 不再拥有描边外扩策略。
    vec2 pos = u_draw_rect.xy + a_pos * u_draw_rect.zw;
    // 把物理像素坐标转换为 NDC。
    vec2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    // 让目标身份成为唯一的 Y 方向事实。
    ndc.y *= u_target_y_sign;
    // 写入当前顶点的裁剪空间位置。
    gl_Position = vec4(ndc, 0.0, 1.0);
    // 局部坐标覆盖共享 draw rect，供片元阶段恢复同心双 SDF。
    v_local = a_pos * u_draw_rect.zw;
    // 保留原始 Shape 尺寸，避免片元阶段重新推导。
    v_rect_size = u_rect.zw;
// 结束 Shape 顶点入口。
}
"#;

// 扇形独占不带描边外扩语义的单位 quad 顶点阶段。
pub(super) const SECTOR_VERTEX: &str = r#"#version 300 es
// 使用高精度浮点保证扇形边界稳定。
precision highp float;
// 读取共享单位四边形的顶点坐标。
layout(location = 0) in vec2 a_pos;
// 读取当前绘制目标的物理尺寸。
uniform vec2 u_viewport;
// 根据当前 render target 选择原生 surface 或 top-left texture 行序。
uniform float u_target_y_sign;
// 读取扇形使用的原始矩形。
uniform vec4 u_rect;
// 输出原始矩形内的局部坐标。
out vec2 v_local;
// 输出原始矩形尺寸供片元裁剪使用。
out vec2 v_rect_size;
// 把扇形原始矩形转换为目标裁剪空间。
void main() {
    // 扇形只覆盖调用方提供的原始矩形。
    vec2 pos = u_rect.xy + a_pos * u_rect.zw;
    // 把物理像素坐标转换为 NDC。
    vec2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    // 让目标身份成为唯一的 Y 方向事实。
    ndc.y *= u_target_y_sign;
    // 写入当前顶点的裁剪空间位置。
    gl_Position = vec4(ndc, 0.0, 1.0);
    // 保留扇形原始矩形内的局部坐标。
    v_local = a_pos * u_rect.zw;
    // 保留扇形原始矩形尺寸。
    v_rect_size = u_rect.zw;
// 结束扇形顶点入口。
}
"#;

// 解析线段使用两端点外接矩形生成单位 quad，并把物理坐标交给片元阶段。
pub(super) const LINE_VERTEX: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_pos;
uniform vec2 u_viewport;
uniform float u_target_y_sign;
uniform vec4 u_points;
uniform vec4 u_params;
out vec2 v_position;
void main() {
    float fringe = max(u_params.x * 0.5, 0.0) + 1.5;
    vec2 lower = min(u_points.xy, u_points.zw) - vec2(fringe);
    vec2 upper = max(u_points.xy, u_points.zw) + vec2(fringe);
    vec2 position = mix(lower, upper, a_pos);
    vec2 ndc = (position / u_viewport) * 2.0 - 1.0;
    ndc.y *= u_target_y_sign;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_position = position;
}
"#;

// 渐变 shader 使用 144 字节 affine GradientConstants 语义（S4 圆角掩码扩展）。
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

// 渐变 fragment 在归一化局部坐标中执行 linear/radial 采样与 S4 圆角掩码。
pub(super) const GRADIENT_FRAGMENT: &str = concat!(
    r#"#version 300 es
precision highp float;
uniform vec4 u_color_a;
uniform vec4 u_color_b;
uniform vec4 u_params;
// S4 圆角掩码：四角半径、quad 宽高+掩码单位起点、掩码单位宽高。
uniform vec4 u_mask_radius;
uniform vec4 u_quad_mask;
uniform vec4 u_mask_size;
in vec2 v_gradient_uv;
out vec4 fragColor;
"#,
    include_str!("../../glsl_common_sdf.frag"),
    r#"
void main() {
    // 圆角掩码与共享 SDF 同式；半径全零保持 1.0 覆盖。
    float coverage = 1.0;
    if (any(greaterThan(u_mask_radius, vec4(0.0)))) {
        vec2 quad_size = u_quad_mask.xy;
        vec2 mask_origin = u_quad_mask.zw;
        vec2 mask_px = (v_gradient_uv - mask_origin) * quad_size;
        vec2 mask_wh = u_mask_size.xy * quad_size;
        float sdf = rounded_rect_sdf(mask_px, mask_wh, u_mask_radius);
        coverage = clamp(0.5 - sdf, 0.0, 1.0);
        if (coverage <= 0.0)
            discard;
    }
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
        fragColor = mix(u_color_a, u_color_b, clamp(t, 0.0, 1.0)) * coverage;
    } else {
        vec2 center = vec2(0.5);
        float distance_to_center = distance(v_gradient_uv, center);
        // FramePlan 已验证径向外半径为严格正值，OpenGL 只机械消费共享字段。
        float outer_radius = u_params.z;
        float inner_radius = u_params.y;
        if (distance_to_center > outer_radius)
            discard;
        float range = max(outer_radius - inner_radius, 1e-6);
        float t = clamp((distance_to_center - inner_radius) / range, 0.0, 1.0);
        vec4 color = mix(u_color_a, u_color_b, t);
        // GradientRect 契约统一输出 straight-alpha，避免混合器再次乘源 alpha。
        fragColor = color * coverage;
    }
}
"#,
);

// 圆角矩形与描边的 SDF fragment 阶段。
// rounded_rect_sdf 与 shadow coverage 由 glsl_common_sdf.frag 唯一提供。
pub(super) const SHAPE_FRAGMENT: &str = concat!(
    r#"#version 300 es
precision highp float;
uniform vec4 u_color;
uniform vec4 u_radius;
// x 保存半描边宽度，y 保存 fringe，z/w 保存共享 outer/inner 原点偏移。
uniform vec4 u_stroke;
in vec2 v_local;
in vec2 v_rect_size;
out vec4 fragColor;
"#,
    include_str!("../../glsl_common_sdf.frag"),
    r#"
void main() {
    float mask;
    // 描边判定只读取共享常量中的半线宽。
    if (u_stroke.x > 0.0) {
        // 使用共享 outer 偏移把 draw rect 局部坐标转换到 outer SDF 原点。
        vec2 outer_local = v_local - vec2(u_stroke.z);
        // 使用共享 inner 偏移保持 inner 与 outer SDF 严格同心。
        vec2 inner_local = v_local - vec2(u_stroke.w);
        // 保存片元阶段使用的半描边宽度。
        float half_stroke = u_stroke.x;
        vec2 outer_size = v_rect_size + 2.0 * half_stroke;
        vec4 outer_radius = u_radius + half_stroke;
        vec2 inner_size = max(v_rect_size - 2.0 * half_stroke, 0.0);
        vec4 inner_radius = max(u_radius - half_stroke, 0.0);
        // 计算外边界有符号距离。
        float outer_sd = rounded_rect_sdf(outer_local, outer_size, outer_radius);
        if (inner_size.x > 0.0 && inner_size.y > 0.0) {
            // 以内缩后的独立局部原点计算同心内边界距离。
            float inner_sd = rounded_rect_sdf(inner_local, inner_size, inner_radius);
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
"#,
);

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

// 解析胶囊距离场为任意方向线段提供单一覆盖率抗锯齿语义。
pub(super) const LINE_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform vec4 u_points;
uniform vec4 u_color;
uniform vec4 u_params;
in vec2 v_position;
out vec4 fragColor;
void main() {
    vec2 start = u_points.xy;
    vec2 segment = u_points.zw - start;
    float length_squared = max(dot(segment, segment), 0.000001);
    float along = clamp(dot(v_position - start, segment) / length_squared, 0.0, 1.0);
    float distance_to_line = length(v_position - (start + along * segment)) - u_params.x * 0.5;
    float derivative = max(fwidth(distance_to_line), 0.0001);
    float coverage = clamp(0.5 - distance_to_line / derivative, 0.0, 1.0);
    if (coverage <= 0.0)
        discard;
    vec4 color = floor(clamp(u_color, 0.0, 1.0) * 255.0 + 0.5);
    vec3 premul = floor(color.rgb * color.a / 255.0);
    fragColor = vec4(premul * coverage, color.a * coverage) / 255.0;
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
// rounded_rect_sdf 与 shadow coverage 由 glsl_common_sdf.frag 唯一提供。
pub(super) const SHADOW_FRAGMENT: &str = concat!(
    r#"#version 300 es
precision highp float;
uniform vec4 u_color;
uniform vec4 u_radius;
uniform vec4 u_params;
uniform vec4 u_size;
in vec2 v_local;
in vec2 v_rect_size;
out vec4 fragColor;
"#,
    include_str!("../../glsl_common_sdf.frag"),
    r#"
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
"#,
);

// Blur pass 只透传共享几何已经冻结的 NDC position 与绝对 source UV。
pub(super) const BLUR_VERTEX: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec2 a_uv;
out vec2 v_uv;
void main() {
    // blur 顶点已是 top-left NDC；纹理目标需把顶部写到 GL 第零行。
    gl_Position = vec4(a_pos.x, -a_pos.y, 0.0, 1.0);
    v_uv = a_uv;
}
"#;

// Blur pass 的 64 tap 高斯采样 fragment 阶段。
pub(super) const BLUR_FRAGMENT: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_tex;
uniform vec4 u_uv_bounds;
uniform vec4 u_step_taps;
uniform vec4 u_weights[16];
in vec2 v_uv;
out vec4 fragColor;
void main() {
    vec2 step_size = u_step_taps.xy;
    int radius = int(u_step_taps.z);
    vec4 color = vec4(0.0);
    for (int index = 0; index < 64; ++index) {
        float weight = u_weights[index / 4][index % 4];
        if (weight <= 0.0)
            break;
        vec2 offset = step_size * float(index - radius);
        vec2 sample_uv = clamp(v_uv + offset, u_uv_bounds.xy, u_uv_bounds.zw);
        color += weight * texture(u_tex, sample_uv);
    }
    fragColor = color;
}
"#;
