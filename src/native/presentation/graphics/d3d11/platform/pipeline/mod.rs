//! D3D11 native geometry pipeline — solid/stroke rects + glyph atlas +
//! path meshes + box shadow.
//!
//! Hot Canvas2D paths: `fill_rect` / `fill_circle` / `stroke_rect` /
//! `stroke_circle` (+ axis-aligned `draw_line` via solid fill), identity
//! solid `blit_glyph` via coverage atlas, identity linear/radial gradient
//! fills, identity simple `fill_path` / `stroke_path` (CPU tessellate →
//! solid triangles), and identity box/ambient shadow (SDF outer glow).

#![allow(nonstandard_style)]

use std::ffi::CStr;

use crate::core::{Errc, Error, Result};
// 引入跨 Adapter 共享的 pipeline 固定状态语义。
use crate::native::present::rhi::{
    PIPELINE_DEPTH_STENCIL_DISABLED, PIPELINE_RASTER_2D, PipelineBlend, PipelineBlendFactor,
    PipelineBlendOperation, PipelineColorWriteMask, PipelineCullMode, PipelineDepthClip,
    PipelineDepthState, PipelineDepthStencilState, PipelineDitherState, PipelineFrontFace,
    PipelineMultisampleState, PipelinePrimitiveTopology, PipelineRasterState, PipelineStencilState,
};
use ::windows::Win32::Foundation::{FALSE, TRUE};
use ::windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use ::windows::Win32::Graphics::Direct3D::{
    D3D_PRIMITIVE_TOPOLOGY, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob,
};
use ::windows::Win32::Graphics::Direct3D11::{
    D3D11_BLEND, D3D11_BLEND_DESC, D3D11_BLEND_INV_SRC_ALPHA, D3D11_BLEND_ONE, D3D11_BLEND_OP,
    D3D11_BLEND_OP_ADD, D3D11_BLEND_SRC_ALPHA, D3D11_BLEND_ZERO, D3D11_COLOR_WRITE_ENABLE_ALL,
    D3D11_COMPARISON_ALWAYS, D3D11_COMPARISON_FUNC, D3D11_CULL_MODE, D3D11_CULL_NONE,
    D3D11_DEPTH_STENCIL_DESC, D3D11_DEPTH_STENCILOP_DESC, D3D11_DEPTH_WRITE_MASK,
    D3D11_DEPTH_WRITE_MASK_ZERO, D3D11_FILL_SOLID, D3D11_INPUT_ELEMENT_DESC,
    D3D11_INPUT_PER_VERTEX_DATA, D3D11_RASTERIZER_DESC, D3D11_RENDER_TARGET_BLEND_DESC,
    D3D11_STENCIL_OP_KEEP, ID3D11BlendState, ID3D11Buffer, ID3D11DepthStencilState, ID3D11Device,
    ID3D11DeviceContext, ID3D11InputLayout, ID3D11PixelShader, ID3D11RasterizerState,
    ID3D11RenderTargetView, ID3D11SamplerState, ID3D11ShaderResourceView, ID3D11VertexShader,
};
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R32_UINT, DXGI_FORMAT_R32G32_FLOAT, DXGI_FORMAT_R32G32B32A32_FLOAT,
};
use ::windows::core::{BOOL, PCSTR};

const RECT_HLSL: &str = r#"
cbuffer RectCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_rect;
    float4 u_color;
    float4 u_radius;
    // x 保存半描边宽度，y 保存 fringe，z/w 保存共享 outer/inner 原点偏移。
    float4 u_stroke;
    // 共享 GPU Raster Module 已经计算完成的实际绘制边界。
    float4 u_draw_rect;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 local : TEXCOORD0;
    float2 rect_size : TEXCOORD1;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    // 平台 shader 只消费共享层冻结的绘制边界，不再自行推导描边外扩。
    float2 pos = u_draw_rect.xy + input.pos * u_draw_rect.zw;
    float2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    // 局部坐标覆盖共享 draw rect，供片元阶段恢复同心双 SDF。
    o.local = input.pos * u_draw_rect.zw;
    o.rect_size = u_rect.zw;
    return o;
}

