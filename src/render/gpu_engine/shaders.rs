// ============================================================================
// graphics/gpu_engine/shaders.rs — GLES 3.0 着色器
//
// 矩形实例化渲染管线着色器，支持：纯色/纹理/渐变/圆角/阴影模糊。
// ============================================================================

/// 矩形顶点着色器 —— 单 quad 变换。
pub const RECT_VERT: &str = r#"#version 300 es
precision highp float;

in vec2 a_pos;

uniform vec2 u_viewport;
uniform vec4 u_rect;

out vec2 v_local;
out vec2 v_rect_size;

void main() {
    vec2 pos = u_rect.xy + a_pos * u_rect.zw;
    vec2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    gl_Position = vec4(ndc, 0.0, 1.0);

    v_local = a_pos * u_rect.zw;
    v_rect_size = u_rect.zw;
}
"#;

/// 矩形片段着色器 —— 纯色 + 圆角 SDF。
pub const RECT_FRAG: &str = r#"#version 300 es
precision highp float;

in vec2 v_local;
in vec2 v_rect_size;

uniform vec4 u_color;
uniform vec4 u_radius;

out vec4 fragColor;

// 圆角 SDF 遮罩
float corner_mask(vec2 p, float r) {
    return 1.0 - smoothstep(r - 1.0, r + 1.0, length(p));
}

float rounded_rect_mask(vec2 local, vec2 size, vec4 radius) {
    float m = 1.0;
    if (radius.x > 0.0 && local.x < radius.x && local.y < radius.x)
        m *= corner_mask(local - vec2(radius.x), radius.x);
    if (radius.y > 0.0 && local.x > size.x - radius.y && local.y < radius.y)
        m *= corner_mask(local - vec2(size.x - radius.y, radius.y), radius.y);
    if (radius.z > 0.0 && local.x > size.x - radius.z && local.y > size.y - radius.z)
        m *= corner_mask(local - vec2(size.x - radius.z, size.y - radius.z), radius.z);
    if (radius.w > 0.0 && local.x < radius.w && local.y > size.y - radius.w)
        m *= corner_mask(local - vec2(radius.w, size.y - radius.w), radius.w);
    return m;
}

void main() {
    vec2 size = v_rect_size;
    float mask = rounded_rect_mask(v_local, size, u_radius);
    if (mask <= 0.0) discard;

    fragColor = u_color * mask;
}
"#;

/// 高斯模糊片段着色器 —— 单 Pass（9-tap 水平模糊）
pub const BLUR_FRAG: &str = r#"#version 300 es
precision highp float;

in vec2 v_uv;
uniform sampler2D u_source;
uniform vec2 u_texel_size;
uniform vec2 u_direction; // (1,0)=水平, (0,1)=垂直

out vec4 fragColor;

void main() {
    vec2 dir = u_texel_size * u_direction;
    vec4 color = vec4(0.0);
    float weights[5] = float[](0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    color += texture(u_source, v_uv) * weights[0];
    for (int i = 1; i < 5; i++) {
        color += texture(u_source, v_uv + dir * float(i)) * weights[i];
        color += texture(u_source, v_uv - dir * float(i)) * weights[i];
    }
    fragColor = color;
}
"#;

/// 全屏 quad 的顶点着色器（用于模糊 pass）
pub const FULLSCREEN_VERT: &str = r#"#version 300 es
precision highp float;

in vec2 a_pos;
out vec2 v_uv;

void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_uv = a_pos * 0.5 + 0.5;
}
"#;
