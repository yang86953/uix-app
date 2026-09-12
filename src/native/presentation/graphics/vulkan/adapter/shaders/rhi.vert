#version 450

// Vulkan 薄 RHI 顶点阶段的唯一源码；构建产物按共享 PipelineKind 选择宏变体。

#if defined(UIX_MESH)
layout(set = 0, binding = 0, std140) uniform MeshUniforms {
    vec2 viewport;
    vec2 _pad0;
    vec4 color;
} u;
layout(location = 0) in vec2 a_pos;
layout(location = 1) in float a_coverage;
layout(location = 0) out float v_coverage;

void main() {
    vec2 ndc = (a_pos / u.viewport) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_coverage = a_coverage;
}

#elif defined(UIX_SAMPLED)
layout(set = 0, binding = 0, std140) uniform SampledUniforms {
    vec2 viewport;
    vec2 _pad0;
    vec4 surface_clip;
} u;
layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec2 a_uv;
layout(location = 2) in vec4 a_color;
layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec4 v_color;

void main() {
    vec2 ndc = (a_pos / u.viewport) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_uv = a_uv;
    v_color = a_color;
}

#elif defined(UIX_GRADIENT)
layout(set = 0, binding = 0, std140) uniform GradientUniforms {
    vec2 viewport;
    vec2 _pad0;
    vec4 origin_edge_x;
    vec4 edge_y;
    vec4 color_a;
    vec4 color_b;
    vec4 params;
    // S4 圆角掩码：四角半径（tl/tr/br/bl 逻辑像素）、quad 逻辑宽高与
    // 掩码单位矩形起点、掩码单位矩形宽高；半径全零关闭掩码。
    vec4 mask_radius;
    vec4 quad_mask;
    vec4 mask_size;
} u;
layout(location = 0) in vec2 a_pos;
layout(location = 0) out vec2 v_local;

void main() {
    vec2 position = u.origin_edge_x.xy
        + a_pos.x * u.origin_edge_x.zw
        + a_pos.y * u.edge_y.xy;
    vec2 ndc = (position / u.viewport) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_local = a_pos;
}

#elif defined(UIX_SHAPE)
layout(set = 0, binding = 0, std140) uniform ShapeUniforms {
    vec2 viewport;
    vec2 _pad0;
    vec4 rect;
    vec4 color;
    vec4 radius;
    vec4 stroke;
    vec4 draw_rect;
} u;
layout(location = 0) in vec2 a_pos;
layout(location = 0) out vec2 v_local;
layout(location = 1) out vec2 v_rect_size;

void main() {
    vec2 position = u.draw_rect.xy + a_pos * u.draw_rect.zw;
    vec2 ndc = (position / u.viewport) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_local = a_pos * u.draw_rect.zw;
    v_rect_size = u.rect.zw;
}

#elif defined(UIX_SHADOW)
layout(set = 0, binding = 0, std140) uniform ShadowUniforms {
    vec2 viewport;
    vec2 _pad0;
    vec4 rect;
    vec4 color;
    vec4 radius;
    vec4 params;
    vec4 size;
} u;
layout(location = 0) in vec2 a_pos;
layout(location = 0) out vec2 v_local;
layout(location = 1) out vec2 v_rect_size;

void main() {
    vec2 blur = max(u.params.zw, vec2(0.0));
    vec2 body_size = max(u.size.xy, vec2(0.0001));
    vec2 expanded_size = body_size + 2.0 * blur;
    vec2 axis_x = u.rect.zw / max(expanded_size.x, 0.0001);
    vec2 axis_y = u.params.xy / max(expanded_size.y, 0.0001);
    vec2 draw_origin = u.rect.xy - axis_x - axis_y;
    vec2 draw_edge_x = u.rect.zw + axis_x * 2.0;
    vec2 draw_edge_y = u.params.xy + axis_y * 2.0;
    vec2 position = draw_origin + a_pos.x * draw_edge_x + a_pos.y * draw_edge_y;
    vec2 ndc = (position / u.viewport) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_local = a_pos * (expanded_size + vec2(2.0));
    v_rect_size = body_size;
}

#elif defined(UIX_BLUR)
layout(set = 0, binding = 0, std140) uniform BlurUniforms {
    vec4 uv_bounds;
    vec4 step_taps;
    vec4 weights[16];
} u;
layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec2 a_uv;
layout(location = 0) out vec2 v_uv;

void main() {
    // Drawing Blur 使用 D3D/OpenGL 的上正 NDC；Vulkan viewport 为下正，机械翻转 Y。
    gl_Position = vec4(a_pos.x, -a_pos.y, 0.0, 1.0);
    v_uv = a_uv;
}

#elif defined(UIX_MSDF)
layout(set = 0, binding = 0, std140) uniform MsdfUniforms {
    vec2 viewport;
    vec2 texture_size;
    vec4 range_pad;
} u;
layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec2 a_uv;
layout(location = 2) in vec4 a_color;
layout(location = 0) out vec2 v_uv;
layout(location = 1) out vec4 v_color;

void main() {
    vec2 ndc = (a_pos / u.viewport) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_uv = a_uv;
    v_color = a_color;
}

#elif defined(UIX_SECTOR)
layout(set = 0, binding = 0, std140) uniform SectorUniforms {
    vec2 viewport;
    vec2 _pad0;
    vec4 rect;
    vec4 color;
    vec4 angles;
} u;
layout(location = 0) in vec2 a_pos;
layout(location = 0) out vec2 v_local;
layout(location = 1) out vec2 v_rect_size;

void main() {
    vec2 position = u.rect.xy + a_pos * u.rect.zw;
    vec2 ndc = (position / u.viewport) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_local = a_pos * u.rect.zw;
    v_rect_size = u.rect.zw;
}

#elif defined(UIX_LINE)
layout(set = 0, binding = 0, std140) uniform LineUniforms {
    vec2 viewport;
    vec2 _pad0;
    vec4 points;
    vec4 color;
    vec4 params;
} u;
layout(location = 0) in vec2 a_pos;
layout(location = 0) out vec2 v_position;

void main() {
    float fringe = max(u.params.x * 0.5, 0.0) + 1.5;
    vec2 lower = min(u.points.xy, u.points.zw) - vec2(fringe);
    vec2 upper = max(u.points.xy, u.points.zw) + vec2(fringe);
    vec2 position = mix(lower, upper, a_pos);
    vec2 ndc = (position / u.viewport) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_position = position;
}

#else
#error "必须选择一个 Vulkan RHI 顶点变体"
#endif
