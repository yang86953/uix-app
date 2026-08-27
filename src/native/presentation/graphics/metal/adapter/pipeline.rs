//! Metal pipeline、MSL library 与共享 RHI 固定状态映射。

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlendFactor, MTLBlendOperation, MTLColorWriteMask, MTLDevice, MTLLibrary, MTLPixelFormat,
    MTLPrimitiveTopologyClass, MTLRenderPipelineDescriptor, MTLRenderPipelineState,
    MTLVertexDescriptor, MTLVertexFormat, MTLVertexStepFunction,
};

use crate::core::{Errc, Error, Result};
use crate::platform::presentation::rhi::{
    PipelineBlendFactor, PipelineDesc, PipelineKind, PipelineVertexFormat, TextureFormat,
};

const MSL_SOURCE: &str = include_str!("shaders.metal");

// 同一共享 pipeline 必须为两种可渲染纹理格式物化独立 Metal PSO。
pub(super) struct MetalPipelineResource {
    bgra: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    rgba: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
}

impl MetalPipelineResource {
    // 目标格式由当前 pass 的真实资源描述决定，禁止沿用历史 PSO。
    pub(super) fn for_format(
        &self,
        format: TextureFormat,
    ) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
        match format {
            TextureFormat::Bgra8Unorm => Ok(self.bgra.clone()),
            TextureFormat::Rgba8Unorm => Ok(self.rgba.clone()),
            TextureFormat::R8Unorm => Err(Error::new(
                Errc::InvalidArgument,
                "Metal RHI cannot render into an R8 texture",
            )),
        }
    }
}

// 编译一次内置 MSL；运行时编译错误保留为 typed 平台失败。
pub(super) fn compile_library(
    device: &ProtocolObject<dyn MTLDevice>,
) -> Result<Retained<ProtocolObject<dyn MTLLibrary>>> {
    let source = NSString::from_str(MSL_SOURCE);
    device
        .newLibraryWithSource_options_error(&source, None)
        .map_err(|error| metal_error("compile built-in MSL library", &error))
}

// 为共享描述创建 BGRA/RGBA 两个格式变体。
pub(super) fn create_pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
    library: &ProtocolObject<dyn MTLLibrary>,
    desc: PipelineDesc,
) -> Result<MetalPipelineResource> {
    let (vertex_name, fragment_name) = shader_entries(desc.kind);
    let bgra = create_pipeline_state(
        device,
        library,
        desc.kind,
        vertex_name,
        fragment_name,
        MTLPixelFormat::BGRA8Unorm,
    )?;
    let rgba = create_pipeline_state(
        device,
        library,
        desc.kind,
        vertex_name,
        fragment_name,
        MTLPixelFormat::RGBA8Unorm,
    )?;
    Ok(MetalPipelineResource { bgra, rgba })
}

// 局部清理同样按目标格式创建 PSO，但不读取顶点资源。
pub(super) fn create_clear_pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
    library: &ProtocolObject<dyn MTLLibrary>,
    format: TextureFormat,
) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
    let pixel_format = texture_pixel_format(format)?;
    create_pipeline_state_raw(
        device,
        library,
        "clear_vs",
        "clear_fs",
        pixel_format,
        None,
        None,
    )
}

fn create_pipeline_state(
    device: &ProtocolObject<dyn MTLDevice>,
    library: &ProtocolObject<dyn MTLLibrary>,
    kind: PipelineKind,
    vertex_name: &str,
    fragment_name: &str,
    pixel_format: MTLPixelFormat,
) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
    create_pipeline_state_raw(
        device,
        library,
        vertex_name,
        fragment_name,
        pixel_format,
        Some(kind.contract().vertex),
        Some(kind),
    )
}

fn create_pipeline_state_raw(
    device: &ProtocolObject<dyn MTLDevice>,
    library: &ProtocolObject<dyn MTLLibrary>,
    vertex_name: &str,
    fragment_name: &str,
    pixel_format: MTLPixelFormat,
    vertex_layout: Option<crate::platform::presentation::rhi::PipelineVertexLayout>,
    kind: Option<PipelineKind>,
) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
    let vertex_name = NSString::from_str(vertex_name);
    let fragment_name = NSString::from_str(fragment_name);
    let vertex = library
        .newFunctionWithName(&vertex_name)
        .ok_or_else(|| Error::new(Errc::PlatformError, "Metal MSL vertex entry is missing"))?;
    let fragment = library
        .newFunctionWithName(&fragment_name)
        .ok_or_else(|| Error::new(Errc::PlatformError, "Metal MSL fragment entry is missing"))?;
    let descriptor = MTLRenderPipelineDescriptor::new();
    descriptor.setVertexFunction(Some(&vertex));
    descriptor.setFragmentFunction(Some(&fragment));
    descriptor.setRasterSampleCount(1);
    // SAFETY: 当前闭集只包含 triangle-list pipeline。
    unsafe {
        descriptor.setInputPrimitiveTopology(MTLPrimitiveTopologyClass::Triangle);
    }
    if let Some(layout) = vertex_layout {
        let vertex_descriptor = metal_vertex_descriptor(layout)?;
        descriptor.setVertexDescriptor(Some(&vertex_descriptor));
    }
    let attachments = descriptor.colorAttachments();
    // SAFETY: Metal render pipeline 固定拥有至少八个颜色附件描述槽，索引零有效。
    let attachment = unsafe { attachments.objectAtIndexedSubscript(0) };
    attachment.setPixelFormat(pixel_format);
    if let Some(kind) = kind {
        configure_blend(&attachment, kind);
    } else {
        attachment.setBlendingEnabled(false);
        attachment.setWriteMask(MTLColorWriteMask::All);
    }
    device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .map_err(|error| metal_error("create render pipeline state", &error))
}