// Port of CPU `rounded_rect_sdf` (center-relative, per-corner radius).
float rounded_rect_sdf(float2 local, float2 size, float4 radius)
{
    float2 half_size = size * 0.5;
    float2 q = local - half_size;
    float cr;
    if (q.x < 0.0)
        cr = (q.y < 0.0) ? radius.x : radius.w;
    else
        cr = (q.y < 0.0) ? radius.y : radius.z;
    float2 d = abs(q) - half_size + cr;
    float outside = length(max(d, 0.0));
    float inside = min(max(d.x, d.y), 0.0);
    return outside + inside - cr;
}

float4 PSMain(VSOut input) : SV_Target
{
    float mask;
    if (u_stroke.x > 0.0)
    {
        // 使用共享 outer 偏移把 draw rect 局部坐标转换到 outer SDF 原点。
        float2 outer_local = input.local - float2(u_stroke.z, u_stroke.z);
        // 使用共享 inner 偏移保持 inner 与 outer SDF 严格同心。
        float2 inner_local = input.local - float2(u_stroke.w, u_stroke.w);
        // 双 SDF（与 CPU 一致）：外扩/内缩 half 使弧线端点对齐像素
        // 中心，消除整数坐标下顶/底圆角起点偏差；中心行 coverage 与 CPU 相同。
        float h = u_stroke.x;
        float2 outer_size = input.rect_size + 2.0 * h;
        float4 outer_rad = u_radius + h;
        float2 inner_size = max(input.rect_size - 2.0 * h, 0.0);
        float4 inner_rad = max(u_radius - h, 0.0);
        // 计算外边界有符号距离。
        float outer_sd = rounded_rect_sdf(outer_local, outer_size, outer_rad);
        if (inner_size.x > 0.0 && inner_size.y > 0.0)
        {
            // 以内缩后的独立局部原点计算同心内边界距离。
            float inner_sd = rounded_rect_sdf(inner_local, inner_size, inner_rad);
            // Matches CPU `sdf_to_coverage(outer) * sdf_to_coverage(-inner)`.
            mask = saturate(0.5 - outer_sd) * saturate(0.5 + inner_sd);
        }
        else
        {
            // 描边宽度盖满矩形：直接填充外扩圆角矩形。
            mask = saturate(0.5 - outer_sd);
        }
    }
    else
    {
        // Same 1px linear AA coverage as CPU `sdf_to_coverage`.
        mask = any(u_radius > 0.0)
            ? saturate(0.5 - rounded_rect_sdf(input.local, input.rect_size, u_radius))
            : 1.0;
    }
    if (mask <= 0.0)
        discard;
    // Match CPU fill/stroke: quantize the 8-bit straight color, premultiply
    // once, then apply analytic coverage before premultiplied SrcOver blend.
    float4 color = floor(saturate(u_color) * 255.0 + 0.5);
    float3 premul = floor(color.rgb * color.a / 255.0);
    return float4(premul * mask, color.a * mask) / 255.0;
}
"#;

const GLYPH_HLSL: &str = r#"
cbuffer GlyphCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
};

Texture2D<float> u_atlas : register(t0);
SamplerState u_samp : register(s0);

