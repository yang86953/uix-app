#include <metal_stdlib>
using namespace metal;

// Metal Adapter 直接消费共享 RHI 冻结的 float4 常量块，避免复制另一套结构体 ABI。
struct UnitVertex { float2 position [[attribute(0)]]; };
struct MeshVertex { float2 position [[attribute(0)]]; float coverage [[attribute(1)]]; };
struct SampleVertex {
    float2 position [[attribute(0)]];
    float2 uv [[attribute(1)]];
    float4 color [[attribute(2)]];
};
struct BlurVertex { float2 position [[attribute(0)]]; float2 uv [[attribute(1)]]; };

struct UnitOut { float4 position [[position]]; float2 local; float2 size; };
struct MeshOut { float4 position [[position]]; float coverage; };
struct SampleOut { float4 position [[position]]; float2 uv; float4 color; };
struct BlurOut { float4 position [[position]]; float2 uv; };
struct LineOut { float4 position [[position]]; float2 pixel; };

float2 uix_ndc(float2 pixel, float2 viewport) {
    float2 ndc = pixel / viewport * 2.0 - 1.0;
    ndc.y = -ndc.y;
    return ndc;
}

float uix_rounded_rect_sdf(float2 local, float2 size, float4 radius) {
    // 直角矩形与各角圆弧约束的交集：直边距离为基线，落入切线方形的
    // 每个角弧距离（圆外为正）取最大。归一化只保证相邻切线方形不相交，
    // 对角方形可重叠（重叠区须同时满足两角约束）。local 为左上角原点，
    // radius 分量顺序 tl/tr/br/bl。
    float2 q = abs(local - size * 0.5) - size * 0.5;
    float combined = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0);
    if (radius.x > 0.0 && local.x < radius.x && local.y < radius.x) {
        combined = max(combined, distance(local, float2(radius.x)) - radius.x);
    }
    if (radius.y > 0.0 && local.x > size.x - radius.y && local.y < radius.y) {
        combined = max(combined, distance(local, float2(size.x - radius.y, radius.y)) - radius.y);
    }
    if (radius.z > 0.0 && local.x > size.x - radius.z && local.y > size.y - radius.z) {
        combined = max(combined, distance(local, size - radius.z) - radius.z);
    }
    if (radius.w > 0.0 && local.x < radius.w && local.y > size.y - radius.w) {
        combined = max(combined, distance(local, float2(radius.w, size.y - radius.w)) - radius.w);
    }
    return combined;
}

float4 uix_quantized_premul(float4 color, float coverage) {
    float4 byte_color = floor(clamp(color, 0.0, 1.0) * 255.0 + 0.5);
    float3 premul = floor(byte_color.rgb * byte_color.a / 255.0);
    return float4(premul * coverage, byte_color.a * coverage) / 255.0;
}

vertex MeshOut solid_vs(MeshVertex input [[stage_in]], constant float4* c [[buffer(1)]]) {
    return { float4(uix_ndc(input.position, c[0].xy), 0.0, 1.0), input.coverage };
}

fragment float4 solid_fs(MeshOut input [[stage_in]], constant float4* c [[buffer(1)]]) {
    return float4(c[1].rgb, c[1].a * clamp(input.coverage, 0.0, 1.0));
}

vertex SampleOut sampled_vs(SampleVertex input [[stage_in]], constant float4* c [[buffer(1)]]) {
    return { float4(uix_ndc(input.position, c[0].xy), 0.0, 1.0), input.uv, input.color };
}