fn metal_vertex_descriptor(
    layout: crate::platform::presentation::rhi::PipelineVertexLayout,
) -> Result<Retained<MTLVertexDescriptor>> {
    let descriptor = MTLVertexDescriptor::new();
    let attributes = descriptor.attributes();
    for attribute in layout.attributes() {
        // SAFETY: 共享布局 location 受固定三槽闭集约束。
        let native = unsafe { attributes.objectAtIndexedSubscript(attribute.location() as usize) };
        native.setFormat(match attribute.format() {
            PipelineVertexFormat::Float32 => MTLVertexFormat::Float,
            PipelineVertexFormat::Float32x2 => MTLVertexFormat::Float2,
            PipelineVertexFormat::Float32x4 => MTLVertexFormat::Float4,
        });
        // SAFETY: 共享偏移和 buffer 槽位已经落在 Metal NSUInteger 值域。
        unsafe {
            native.setOffset(attribute.offset_bytes() as usize);
            native.setBufferIndex(0);
        }
    }
    let layouts = descriptor.layouts();
    // SAFETY: 所有现有顶点输入固定使用 buffer 槽零。
    let native_layout = unsafe { layouts.objectAtIndexedSubscript(0) };
    // SAFETY: 共享顶点步长是已验证的非零 u32。
    unsafe {
        native_layout.setStride(layout.stride_bytes() as usize);
        native_layout.setStepRate(1);
    }
    native_layout.setStepFunction(MTLVertexStepFunction::PerVertex);
    Ok(descriptor)
}

fn configure_blend(
    attachment: &objc2_metal::MTLRenderPipelineColorAttachmentDescriptor,
    kind: PipelineKind,
) {
    let blend = kind.contract().blend.state();
    attachment.setBlendingEnabled(blend.enabled);
    attachment.setSourceRGBBlendFactor(blend_factor(blend.source_color));
    attachment.setDestinationRGBBlendFactor(blend_factor(blend.destination_color));
    attachment.setSourceAlphaBlendFactor(blend_factor(blend.source_alpha));
    attachment.setDestinationAlphaBlendFactor(blend_factor(blend.destination_alpha));
    attachment.setRgbBlendOperation(MTLBlendOperation::Add);
    attachment.setAlphaBlendOperation(MTLBlendOperation::Add);
    attachment.setWriteMask(MTLColorWriteMask::All);
}

const fn blend_factor(factor: PipelineBlendFactor) -> MTLBlendFactor {
    match factor {
        PipelineBlendFactor::Zero => MTLBlendFactor::Zero,
        PipelineBlendFactor::One => MTLBlendFactor::One,
        PipelineBlendFactor::SourceAlpha => MTLBlendFactor::SourceAlpha,
        PipelineBlendFactor::OneMinusSourceAlpha => MTLBlendFactor::OneMinusSourceAlpha,
    }
}

const fn shader_entries(kind: PipelineKind) -> (&'static str, &'static str) {
    match kind {
        PipelineKind::SolidMesh => ("solid_vs", "solid_fs"),
        PipelineKind::TexturedQuad | PipelineKind::TexturedQuadAdditive => {
            ("sampled_vs", "textured_fs")
        }
        PipelineKind::GradientRect => ("gradient_vs", "gradient_fs"),
        PipelineKind::GlyphCoverageQuad => ("sampled_vs", "coverage_fs"),
        PipelineKind::ShapeRect | PipelineKind::ShapeRectAdditive => ("shape_vs", "shape_fs"),
        PipelineKind::BoxShadow => ("shadow_vs", "shadow_fs"),
        PipelineKind::BlurPass => ("blur_vs", "blur_fs"),
        PipelineKind::MsdfGlyphQuad => ("sampled_vs", "msdf_fs"),
        PipelineKind::Sector => ("sector_vs", "sector_fs"),
        PipelineKind::LineSegment => ("line_vs", "line_fs"),
    }
}

pub(super) const fn texture_pixel_format(format: TextureFormat) -> Result<MTLPixelFormat> {
    Ok(match format {
        TextureFormat::Bgra8Unorm => MTLPixelFormat::BGRA8Unorm,
        TextureFormat::Rgba8Unorm => MTLPixelFormat::RGBA8Unorm,
        TextureFormat::R8Unorm => MTLPixelFormat::R8Unorm,
    })
}

fn metal_error(operation: &'static str, error: &objc2_foundation::NSError) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("Metal {operation} failed: {error:?}"),
    )
}