struct VSIn {
    float2 pos : POSITION;
    float2 uv : TEXCOORD0;
    float4 color : COLOR0;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD0;
    float4 color : COLOR0;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    float2 ndc = (input.pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    o.uv = input.uv;
    o.color = input.color;
    return o;
}

float4 PSMain(VSOut input) : SV_Target
{
    // Match the CPU glyph path's two integer truncation steps before the
    // premultiplied SrcOver blend. Keeping this quantization in the shader
    // avoids the 2-channel drift caused by a straight-alpha hardware multiply.
    float coverage = floor(saturate(u_atlas.Sample(u_samp, input.uv)) * 255.0 + 0.5);
    float4 color = floor(saturate(input.color) * 255.0 + 0.5);
    float alpha = floor(color.a * coverage / 255.0);
    float3 premul = floor(color.rgb * color.a / 255.0);
    float3 rgb = floor(premul * coverage / 255.0);
    return float4(rgb, alpha) / 255.0;
}
"#;

// 薄 RHI 的通用 sampled quad 像素着色器，顶点阶段复用 glyph 的 position/uv/color ABI。
const RHI_TEXTURED_PS_HLSL: &str = r#"
Texture2D u_tex : register(t0);
SamplerState u_samp : register(s0);

struct VSOut {
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD0;
    float4 color : COLOR0;
};

float4 PSMain(VSOut input) : SV_Target
{
    float4 sample = u_tex.Sample(u_samp, input.uv);
    float4 tint = saturate(input.color);
    return float4(sample.rgb * tint.rgb, sample.a * tint.a);
}
"#;

const GRADIENT_HLSL: &str = r#"
cbuffer GradCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_origin_edge_x; // xy = origin, zw = physical edge X
    float4 u_edge_y;        // xy = physical edge Y, zw = padding
    float4 u_color_a;
    float4 u_color_b;
    // x = mode (0=linear, 1=radial)
    // y = linear dir (0..3) OR radial inner radius in local space
    // z = radial outer radius in local space OR linear edge X length
    // w = linear edge Y length
    float4 u_params;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 local : TEXCOORD0;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    float2 origin = u_origin_edge_x.xy;
    float2 edge_x = u_origin_edge_x.zw;
    float2 edge_y = u_edge_y.xy;
    float2 pos = origin + input.pos.x * edge_x + input.pos.y * edge_y;
    float2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    o.local = input.pos;
    return o;
}

float4 PSMain(VSOut input) : SV_Target
{
    float mode = u_params.x;
    if (mode < 0.5)
    {
        // Linear — matches CPU fill_linear_gradient t calculation.
        float dir = u_params.y;
        float t;
        float2 local = input.local;
        float2 size = float2(u_params.z, u_params.w);
        if (dir < 0.5)
            t = local.x;
        else if (dir < 1.5)
            t = local.y;
        else if (dir < 2.5)
            t = (local.x * size.x + local.y * size.y) / max(size.x + size.y, 1e-6);
        else
            t = (local.x * size.x - local.y * size.y + size.y) / max(size.x + size.y, 1e-6);
        t = saturate(t);
        return lerp(u_color_a, u_color_b, t);
    }
    else
    {
        // Radial — local space is the original circle's normalized square.
        float2 center = float2(0.5, 0.5);
        float dist = length(input.local - center);
        float outer_r = u_params.z;
        float inner_r = u_params.y;
        if (dist > outer_r)
            discard;
        float range = max(outer_r - inner_r, 1e-6);
        float t = saturate((dist - inner_r) / range);
        return lerp(u_color_a, u_color_b, t);
    }
}
"#;

const MESH_HLSL: &str = r#"
cbuffer MeshCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    float4 u_color;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    float2 ndc = (input.pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    return o;
}

float4 PSMain(VSOut input) : SV_Target
{
    return u_color;
}
"#;

// Blur pass 使用与共享 Blur ABI 对齐的可分离高斯采样 shader。
const BLUR_HLSL: &str = r#"
Texture2D u_tex : register(t0);
SamplerState u_samp : register(s0);

cbuffer BlurCB : register(b0)
{
    // xy = target viewport size; zw = source texture size (physical px)
    float4 u_sizes;
    // xy = target region origin; zw = region size (physical px)
    float4 u_region;
    // xy = pixel sampling direction; z = tap radius (taps = 2r+1)
    float4 u_dir_taps;
    // Gaussian weights ordered [-r..+r], zero-terminated, max 64 taps
    float4 u_weights[16];
};

struct VSIn { float2 pos : POSITION; };
struct VSOut { float4 pos : SV_POSITION; float2 uv : TEXCOORD0; };

VSOut VSMain(VSIn input)
{
    VSOut o;
    o.pos = float4(input.pos, 0.0, 1.0);
    // D3D texture (0,0) is top-left; map the region to the source UV space.
    float2 unit = float2(input.pos.x * 0.5 + 0.5, 0.5 - input.pos.y * 0.5);
    o.uv = (u_region.xy + unit * u_region.zw) / u_sizes.zw;
    return o;
}

// Separable Gaussian: one pass samples along u_dir_taps.xy.
float4 PSMain(VSOut input) : SV_TARGET
{
    float2 step = u_dir_taps.xy / u_sizes.zw;
    int radius = (int)u_dir_taps.z;
    float4 color = 0;
    for (int i = 0; i < 64; ++i)
    {
        float w = u_weights[i / 4][i % 4];
        if (w <= 0.0) break;
        float2 off = step * (float)(i - radius);
        color += w * u_tex.Sample(u_samp, input.uv + off);
    }
    return color;
}
"#;