fragment float4 textured_fs(
    SampleOut input [[stage_in]],
    constant float4* c [[buffer(1)]],
    texture2d<float> image [[texture(0)]],
    sampler image_sampler [[sampler(0)]]) {
    float4 sampled = image.sample(image_sampler, input.uv);
    float coverage = 1.0;
    float fill_alpha = 0.0;
    float radius = c[1].x;
    if (radius > 0.0) {
        float2 half_size = c[0].xy * 0.5;
        float2 distance = abs(input.position.xy - half_size) - half_size + radius;
        float signed_distance = length(max(distance, 0.0))
            + min(max(distance.x, distance.y), 0.0) - radius;
        coverage = clamp(0.5 - signed_distance / max(fwidth(signed_distance), 0.0001), 0.0, 1.0);
        // c[0].zw 保存左上/右上、c[1].zw 保存左下/右下缺口补画峰值；
        // c[1].y 保存补画物理衰减距离，同时是补画启用事实。
        if (c[1].y > 0.0) {
            // 圆角缺口用与客户端阴影环一致的二次衰减延伸外圈阴影；
            // uv 保持内容左上原点约定，按所在象限取逐角补画峰值。
            float horizontal = input.uv.y < 0.5
                ? mix(c[0].z, c[0].w, step(0.5, input.uv.x))
                : mix(c[1].z, c[1].w, step(0.5, input.uv.x));
            float falloff = clamp(1.0 - signed_distance / c[1].y, 0.0, 1.0);
            fill_alpha = horizontal * falloff * falloff;
        }
    }
    // 缺口阴影是预乘黑色，只向输出 alpha 贡献补画事实。
    return float4(sampled.rgb * input.color.rgb * coverage,
                  sampled.a * input.color.a * coverage + fill_alpha * (1.0 - coverage));
}

fragment float4 coverage_fs(
    SampleOut input [[stage_in]],
    texture2d<float> atlas [[texture(0)]],
    sampler atlas_sampler [[sampler(0)]]) {
    float coverage = floor(atlas.sample(atlas_sampler, input.uv).r * 255.0 + 0.5);
    float4 color = floor(input.color * 255.0 + 0.5);
    float alpha = floor(color.a * coverage / 255.0);
    float3 premul = floor(color.rgb * color.a / 255.0);
    return float4(floor(premul * coverage / 255.0), alpha) / 255.0;
}

vertex UnitOut gradient_vs(UnitVertex input [[stage_in]], constant float4* c [[buffer(1)]]) {
    float2 pixel = c[1].xy + input.position.x * c[1].zw + input.position.y * c[2].xy;
    return { float4(uix_ndc(pixel, c[0].xy), 0.0, 1.0), input.position, float2(0.0) };
}

fragment float4 gradient_fs(UnitOut input [[stage_in]], constant float4* c [[buffer(1)]]) {
    float4 params = c[5];
    // S4 圆角掩码：c[6]=四角半径，c[7]=quad 宽高+掩码单位起点，c[8]=掩码单位宽高。
    float coverage = 1.0;
    if (any(c[6] > float4(0.0))) {
        float2 quad_size = c[7].xy;
        float2 mask_origin = c[7].zw;
        float2 mask_px = (input.local - mask_origin) * quad_size;
        float2 mask_wh = c[8].xy * quad_size;
        float sdf = uix_rounded_rect_sdf(mask_px, mask_wh, c[6]);
        coverage = clamp(0.5 - sdf, 0.0, 1.0);
        if (coverage <= 0.0) discard_fragment();
    }
    if (params.x < 0.5) {
        float2 local = input.local;
        float2 size = params.zw;
        float t = params.y < 0.5 ? local.x
            : params.y < 1.5 ? local.y
            : params.y < 2.5
                ? (local.x * size.x + local.y * size.y) / max(size.x + size.y, 1e-6)
                : (local.x * size.x - local.y * size.y + size.y) / max(size.x + size.y, 1e-6);
        return mix(c[3], c[4], clamp(t, 0.0, 1.0)) * coverage;
    }
    float distance = length(input.local - 0.5);
    if (distance > params.z) discard_fragment();
    float t = clamp((distance - params.y) / max(params.z - params.y, 1e-6), 0.0, 1.0);
    return mix(c[3], c[4], t) * coverage;
}

vertex UnitOut shape_vs(UnitVertex input [[stage_in]], constant float4* c [[buffer(1)]]) {
    float2 pixel = c[5].xy + input.position * c[5].zw;
    return { float4(uix_ndc(pixel, c[0].xy), 0.0, 1.0), input.position * c[5].zw, c[1].zw };
}

