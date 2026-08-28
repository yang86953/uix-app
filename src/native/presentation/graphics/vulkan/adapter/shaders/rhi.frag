#version 450

// Vulkan 薄 RHI 片元阶段的唯一源码；颜色、覆盖率与混合输入保持共享 ABI。
layout(location = 0) out vec4 out_color;

#if defined(UIX_MESH)
layout(set = 0, binding = 0, std140) uniform MeshUniforms {
    vec2 viewport;
    vec2 _pad0;
    vec4 color;
} u;
layout(location = 0) in float v_coverage;

void main() {
    out_color = vec4(u.color.rgb, u.color.a * clamp(v_coverage, 0.0, 1.0));
}

#elif defined(UIX_TEXTURED)
layout(set = 0, binding = 0, std140) uniform SampledUniforms {
    vec2 viewport;
    vec2 _pad0;
    // x 保存物理圆角半径；y/z 保存缺口阴影峰值不透明度与物理外扩距离。
    vec4 surface_clip;
} u;
layout(set = 0, binding = 1) uniform sampler2D u_texture;
layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec4 v_color;

void main() {
    vec4 sample_color = texture(u_texture, v_uv);
    float coverage = 1.0;
    float fill_alpha = 0.0;
    if (u.surface_clip.x > 0.0) {
        vec2 half_size = u.viewport * 0.5;
        vec2 centered = abs(gl_FragCoord.xy - half_size);
        vec2 distance = centered - half_size + u.surface_clip.x;
        float signed_distance = length(max(distance, vec2(0.0)))
            + min(max(distance.x, distance.y), 0.0) - u.surface_clip.x;
        coverage = clamp(0.5 - signed_distance / max(fwidth(signed_distance), 0.0001), 0.0, 1.0);
        if (u.surface_clip.y > 0.0 && u.surface_clip.z > 0.0) {
            // 圆角缺口位于窗口矩形内但内容没有覆盖，用与客户端阴影环一致
            // 的二次衰减把外圈阴影延伸进来，避免缺口透出桌面背景。
            float falloff = clamp(1.0 - signed_distance / u.surface_clip.z, 0.0, 1.0);
            fill_alpha = u.surface_clip.y * falloff * falloff;
        }
    }
    // 缺口阴影是预乘黑色，只向输出贡献 alpha；内容侧保持原预乘采样结果。
    out_color = vec4(sample_color.rgb * v_color.rgb * coverage,
                     sample_color.a * v_color.a * coverage + fill_alpha * (1.0 - coverage));
}

#elif defined(UIX_COVERAGE)
layout(set = 0, binding = 1) uniform sampler2D u_texture;
layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec4 v_color;

void main() {
    float coverage = floor(texture(u_texture, v_uv).r * 255.0 + 0.5);
    vec4 color = floor(v_color * 255.0 + 0.5);
    float alpha = floor(color.a * coverage / 255.0);
    vec3 premultiplied = floor(color.rgb * color.a / 255.0);
    vec3 rgb = floor(premultiplied * coverage / 255.0);
    out_color = vec4(rgb, alpha) / 255.0;
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
} u;
layout(location = 0) in vec2 v_local;