const SHADOW_HLSL: &str = r#"
cbuffer ShadowCB : register(b0)
{
    float2 u_viewport;
    float2 _pad0;
    // Shadow expanded quad origin and x edge in device coordinates.
    float4 u_rect;
    float4 u_color;
    float4 u_radius;
    // xy = y edge, zw = x/y blur in device coordinates.
    float4 u_params;
    // xy = shadow body size, z = ambient flag, w unused.
    float4 u_size;
};

struct VSIn {
    float2 pos : POSITION;
};

struct VSOut {
    float4 pos : SV_POSITION;
    float2 local : TEXCOORD0;
    float2 rect_size : TEXCOORD1;
};

VSOut VSMain(VSIn input)
{
    VSOut o;
    float2 blur = max(u_params.zw, 0.0);
    float2 body_size = max(u_size.xy, float2(0.0001, 0.0001));
    float2 expanded_size = body_size + 2.0 * blur;
    float2 axis_x = u_rect.zw / max(expanded_size.x, 0.0001);
    float2 axis_y = u_params.xy / max(expanded_size.y, 0.0001);
    float2 draw_xy = u_rect.xy - axis_x - axis_y;
    float2 draw_edge_x = u_rect.zw + axis_x * 2.0;
    float2 draw_edge_y = u_params.xy + axis_y * 2.0;
    float2 pos = draw_xy + input.pos.x * draw_edge_x + input.pos.y * draw_edge_y;
    float2 ndc = (pos / u_viewport) * 2.0 - 1.0;
    ndc.y = -ndc.y;
    o.pos = float4(ndc, 0.0, 1.0);
    o.local = input.pos * (expanded_size + 2.0);
    o.rect_size = body_size;
    return o;
}

float rounded_rect_sdf(float2 local, float2 size, float4 radius)
{
    float2 half_size = size * 0.5;
    float2 q = local - half_size;
    float cr;
    if (q.x < 0.0)
        cr = (q.y < 0.0) ? radius.x : radius.w;
    else
        cr = (q.y < 0.0) ? radius.y : radius.z;
    float2 d = abs(q) - half_size + cr;
    float outside = length(max(d, 0.0));
    float inside = min(max(d.x, d.y), 0.0);
    return outside + inside - cr;
}

float shadow_coverage(float sd, float blur)
{
    float t = saturate((blur - sd) / (2.0 * blur));
    return t * t * (3.0 - 2.0 * t);
}

float shadow_coverage_ambient(float sd, float blur)
{
    float halfb = blur * 0.5;
    float t = saturate((halfb - sd) / (blur + halfb));
    float t2 = t * t;
    return t2 * t2 * (5.0 - 4.0 * t);
}

float4 PSMain(VSOut input) : SV_Target
{
    float2 blur = max(u_params.zw, 0.0);
    float blur_radius = max(blur.x, blur.y);
    float2 shape_local = input.local - blur - 1.0;
    float sd = rounded_rect_sdf(shape_local, input.rect_size, u_radius);
    float coverage;
    if (blur_radius > 0.5)
    {
        if (u_size.z > 0.5)
            coverage = shadow_coverage_ambient(sd, blur_radius);
        else
            coverage = shadow_coverage(sd, blur_radius);
    }
    else
    {
        // Matches CPU sdf_to_coverage when blur is negligible.
        coverage = saturate(0.5 - sd);
    }
    if (coverage <= 0.0)
        discard;
    // Straight-alpha for SRC_ALPHA blend (matches glyph / rounded-rect path).
    return float4(u_color.rgb, u_color.a * coverage);
}
"#;

fn d3d_error(operation: &str, err: ::windows::core::Error) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("D3d11Pipeline: {operation} failed: {err}"),
    )
}