fragment float4 shape_fs(UnitOut input [[stage_in]], constant float4* c [[buffer(1)]]) {
    float mask;
    if (c[4].x > 0.0) {
        float h = c[4].x;
        float outer = uix_rounded_rect_sdf(input.local - c[4].zz, input.size + 2.0 * h, c[3] + h);
        float2 inner_size = max(input.size - 2.0 * h, 0.0);
        if (inner_size.x > 0.0 && inner_size.y > 0.0) {
            float inner = uix_rounded_rect_sdf(input.local - c[4].ww, inner_size, max(c[3] - h, 0.0));
            mask = clamp(0.5 - outer, 0.0, 1.0) * clamp(0.5 + inner, 0.0, 1.0);
        } else {
            mask = clamp(0.5 - outer, 0.0, 1.0);
        }
    } else {
        mask = any(c[3] > 0.0)
            ? clamp(0.5 - uix_rounded_rect_sdf(input.local, input.size, c[3]), 0.0, 1.0)
            : 1.0;
    }
    if (mask <= 0.0) discard_fragment();
    return uix_quantized_premul(c[2], mask);
}

vertex UnitOut shadow_vs(UnitVertex input [[stage_in]], constant float4* c [[buffer(1)]]) {
    float2 blur = max(c[4].zw, 0.0);
    float2 body_size = max(c[5].xy, 0.0001);
    float2 expanded = body_size + 2.0 * blur;
    float2 axis_x = c[1].zw / max(expanded.x, 0.0001);
    float2 axis_y = c[4].xy / max(expanded.y, 0.0001);
    float2 pixel = c[1].xy - axis_x - axis_y
        + input.position.x * (c[1].zw + 2.0 * axis_x)
        + input.position.y * (c[4].xy + 2.0 * axis_y);
    return { float4(uix_ndc(pixel, c[0].xy), 0.0, 1.0), input.position * (expanded + 2.0), body_size };
}

fragment float4 shadow_fs(UnitOut input [[stage_in]], constant float4* c [[buffer(1)]]) {
    float2 blur = max(c[4].zw, 0.0);
    float blur_radius = max(blur.x, blur.y);
    float distance = uix_rounded_rect_sdf(input.local - blur - 1.0, input.size, c[3]);
    float coverage;
    if (blur_radius > 0.5) {
        if (c[5].z > 0.5) {
            float half_blur = blur_radius * 0.5;
            float t = clamp((half_blur - distance) / (blur_radius + half_blur), 0.0, 1.0);
            coverage = t * t * t * t * (5.0 - 4.0 * t);
        } else {
            float t = clamp((blur_radius - distance) / (2.0 * blur_radius), 0.0, 1.0);
            coverage = t * t * (3.0 - 2.0 * t);
        }
    } else {
        coverage = clamp(0.5 - distance, 0.0, 1.0);
    }
    if (coverage <= 0.0) discard_fragment();
    return float4(c[2].rgb, c[2].a * coverage);
}

vertex BlurOut blur_vs(BlurVertex input [[stage_in]]) {
    return { float4(input.position, 0.0, 1.0), input.uv };
}

fragment float4 blur_fs(
    BlurOut input [[stage_in]],
    constant float4* c [[buffer(1)]],
    texture2d<float> image [[texture(0)]],
    sampler image_sampler [[sampler(0)]]) {
    float4 color = 0.0;
    int radius = int(c[1].z);
    for (int i = 0; i < 64; ++i) {
        float weight = c[2 + i / 4][i % 4];
        if (weight <= 0.0) break;
        float2 uv = clamp(input.uv + c[1].xy * float(i - radius), c[0].xy, c[0].zw);
        color += weight * image.sample(image_sampler, uv);
    }
    return color;
}

float uix_median(float a, float b, float c) {
    return max(min(a, b), min(max(a, b), c));
}