void main() {
    if (u.params.x < 0.5) {
        float direction = u.params.y;
        float t;
        if (direction < 0.5) {
            t = v_local.x;
        } else if (direction < 1.5) {
            t = v_local.y;
        } else if (direction < 2.5) {
            t = (v_local.x * u.params.z + v_local.y * u.params.w)
                / max(u.params.z + u.params.w, 0.000001);
        } else {
            t = (v_local.x * u.params.z - v_local.y * u.params.w + u.params.w)
                / max(u.params.z + u.params.w, 0.000001);
        }
        out_color = mix(u.color_a, u.color_b, clamp(t, 0.0, 1.0));
        return;
    }

    float distance_to_center = distance(v_local, vec2(0.5));
    float outer_radius = u.params.z;
    if (distance_to_center > outer_radius) {
        discard;
    }
    float range = max(outer_radius - u.params.y, 0.000001);
    float t = clamp((distance_to_center - u.params.y) / range, 0.0, 1.0);
    out_color = mix(u.color_a, u.color_b, t);
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
layout(location = 0) in vec2 v_local;
layout(location = 1) in vec2 v_rect_size;

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

void main() {
    float mask;
    if (u.stroke.x > 0.0) {
        vec2 outer_local = v_local - vec2(u.stroke.z);
        vec2 inner_local = v_local - vec2(u.stroke.w);
        float half_stroke = u.stroke.x;
        vec2 outer_size = v_rect_size + 2.0 * half_stroke;
        vec4 outer_radius = u.radius + half_stroke;
        vec2 inner_size = max(v_rect_size - 2.0 * half_stroke, vec2(0.0));
        vec4 inner_radius = max(u.radius - half_stroke, vec4(0.0));
        float outer_distance = rounded_rect_sdf(outer_local, outer_size, outer_radius);
        if (inner_size.x > 0.0 && inner_size.y > 0.0) {
            float inner_distance = rounded_rect_sdf(inner_local, inner_size, inner_radius);
            mask = clamp(0.5 - outer_distance, 0.0, 1.0)
                * clamp(0.5 + inner_distance, 0.0, 1.0);
        } else {
            mask = clamp(0.5 - outer_distance, 0.0, 1.0);
        }
    } else {
        mask = any(greaterThan(u.radius, vec4(0.0)))
            ? clamp(0.5 - rounded_rect_sdf(v_local, v_rect_size, u.radius), 0.0, 1.0)
            : 1.0;
    }
    if (mask <= 0.0) {
        discard;
    }
    vec4 color = floor(clamp(u.color, 0.0, 1.0) * 255.0 + 0.5);
    vec3 premultiplied = floor(color.rgb * color.a / 255.0);
    out_color = vec4(premultiplied * mask, color.a * mask) / 255.0;
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
layout(location = 0) in vec2 v_local;
layout(location = 1) in vec2 v_rect_size;

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

float ambient_coverage(float signed_distance, float blur) {
    float half_blur = blur * 0.5;
    float t = clamp((half_blur - signed_distance) / (blur + half_blur), 0.0, 1.0);
    float squared = t * t;
    return squared * squared * (5.0 - 4.0 * t);
}

void main() {
    vec2 blur = max(u.params.zw, vec2(0.0));
    float blur_radius = max(blur.x, blur.y);
    vec2 shape_local = v_local - blur - vec2(1.0);
    float signed_distance = rounded_rect_sdf(shape_local, v_rect_size, u.radius);
    float coverage;
    if (blur_radius > 0.5) {
        coverage = u.size.z > 0.5
            ? ambient_coverage(signed_distance, blur_radius)
            : shadow_coverage(signed_distance, blur_radius);
    } else {
        coverage = clamp(0.5 - signed_distance, 0.0, 1.0);
    }
    if (coverage <= 0.0) {
        discard;
    }
    out_color = vec4(u.color.rgb, u.color.a * coverage);
}

#elif defined(UIX_BLUR)
layout(set = 0, binding = 0, std140) uniform BlurUniforms {
    vec4 uv_bounds;
    vec4 step_taps;
    vec4 weights[16];
} u;
layout(set = 0, binding = 1) uniform sampler2D u_texture;
layout(location = 0) in vec2 v_uv;

void main() {
    vec2 step_size = u.step_taps.xy;
    int radius = int(u.step_taps.z);
    vec4 color = vec4(0.0);
    for (int index = 0; index < 64; ++index) {
        float weight = u.weights[index / 4][index % 4];
        if (weight <= 0.0) {
            break;
        }
        vec2 offset = step_size * float(index - radius);
        vec2 sample_uv = clamp(v_uv + offset, u.uv_bounds.xy, u.uv_bounds.zw);
        color += weight * texture(u_texture, sample_uv);
    }
    out_color = color;
}

#elif defined(UIX_MSDF)
layout(set = 0, binding = 0, std140) uniform MsdfUniforms {
    vec2 viewport;
    vec2 texture_size;
    vec4 range_pad;
} u;
layout(set = 0, binding = 1) uniform sampler2D u_texture;
layout(location = 0) in vec2 v_uv;
layout(location = 1) in vec4 v_color;

float median3(float red, float green, float blue) {
    return max(min(red, green), min(max(red, green), blue));
}

void main() {
    vec3 encoded = texture(u_texture, v_uv).rgb;
    float signed_distance = median3(encoded.r, encoded.g, encoded.b) - 0.5;
    vec2 derivative = max(fwidth(v_uv), vec2(0.000001));
    vec2 unit_range = u.range_pad.x / max(u.texture_size, vec2(1.0));
    vec2 screen_texture_size = 1.0 / derivative;
    float pixel_range = max(0.5 * dot(unit_range, screen_texture_size), 1.0);
    float coverage = clamp(0.5 - signed_distance * pixel_range, 0.0, 1.0);
    float coverage_byte = floor(coverage * 255.0 + 0.5);
    vec4 color = floor(v_color * 255.0 + 0.5);
    float alpha = floor(color.a * coverage_byte / 255.0);
    vec3 premultiplied = floor(color.rgb * color.a / 255.0);
    vec3 rgb = floor(premultiplied * coverage_byte / 255.0);
    out_color = vec4(rgb, alpha) / 255.0;
}

#elif defined(UIX_SECTOR)
layout(set = 0, binding = 0, std140) uniform SectorUniforms {
    vec2 viewport;
    vec2 _pad0;
    vec4 rect;
    vec4 color;
    vec4 angles;
} u;
layout(location = 0) in vec2 v_local;
layout(location = 1) in vec2 v_rect_size;

const float TAU = 6.283185307179586;

void main() {
    vec2 unit = (v_local / max(v_rect_size, vec2(0.0001)) - 0.5) * 2.0;
    float radius = length(unit);
    float radial_width = max(fwidth(radius), 0.0001);
    float radial_mask = clamp((1.0 - radius) / radial_width + 0.5, 0.0, 1.0);
    float angular_mask = 1.0;
    if (u.angles.y < TAU - 0.0001 && radius > 0.0001) {
        float angle = atan(unit.y, unit.x);
        if (angle < 0.0) {
            angle += TAU;
        }
        float delta = mod(angle - u.angles.x + TAU, TAU);
        float edge = min(delta, u.angles.y - delta);
        float angular_width = max(fwidth(angle), 0.0001);
        angular_mask = clamp(edge / angular_width + 0.5, 0.0, 1.0);
        if (delta > u.angles.y) {
            angular_mask = 0.0;
        }
    }
    float mask = radial_mask * angular_mask;
    if (mask <= 0.0) {
        discard;
    }
    vec4 color = floor(clamp(u.color, 0.0, 1.0) * 255.0 + 0.5);
    vec3 premultiplied = floor(color.rgb * color.a / 255.0);
    out_color = vec4(premultiplied * mask, color.a * mask) / 255.0;
}

#elif defined(UIX_LINE)
layout(set = 0, binding = 0, std140) uniform LineUniforms {
    vec2 viewport;
    vec2 _pad0;
    vec4 points;
    vec4 color;
    vec4 params;
} u;
layout(location = 0) in vec2 v_position;

void main() {
    vec2 start = u.points.xy;
    vec2 segment = u.points.zw - start;
    float length_squared = max(dot(segment, segment), 0.000001);
    float along = clamp(dot(v_position - start, segment) / length_squared, 0.0, 1.0);
    float distance_to_line = length(v_position - (start + along * segment)) - u.params.x * 0.5;
    float derivative = max(fwidth(distance_to_line), 0.0001);
    float coverage = clamp(0.5 - distance_to_line / derivative, 0.0, 1.0);
    if (coverage <= 0.0) {
        discard;
    }
    vec4 color = floor(clamp(u.color, 0.0, 1.0) * 255.0 + 0.5);
    vec3 premultiplied = floor(color.rgb * color.a / 255.0);
    out_color = vec4(premultiplied * coverage, color.a * coverage) / 255.0;
}

#else
#error "必须选择一个 Vulkan RHI 片元变体"
#endif