fn compile_shader(source: &str, entry: &CStr, target: &CStr) -> Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    // SAFETY: source/entry/target 指针在同步编译期间有效且长度准确，两个输出槽完整初始化并由 COM 接管。
    let hr = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            PCSTR::null(),
            None,
            None,
            PCSTR::from_raw(entry.as_ptr().cast()),
            PCSTR::from_raw(target.as_ptr().cast()),
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if hr.is_err() {
        let detail = errors
            .as_ref()
            .map(|blob| {
                // SAFETY: blob 是 D3DCompile 返回的存活 ID3DBlob，指针在 blob 存活期间有效。
                let ptr = unsafe { blob.GetBufferPointer() } as *const u8;
                // SAFETY: 同一存活 blob 返回与上方指针对应的字节长度。
                let len = unsafe { blob.GetBufferSize() };
                if ptr.is_null() || len == 0 {
                    return String::new();
                }
                // SAFETY: ptr 已校验非空且 blob 保持存活，GetBufferSize 保证该区域覆盖 len 字节。
                let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
                String::from_utf8_lossy(bytes).into_owned()
            })
            .unwrap_or_default();
        return Err(Error::new(
            Errc::PlatformError,
            format!("D3d11Pipeline: D3DCompile {entry:?}/{target:?} failed: {detail}"),
        ));
    }
    code.ok_or_else(|| {
        Error::new(
            Errc::PlatformError,
            format!("D3d11Pipeline: D3DCompile {entry:?}/{target:?} returned no blob"),
        )
    })
}

// D3D11 着色管线只由 crate 内部上下文创建和持有。
pub(crate) struct D3d11Pipeline {
    vs_rect: ID3D11VertexShader,
    ps_rect: ID3D11PixelShader,
    layout: ID3D11InputLayout,
    // Blur pass 必须使用读取 BlurCB 的专用 VS，不能复用 BlitCB ABI。
    vs_blur: ID3D11VertexShader,
    /// 可分离高斯模糊像素着色器。
    ps_blur: ID3D11PixelShader,
    vs_glyph: ID3D11VertexShader,
    ps_glyph: ID3D11PixelShader,
    // 薄 RHI 的 RGBA8 MSDF 字形像素着色器。
    ps_msdf: ID3D11PixelShader,
    // 薄 RHI 通用纹理 quad 的像素着色器。
    ps_rhi_textured: ID3D11PixelShader,
    layout_glyph: ID3D11InputLayout,
    vs_grad: ID3D11VertexShader,
    ps_grad: ID3D11PixelShader,
    vs_mesh: ID3D11VertexShader,
    ps_mesh: ID3D11PixelShader,
    vs_shadow: ID3D11VertexShader,
    ps_shadow: ID3D11PixelShader,
    // 薄 RHI 的原生扇形 VS/PS。
    vs_sector: ID3D11VertexShader,
    ps_sector: ID3D11PixelShader,
    blend_alpha: ID3D11BlendState,
    blend_premultiplied: ID3D11BlendState,
    // sampled Additive quad 使用源与目标都为 ONE 的 blend 状态。
    blend_additive: ID3D11BlendState,
    blend_replace: ID3D11BlendState,
    // 保存 blend、rasterizer 与输出合并阶段共同使用的共享采样覆盖语义。
    multisample_state: PipelineMultisampleState,
    // 保存由共享二维光栅状态创建的原生对象。
    rasterizer: ID3D11RasterizerState,
    // 保存 rasterizer 对象对应的共享创建语义。
    raster_state: PipelineRasterState,
    // 保存由共享关闭深度模板状态创建的原生对象。
    depth_stencil: ID3D11DepthStencilState,
    // 保存 depth-stencil 对象对应的共享创建语义。
    depth_stencil_state: PipelineDepthStencilState,
}

// 把 API 无关原语拓扑翻译为 D3D11 枚举。
fn d3d11_primitive_topology(topology: PipelinePrimitiveTopology) -> D3D_PRIMITIVE_TOPOLOGY {
    // 只映射共享层允许的封闭拓扑集合。
    match topology {
        // TriangleList 对应 D3D11 的独立三角形列表。
        PipelinePrimitiveTopology::TriangleList => D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
    }
}

// 把 API 无关采样覆盖状态翻译为 D3D11 输出掩码。
fn d3d11_sample_mask(state: PipelineMultisampleState) -> u32 {
    // 共享值对象已经穷尽定义每个变体的输出位集合。
    state.sample_mask()
}