fragment float4 msdf_fs(
    SampleOut input [[stage_in]],
    constant float4* c [[buffer(1)]],
    texture2d<float> atlas [[texture(0)]],
    sampler atlas_sampler [[sampler(0)]]) {
    float3 encoded = atlas.sample(atlas_sampler, input.uv).rgb;
    float signed_distance = uix_median(encoded.r, encoded.g, encoded.b) - 0.5;
    float2 uv_derivative = abs(dfdx(input.uv)) + abs(dfdy(input.uv));
    float2 unit_range = c[1].zz / max(c[1].xy, 1.0);
    float2 screen_texture_size = 1.0 / max(uv_derivative, 0.000001);
    float coverage = clamp(0.5 - signed_distance * max(0.5 * dot(unit_range, screen_texture_size), 1.0), 0.0, 1.0);
    float coverage_byte = floor(coverage * 255.0 + 0.5);
    float4 color = floor(input.color * 255.0 + 0.5);
    float alpha = floor(color.a * coverage_byte / 255.0);
    float3 premul = floor(color.rgb * color.a / 255.0);
    return float4(floor(premul * coverage_byte / 255.0), alpha) / 255.0;
}

vertex UnitOut sector_vs(UnitVertex input [[stage_in]], constant float4* c [[buffer(1)]]) {
    float2 pixel = c[1].xy + input.position * c[1].zw;
    return { float4(uix_ndc(pixel, c[0].xy), 0.0, 1.0), input.position * c[1].zw, c[1].zw };
}

fragment float4 sector_fs(UnitOut input [[stage_in]], constant float4* c [[buffer(1)]]) {
    const float tau = 6.283185307179586;
    float2 unit = (input.local / max(input.size, 0.0001) - 0.5) * 2.0;
    float radius = length(unit);
    float radial_mask = clamp((1.0 - radius) / max(fwidth(radius), 0.0001) + 0.5, 0.0, 1.0);
    float angular_mask = 1.0;
    if (c[3].y < tau - 0.0001 && radius > 0.0001) {
        float angle = atan2(unit.y, unit.x);
        if (angle < 0.0) angle += tau;
        float delta = fmod(angle - c[3].x + tau, tau);
        float edge = min(delta, c[3].y - delta);
        angular_mask = clamp(edge / max(fwidth(angle), 0.0001) + 0.5, 0.0, 1.0);
        if (delta > c[3].y) angular_mask = 0.0;
    }
    float mask = radial_mask * angular_mask;
    if (mask <= 0.0) discard_fragment();
    return uix_quantized_premul(c[2], mask);
}

vertex LineOut line_vs(UnitVertex input [[stage_in]], constant float4* c [[buffer(1)]]) {
    float fringe = max(c[3].x * 0.5, 0.0) + 1.5;
    float2 lower = min(c[1].xy, c[1].zw) - fringe;
    float2 upper = max(c[1].xy, c[1].zw) + fringe;
    float2 pixel = mix(lower, upper, input.position);
    return { float4(uix_ndc(pixel, c[0].xy), 0.0, 1.0), pixel };
}

fragment float4 line_fs(LineOut input [[stage_in]], constant float4* c [[buffer(1)]]) {
    float2 segment = c[1].zw - c[1].xy;
    float along = clamp(dot(input.pixel - c[1].xy, segment) / max(dot(segment, segment), 0.000001), 0.0, 1.0);
    float distance = length(input.pixel - (c[1].xy + along * segment)) - c[3].x * 0.5;
    float coverage = clamp(0.5 - distance / max(fwidth(distance), 0.0001), 0.0, 1.0);
    if (coverage <= 0.0) discard_fragment();
    return uix_quantized_premul(c[2], coverage);
}

vertex float4 clear_vs(uint vertex_id [[vertex_id]]) {
    float2 position = vertex_id == 0 ? float2(-1.0, -1.0)
        : vertex_id == 1 ? float2(3.0, -1.0)
        : float2(-1.0, 3.0);
    return float4(position, 0.0, 1.0);
}

fragment float4 clear_fs(constant float4& color [[buffer(0)]]) { return color; }
