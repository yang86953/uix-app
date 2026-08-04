//! Native OpenGL ES shader sources.
//!
//! These sources are part of the GL API implementation. Program, texture and
//! framebuffer lifetime are still migrated separately from the draw layer.

/// Instanced rectangle vertex shader.
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

/// Rounded-rectangle fragment shader.
pub const RECT_FRAG: &str = r#"#version 300 es
precision highp float;

in vec2 v_local;
in vec2 v_rect_size;

uniform vec4 u_color;
uniform vec4 u_radius;
// 描边使用相对 rect 边界居中的半线宽。
uniform float u_stroke;

out vec4 fragColor;

// Port of CPU `rounded_rect_sdf` (center-relative, per-corner radius).
float rounded_rect_sdf(vec2 local, vec2 size, vec4 radius) {
    vec2 half_size = size * 0.5;
    vec2 q = local - half_size;
    float cr;
    if (q.x < 0.0)
        cr = (q.y < 0.0) ? radius.x : radius.w;
    else
        cr = (q.y < 0.0) ? radius.y : radius.z;
    vec2 d = abs(q) - half_size + cr;
    float outside = length(max(d, vec2(0.0)));
    float inside = min(max(d.x, d.y), 0.0);
    return outside + inside - cr;
}

void main() {
    vec2 size = v_rect_size;
    float mask;
    if (u_stroke > 0.0) {
        // outer shape 向 rect 外扩，inner shape 向内收缩。
        float half_stroke = u_stroke;
        vec2 outer_size = size + 2.0 * half_stroke;
        vec4 outer_radius = u_radius + half_stroke;
        vec2 inner_size = max(size - 2.0 * half_stroke, vec2(0.0));
        vec4 inner_radius = max(u_radius - half_stroke, vec4(0.0));
        float outer_sd = rounded_rect_sdf(v_local + vec2(half_stroke), outer_size, outer_radius);
        if (inner_size.x > 0.0 && inner_size.y > 0.0) {
            float inner_sd = rounded_rect_sdf(v_local - vec2(half_stroke), inner_size, inner_radius);
            mask = clamp(0.5 - outer_sd, 0.0, 1.0)
                * clamp(0.5 + inner_sd, 0.0, 1.0);
        } else {
            mask = clamp(0.5 - outer_sd, 0.0, 1.0);
        }
    } else {
        mask = any(greaterThan(u_radius, vec4(0.0)))
            ? clamp(0.5 - rounded_rect_sdf(v_local, size, u_radius), 0.0, 1.0)
            : 1.0;
    }
    if (mask <= 0.0) discard;

    // Match CPU solid fill: quantize the 8-bit straight color, premultiply
    // once, then apply analytic coverage before premultiplied SrcOver blend.
    vec4 color = floor(clamp(u_color, 0.0, 1.0) * 255.0 + 0.5);
    vec3 premul = floor(color.rgb * color.a / 255.0);
    fragColor = vec4(premul * mask, color.a * mask) / 255.0;
}
"#;

/// Batched glyph quad vertex shader. Coverage stays in a bounded R8 atlas;
/// color is carried per vertex so one draw can preserve glyph painter order.
pub const GLYPH_VERT: &str = r#"#version 300 es
precision highp float;

layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec2 a_uv;
layout(location = 2) in vec4 a_color;

uniform vec2 u_viewport;

out vec2 v_uv;
out vec4 v_color;

void main() {
    vec2 ndc = (a_pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_uv = a_uv;
    v_color = a_color;
}
"#;

/// R8 glyph coverage shader matching the CPU path's two integer truncation
/// stages before premultiplied SrcOver blending.
pub const GLYPH_FRAG: &str = r#"#version 300 es
precision highp float;

in vec2 v_uv;
in vec4 v_color;

uniform sampler2D u_atlas;

out vec4 fragColor;

void main() {
    float coverage = floor(clamp(texture(u_atlas, v_uv).r, 0.0, 1.0) * 255.0 + 0.5);
    vec4 color = floor(clamp(v_color, 0.0, 1.0) * 255.0 + 0.5);
    float alpha = floor(color.a * coverage / 255.0);
    vec3 premul = floor(color.rgb * color.a / 255.0);
    vec3 rgb = floor(premul * coverage / 255.0);
    fragColor = vec4(rgb, alpha) / 255.0;
}
"#;

/// Gaussian blur fragment shader.
#[cfg(test)]
pub const BLUR_FRAG: &str = r#"#version 300 es
precision highp float;

in vec2 v_uv;
uniform sampler2D u_source;
uniform vec2 u_texel_size;
uniform vec2 u_direction;

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

/// CPU fallback texture composite shader, including the little-endian BGRA
/// swizzle required for AARRGGBB upload data.
pub const BLIT_FRAG: &str = r#"#version 300 es
precision highp float;

in vec2 v_uv;
uniform sampler2D u_tex;
uniform vec4 u_uv_rect;

out vec4 fragColor;

void main() {
    fragColor = texture(u_tex, u_uv_rect.xy + v_uv * u_uv_rect.zw).bgra;
}
"#;

/// Native GL texture composite shader without a BGRA swizzle.
pub const BLIT_RGBA_FRAG: &str = r#"#version 300 es
precision highp float;

in vec2 v_uv;
uniform sampler2D u_tex;
uniform vec4 u_uv_rect;
// 组 opacity 对已经 premultiplied 的离屏采样结果同步缩放 RGB 和 alpha。
uniform float u_opacity;

out vec4 fragColor;

void main() {
    // Picture texture 的结果必须按组 opacity 保持 premultiplied 语义。
    fragColor = texture(u_tex, u_uv_rect.xy + v_uv * u_uv_rect.zw) * u_opacity;
}
"#;

/// Full-screen quad vertex shader for blur and texture composite passes.
pub const FULLSCREEN_VERT: &str = r#"#version 300 es
precision highp float;

in vec2 a_pos;
out vec2 v_uv;

void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_uv = a_pos * 0.5 + 0.5;
    v_uv.y = 1.0 - v_uv.y;
}
"#;