// 验证 D3D11 固有颜色输出行为能够满足共享抖动语义。
fn d3d11_supports_dither_state(state: PipelineDitherState) -> bool {
    // D3D11 没有可配置 dither 开关，只接受共享层的禁用语义。
    matches!(state, PipelineDitherState::Disabled)
}

// 为所有 D3D11 draw 原语集中映射共享混合语义。
impl D3d11Pipeline {
    // 返回当前 pipeline 契约对应的原生 blend state。
    fn rhi_blend_state(&self, blend: PipelineBlend) -> &ID3D11BlendState {
        // 禁止各 shader helper 再维护一份状态选择逻辑。
        match blend {
            // straight-alpha 映射到 SRC_ALPHA SrcOver。
            PipelineBlend::StraightAlpha => &self.blend_alpha,
            // premultiplied-alpha 映射到 ONE SrcOver。
            PipelineBlend::PremultipliedAlpha => &self.blend_premultiplied,
            // Additive 映射到源目标均为 ONE。
            PipelineBlend::Additive => &self.blend_additive,
            // Replace 映射到关闭颜色混合的覆盖状态。
            PipelineBlend::Replace => &self.blend_replace,
        }
    }

    // 返回当前共享采样覆盖状态对应的 D3D11 输出掩码。
    fn rhi_sample_mask(&self) -> u32 {
        // 禁止 shader helper 继续写死全位掩码。
        d3d11_sample_mask(self.multisample_state)
    }

    // 在统一 draw 边界绑定由共享契约创建的二维固定状态。
    pub(crate) fn apply_rhi_fixed_state(
        // 借用当前 owner-thread immediate context。
        &self,
        // 借用当前 pipeline 所属的 D3D11 context。
        context: &ID3D11DeviceContext,
        // 接收 FramePlan pipeline 的共享原语拓扑。
        topology: PipelinePrimitiveTopology,
        // 接收 FramePlan pipeline 的共享采样覆盖状态。
        multisample: PipelineMultisampleState,
        // 接收 FramePlan pipeline 的共享颜色抖动状态。
        dither: PipelineDitherState,
        // 接收 FramePlan pipeline 的共享光栅状态。
        raster: PipelineRasterState,
        // 接收 FramePlan pipeline 的共享深度模板状态。
        depth_stencil: PipelineDepthStencilState,
    ) -> Result<()> {
        // 原生状态对象必须仍与 pipeline 创建时的共享语义一致。
        if !d3d11_supports_dither_state(dither)
            || multisample != self.multisample_state
            || raster != self.raster_state
            || depth_stencil != self.depth_stencil_state
        {
            // 拒绝把未来新增状态错误映射为当前唯一二维对象。
            return Err(Error::new(
                // 状态身份错配属于稳定参数错误。
                Errc::InvalidArgument,
                // 保留不暴露平台对象的稳定诊断。
                "D3d11Pipeline: fixed state is not created",
            ));
        }
        // SAFETY：两个状态对象由同一 device 创建并在当前 owner thread 存活。
        unsafe {
            // 在所有 shader helper 之前统一绑定共享原语拓扑。
            context.IASetPrimitiveTopology(d3d11_primitive_topology(topology));
            // 在所有 shader helper 之前统一绑定共享 rasterizer。
            context.RSSetState(&self.rasterizer);
            // 显式覆盖 D3D11 的默认深度开启状态。
            context.OMSetDepthStencilState(&self.depth_stencil, 0);
        }
        // 固定状态绑定成功。
        Ok(())
    }
}

mod pipeline;
// 将 MSDF shader 源拆出，保持 pipeline 主模块不超过文件行数边界。
mod msdf_shader;
mod rhi_blur;
mod rhi_gradient;
// 保留只接受薄 RHI packet 的实心网格编码。
mod rhi_mesh;
mod rhi_shadow;
mod rhi_shape;
// 将扇形 shader 与 draw ABI 拆到独立文件，保持 pipeline 主模块边界清晰。
mod rhi_sector;
mod rhi_textured;

// 让 pipeline 构造模块复用 MSDF shader 源而不暴露原生 shader 对象。
pub(super) use msdf_shader::MSDF_GLYPH_HLSL;
