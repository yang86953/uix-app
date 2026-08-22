//! D3D11 native geometry pipeline — solid/stroke rects + glyph atlas +
//! path meshes + box shadow。
//!
//! 所有 D3D shader 语义源码来自 Windows 原生层共享的唯一 HLSL 支持模块。

#![allow(nonstandard_style)]

use std::ffi::CStr;

use crate::core::{Errc, Error, Result};
use crate::native::presentation::graphics::d3d_shader_source::{
    BLUR_HLSL, GLYPH_HLSL, GRADIENT_HLSL, MESH_HLSL, MSDF_GLYPH_HLSL, RECT_HLSL,
    RHI_TEXTURED_PS_HLSL, SHADOW_HLSL,
};
// 引入跨 Adapter 共享的 pipeline 固定状态语义。
use crate::platform::presentation::rhi::{
    IndexFormat, PIPELINE_DEPTH_STENCIL_DISABLED, PIPELINE_RASTER_2D, PipelineBlend,
    PipelineBlendFactor, PipelineBlendOperation, PipelineColorWriteMask, PipelineCullMode,
    PipelineDepthClip, PipelineDepthState, PipelineDepthStencilState, PipelineDitherState,
    PipelineFrontFace, PipelineMultisampleState, PipelinePrimitiveTopology, PipelineRasterState,
    PipelineStencilState, PipelineVertexFormat, PipelineVertexLayout, PipelineVertexSemantic,
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
    DXGI_FORMAT, DXGI_FORMAT_R32_UINT, DXGI_FORMAT_R32G32_FLOAT, DXGI_FORMAT_R32G32B32A32_FLOAT,
    DXGI_FORMAT_UNKNOWN,
};
use ::windows::core::{BOOL, PCSTR};

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
    // Blur pass 使用共享 position/uv-float4 输入布局。
    layout_blur: ID3D11InputLayout,
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

// 保存已经由共享格式映射完成的 D3D11 索引资源绑定。
pub(crate) struct D3d11IndexBinding {
    // 保持原生索引 buffer 在 draw 编码期间存活。
    native: ID3D11Buffer,
    // 保存与共享 IndexFormat 对应的 DXGI 枚举。
    format: DXGI_FORMAT,
}

// 为 D3D11 索引绑定提供唯一格式翻译入口。
impl D3d11IndexBinding {
    // 从已验证资源与共享格式构造原生绑定。
    pub(crate) fn new(native: ID3D11Buffer, format: IndexFormat) -> Self {
        // 穷尽映射共享层允许的索引格式。
        let format = match format {
            // Uint32 对应 D3D11 的 R32_UINT 索引解释。
            IndexFormat::Uint32 => DXGI_FORMAT_R32_UINT,
        };
        // 返回同时冻结 COM 生命周期与原生格式的绑定。
        Self { native, format }
    }
}

/// 绑定或清除一个已经完成共享格式映射的 D3D11 索引资源。
///
/// # Safety
/// 调用者必须保证 context 与可选 buffer 属于同一 owner-thread D3D11 device。
unsafe fn bind_rhi_index_buffer(
    // 借用当前 immediate context。
    context: &ID3D11DeviceContext,
    // 接收可选的类型化原生索引绑定。
    index: Option<&D3d11IndexBinding>,
) {
    // 把可选绑定投影为 D3D11 要求的资源与格式对。
    let (native, format) = match index {
        // 索引绘制同时绑定资源和已映射格式。
        Some(binding) => (Some(&binding.native), binding.format),
        // 非索引绘制清除旧资源并使用无格式占位。
        None => (None, DXGI_FORMAT_UNKNOWN),
    };
    // SAFETY：调用者保证 context 与 buffer 归属同一 owner-thread device。
    unsafe { context.IASetIndexBuffer(native, format, 0) };
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
mod rhi_blur;
mod rhi_gradient;
// 保留只接受薄 RHI packet 的实心网格编码。
mod rhi_mesh;
mod rhi_shadow;
mod rhi_shape;
// 将扇形 shader 与 draw ABI 拆到独立文件，保持 pipeline 主模块边界清晰。
mod rhi_sector;
mod rhi_textured;
