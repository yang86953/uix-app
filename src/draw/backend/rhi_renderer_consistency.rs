//! Drawing System 拥有的 API 无关全图元一致性规范场景。
//!
//! 本模块只冻结场景输入、采样点、期望和容差；它不创建原生资源，也不实现
//! 任一生产光栅算法。Vulkan、D3D11 与 OpenGL 的 GPU harness 必须复用这里的事实。

use super::{FrameUniformPayload, FrameVertexPayload, RhiOp, RhiRenderer};
use crate::draw::backend::gpu::pending::PendingNativeOp;
use crate::platform::presentation::rhi::{
    BLUR_WEIGHT_COUNT, PipelineKind, PipelineSampling, RhiBlurRasterParams, RhiExtent,
    RhiGradientRasterParams, RhiMeshRasterParams, RhiMsdfRasterParams, RhiSampledRasterParams,
    RhiScissor, RhiSectorRasterParams, RhiShadowRasterParams, RhiShapeRasterParams, RhiViewport,
    SamplerDesc, TextureFormat,
};

// 使用三行四列固定画布隔离十一类 pipeline，同时只需一次真实 GPU 提交和回读。
pub(crate) const CONSISTENCY_EXTENT: RhiExtent = RhiExtent::new(64, 48);
// 所有场景共享一个不透明背景，便于同时区分 SrcOver、Additive、裁剪和 discard。
pub(crate) const CONSISTENCY_BACKGROUND: [u8; 4] = [16, 32, 48, 255];
// 当前 PipelineKind 的完整闭集；scene_for_pipeline 的穷尽 match 是新增变体门禁。
pub(crate) const CONSISTENCY_PIPELINES: [PipelineKind; 11] = [
    PipelineKind::SolidMesh,
    PipelineKind::TexturedQuad,
    PipelineKind::GradientRect,
    PipelineKind::GlyphCoverageQuad,
    PipelineKind::ShapeRect,
    PipelineKind::ShapeRectAdditive,
    PipelineKind::BoxShadow,
    PipelineKind::TexturedQuadAdditive,
    PipelineKind::BlurPass,
    PipelineKind::MsdfGlyphQuad,
    PipelineKind::Sector,
];

// 标记规范场景在 Canvas/RHI lowering 中的生产来源，避免把 Blur 伪装成 PendingNativeOp。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConsistencySource {
    // 场景由 PendingNativeOp 经 RhiOp 到达固定 pipeline。
    PendingAndRhi,
    // Blur 由 Drawing backdrop blur Module 直接建立离屏 pass。
    DrawingBlur,
}

// 描述 sampled pipeline 的 API 无关源纹理与过滤契约。
#[derive(Debug, Clone)]
pub(crate) struct ConsistencyTexture {
    pub(crate) extent: RhiExtent,
    pub(crate) format: TextureFormat,
    pub(crate) bytes: Vec<u8>,
    pub(crate) sampler: SamplerDesc,
}

// 一个采样点允许逐通道给出闭区间，并明确该类语义采用的最大容差。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConsistencyTolerance {
    // 清屏、裁剪和 discard 必须逐字节精确。
    Exact,
    // UNORM 量化与固定混合允许一个通道值误差。
    Quantized,
    // 线性过滤、渐变或卷积允许两个通道值误差。
    Filtered,
    // 分析 coverage 边界只使用稳定区间，最多放宽五个通道值。
    Analytic,
}

impl ConsistencyTolerance {
    pub(crate) const fn amount(self) -> u8 {
        match self {
            Self::Exact => 0,
            Self::Quantized => 1,
            Self::Filtered => 2,
            Self::Analytic => 5,
        }
    }
}

// 一个采样点允许逐通道给出闭区间，并引用统一的输出类型容差组。
#[derive(Debug, Clone, Copy)]
pub(crate) struct ConsistencySample {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) minimum: [u8; 4],
    pub(crate) maximum: [u8; 4],
    pub(crate) tolerance: ConsistencyTolerance,
    pub(crate) semantic: &'static str,
}

impl ConsistencySample {
    // 构造逐通道绝对误差相同的精确像素期望。
    fn exact(
        x: u32,
        y: u32,
        rgba: [u8; 4],
        tolerance: ConsistencyTolerance,
        semantic: &'static str,
    ) -> Self {
        let amount = tolerance.amount();
        Self {
            x,
            y,
            minimum: rgba.map(|channel| channel.saturating_sub(amount)),
            maximum: rgba.map(|channel| channel.saturating_add(amount)),
            tolerance,
            semantic,
        }
    }

    // 构造只要求稳定语义区间的分析抗锯齿采样点。
    fn range(
        x: u32,
        y: u32,
        minimum: [u8; 4],
        maximum: [u8; 4],
        tolerance: ConsistencyTolerance,
        semantic: &'static str,
    ) -> Self {
        Self {
            x,
            y,
            minimum,
            maximum,
            tolerance,
            semantic,
        }
    }

    // 由所有原生 harness 共用同一判定逻辑，禁止各 API 私设容差。
    pub(crate) fn accepts(self, actual: [u8; 4]) -> bool {
        actual
            .into_iter()
            .zip(self.minimum)
            .zip(self.maximum)
            .all(|((value, minimum), maximum)| value >= minimum && value <= maximum)
    }
}

// 冻结一个 pipeline 的实际 draw 输入和回读断言。
#[derive(Debug, Clone)]
pub(crate) struct ConsistencyScene {
    pub(crate) name: &'static str,
    pub(crate) kind: PipelineKind,
    pub(crate) source: ConsistencySource,
    pub(crate) vertex: FrameVertexPayload,
    pub(crate) uniform: FrameUniformPayload,
    pub(crate) texture: Option<ConsistencyTexture>,
    pub(crate) scissor: Option<RhiScissor>,
    pub(crate) samples: Vec<ConsistencySample>,
}

// 返回十一类 pipeline 的唯一规范场景；顺序同时固定真实 GPU 诊断输出。
pub(crate) fn canonical_scenes() -> [ConsistencyScene; 11] {
    CONSISTENCY_PIPELINES.map(scene_for_pipeline)
}

// 在任一原生 harness 执行前验证闭集唯一性、共享 ABI、采样资源和采样点边界。
pub(crate) fn validate_canonical_scenes(scenes: &[ConsistencyScene]) -> Result<(), &'static str> {
    if scenes.len() != CONSISTENCY_PIPELINES.len() {
        return Err("canonical scene count does not match PipelineKind coverage");
    }
    for (index, scene) in scenes.iter().enumerate() {
        if scenes[..index]
            .iter()
            .any(|previous| previous.kind == scene.kind)
        {
            return Err("canonical PipelineKind scene is duplicated");
        }
        let contract = scene.kind.contract();
        if scene.vertex.layout() != contract.vertex
            || scene.uniform.layout() != contract.uniform
            || !scene.vertex.is_valid()
            || !scene.uniform.is_valid()
            || scene.samples.is_empty()
        {
            return Err("canonical scene does not match its shared ABI");
        }
        if scene.samples.iter().any(|sample| {
            sample.x >= CONSISTENCY_EXTENT.width
                || sample.y >= CONSISTENCY_EXTENT.height
                || sample.tolerance.amount() > ConsistencyTolerance::Analytic.amount()
        }) {
            return Err("canonical sample or tolerance is outside the shared boundary");
        }
        match (&scene.texture, contract.sampling) {
            (None, PipelineSampling::None) => {}
            (Some(texture), sampling)
                if sampling.accepts(texture.format, texture.sampler)
                    && texture.bytes.len()
                        == texture.extent.width as usize
                            * texture.extent.height as usize
                            * texture.format.bytes_per_pixel() => {}
            _ => return Err("canonical sampled resource does not match PipelineContract"),
        }
    }
    if scenes
        .iter()
        .filter(|scene| scene.source == ConsistencySource::DrawingBlur)
        .any(|scene| scene.kind != PipelineKind::BlurPass)
    {
        return Err("only BlurPass may bypass PendingNativeOp and RhiOp");
    }
    Ok(())
}

// 穷尽映射 PipelineKind；新增变体若没有一致性场景，测试构建直接编译失败。
fn scene_for_pipeline(kind: PipelineKind) -> ConsistencyScene {
    match kind {
        PipelineKind::SolidMesh => solid_mesh_scene(),
        PipelineKind::TexturedQuad => textured_scene(false),
        PipelineKind::GradientRect => gradient_scene(),
        PipelineKind::GlyphCoverageQuad => coverage_scene(),
        PipelineKind::ShapeRect => shape_scene(false),
        PipelineKind::ShapeRectAdditive => shape_scene(true),
        PipelineKind::BoxShadow => shadow_scene(),
        PipelineKind::TexturedQuadAdditive => textured_scene(true),
        PipelineKind::BlurPass => blur_scene(),
        PipelineKind::MsdfGlyphQuad => msdf_scene(),
        PipelineKind::Sector => sector_scene(),
    }
}

// PendingNativeOp 的穷尽路由门禁；ScrollCopy 明确是复制语义而不是缺失 pipeline。
#[allow(dead_code)]
pub(crate) fn pending_pipeline_route(operation: &PendingNativeOp) -> Option<PipelineKind> {
    match operation {
        PendingNativeOp::SolidRect(rect) => Some(if rect.additive {
            PipelineKind::ShapeRectAdditive
        } else {
            PipelineKind::ShapeRect
        }),
        PendingNativeOp::StrokeRect(rect) => Some(if rect.additive {
            PipelineKind::ShapeRectAdditive
        } else {
            PipelineKind::ShapeRect
        }),
        PendingNativeOp::Glyph(glyph) => Some(if glyph.glyph.outline_mesh.is_some() {
            PipelineKind::MsdfGlyphQuad
        } else {
            PipelineKind::GlyphCoverageQuad
        }),
        PendingNativeOp::LinearGradient(_) | PendingNativeOp::RadialGradient(_) => {
            Some(PipelineKind::GradientRect)
        }
        PendingNativeOp::Sector(_) => Some(PipelineKind::Sector),
        PendingNativeOp::SolidMesh(_) => Some(PipelineKind::SolidMesh),
        PendingNativeOp::BoxShadow(_) => Some(PipelineKind::BoxShadow),
        PendingNativeOp::ImageBlit(image) => Some(if image.blit.additive {
            PipelineKind::TexturedQuadAdditive
        } else {
            PipelineKind::TexturedQuad
        }),
        PendingNativeOp::ScrollCopy(_) => None,
    }
}

// RhiOp 的穷尽路由门禁；Blur 不属于混合 RhiOp，由独立 DrawingBlur 场景覆盖。
#[allow(dead_code)]
pub(crate) fn rhi_pipeline_route(operation: &RhiOp) -> PipelineKind {
    match operation {
        RhiOp::Solid(_) => PipelineKind::SolidMesh,
        RhiOp::Textured(quad) => sampled_pipeline(quad.additive),
        RhiOp::Sampled(quad) => sampled_pipeline(quad.additive),
        RhiOp::Coverage(_) => PipelineKind::GlyphCoverageQuad,
        RhiOp::Msdf(_) => PipelineKind::MsdfGlyphQuad,
        RhiOp::Gradient(_) => PipelineKind::GradientRect,
        RhiOp::Shape(_) => PipelineKind::ShapeRect,
        RhiOp::AdditiveShape(_) => PipelineKind::ShapeRectAdditive,
        RhiOp::Sector(_) => PipelineKind::Sector,
        RhiOp::Shadow(_) => PipelineKind::BoxShadow,
    }
}

fn sampled_pipeline(additive: bool) -> PipelineKind {
    if additive {
        PipelineKind::TexturedQuadAdditive
    } else {
        PipelineKind::TexturedQuad
    }
}

fn viewport() -> RhiViewport {
    RhiViewport {
        width: CONSISTENCY_EXTENT.width as f32,
        height: CONSISTENCY_EXTENT.height as f32,
    }
}

fn background(x: u32, y: u32, semantic: &'static str) -> ConsistencySample {
    ConsistencySample::exact(
        x,
        y,
        CONSISTENCY_BACKGROUND,
        ConsistencyTolerance::Exact,
        semantic,
    )
}

fn rect_vertices(x: f32, y: f32, width: f32, height: f32) -> FrameVertexPayload {
    FrameVertexPayload::position_f32x2([
        x,
        y,
        x + width,
        y,
        x + width,
        y + height,
        x,
        y,
        x + width,
        y + height,
        x,
        y + height,
    ])
}

fn sampled_vertices(x: f32, y: f32, width: f32, height: f32) -> FrameVertexPayload {
    FrameVertexPayload::position_uv_color_f32(super::mixed::textured_vertices_values(
        [
            [x, y],
            [x + width, y],
            [x + width, y + height],
            [x, y + height],
        ],
        [1.0; 4],
        0.0,
        0.0,
        1.0,
        1.0,
    ))
}

fn solid_mesh_scene() -> ConsistencyScene {
    ConsistencyScene {
        name: "SolidMesh",
        kind: PipelineKind::SolidMesh,
        source: ConsistencySource::PendingAndRhi,
        vertex: rect_vertices(2.0, 2.0, 12.0, 12.0),
        uniform: FrameUniformPayload::Mesh(RhiMeshRasterParams::new(
            viewport(),
            [0.5, 0.25, 0.75, 0.5],
        )),
        texture: None,
        scissor: Some(RhiScissor {
            x: 2,
            y: 2,
            width: 8,
            height: 12,
        }),
        samples: vec![
            ConsistencySample::exact(
                5,
                5,
                [72, 48, 120, 255],
                ConsistencyTolerance::Quantized,
                "straight SrcOver",
            ),
            background(12, 5, "scissor clip"),
        ],
    }
}

fn textured_scene(additive: bool) -> ConsistencyScene {
    if additive {
        return ConsistencyScene {
            name: "TexturedQuadAdditive",
            kind: PipelineKind::TexturedQuadAdditive,
            source: ConsistencySource::PendingAndRhi,
            vertex: sampled_vertices(50.0, 18.0, 4.0, 4.0),
            uniform: FrameUniformPayload::Sampled(RhiSampledRasterParams::new(viewport())),
            texture: Some(ConsistencyTexture {
                extent: RhiExtent::new(1, 1),
                format: TextureFormat::Bgra8Unorm,
                // BGRA 存储对应 premultiplied RGBA [64, 32, 16, 128]。
                bytes: vec![16, 32, 64, 128],
                sampler: SamplerDesc::linear_clamp(),
            }),
            scissor: None,
            samples: vec![ConsistencySample::exact(
                51,
                19,
                [80, 64, 64, 255],
                ConsistencyTolerance::Quantized,
                "premultiplied Additive",
            )],
        };
    }
    ConsistencyScene {
        name: "TexturedQuad",
        kind: PipelineKind::TexturedQuad,
        source: ConsistencySource::PendingAndRhi,
        vertex: sampled_vertices(18.0, 2.0, 4.0, 4.0),
        uniform: FrameUniformPayload::Sampled(RhiSampledRasterParams::new(viewport())),
        texture: Some(ConsistencyTexture {
            extent: RhiExtent::new(2, 1),
            format: TextureFormat::Bgra8Unorm,
            // 两个半透明 premultiplied texel：红与蓝；缩放后必须线性过滤。
            bytes: vec![0, 0, 128, 128, 128, 0, 0, 128],
            sampler: SamplerDesc::linear_clamp(),
        }),
        scissor: Some(RhiScissor {
            x: 19,
            y: 2,
            width: 2,
            height: 4,
        }),
        samples: vec![
            ConsistencySample::exact(
                19,
                3,
                [104, 16, 56, 255],
                ConsistencyTolerance::Filtered,
                "linear filter left",
            ),
            ConsistencySample::exact(
                20,
                3,
                [40, 16, 120, 255],
                ConsistencyTolerance::Filtered,
                "linear filter right",
            ),
            background(18, 3, "sampled scissor clip"),
        ],
    }
}

fn gradient_scene() -> ConsistencyScene {
    let corners = [[34.0, 2.0], [42.0, 2.0], [46.0, 10.0], [38.0, 10.0]];
    ConsistencyScene {
        name: "GradientRect",
        kind: PipelineKind::GradientRect,
        source: ConsistencySource::PendingAndRhi,
        vertex: RhiRenderer::unit_quad_vertex_payload(),
        uniform: FrameUniformPayload::Gradient(RhiGradientRasterParams::new(
            viewport(),
            corners,
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 0.0],
        )),
        texture: None,
        scissor: None,
        samples: vec![
            ConsistencySample::exact(
                39,
                5,
                [135, 0, 120, 255],
                ConsistencyTolerance::Filtered,
                "affine gradient",
            ),
            background(36, 8, "affine quad outside"),
        ],
    }
}

fn coverage_scene() -> ConsistencyScene {
    ConsistencyScene {
        name: "GlyphCoverageQuad",
        kind: PipelineKind::GlyphCoverageQuad,
        source: ConsistencySource::PendingAndRhi,
        vertex: FrameVertexPayload::position_uv_color_f32(super::mixed::textured_vertices_values(
            [[50.0, 2.0], [54.0, 2.0], [54.0, 6.0], [50.0, 6.0]],
            [1.0, 0.5, 0.25, 0.8],
            0.0,
            0.0,
            1.0,
            1.0,
        )),
        uniform: FrameUniformPayload::Sampled(RhiSampledRasterParams::new(viewport())),
        texture: Some(ConsistencyTexture {
            extent: RhiExtent::new(2, 2),
            format: TextureFormat::R8Unorm,
            bytes: vec![0, 64, 128, 255],
            sampler: SamplerDesc::nearest_clamp(),
        }),
        scissor: None,
        samples: vec![
            background(50, 2, "zero coverage"),
            ConsistencySample::exact(
                53,
                2,
                [64, 51, 50, 255],
                ConsistencyTolerance::Quantized,
                "coverage 64",
            ),
            ConsistencySample::exact(
                50,
                5,
                [112, 70, 54, 255],
                ConsistencyTolerance::Quantized,
                "coverage 128",
            ),
            ConsistencySample::exact(
                53,
                5,
                [207, 108, 61, 255],
                ConsistencyTolerance::Quantized,
                "coverage 255",
            ),
        ],
    }
}

fn shape_scene(additive: bool) -> ConsistencyScene {
    let (name, kind, x, center, expected) = if additive {
        (
            "ShapeRectAdditive",
            PipelineKind::ShapeRectAdditive,
            18.0,
            (22, 22),
            [118, 83, 73, 255],
        )
    } else {
        (
            "ShapeRect",
            PipelineKind::ShapeRect,
            2.0,
            (6, 22),
            [110, 67, 49, 255],
        )
    };
    let mut samples = vec![ConsistencySample::exact(
        center.0,
        center.1,
        expected,
        ConsistencyTolerance::Quantized,
        if additive {
            "quantized premultiplied Additive"
        } else {
            "quantized premultiplied SrcOver"
        },
    )];
    if !additive {
        samples.push(background(2, 18, "rounded corner discard"));
        samples.push(background(13, 19, "shape outside"));
    }
    ConsistencyScene {
        name,
        kind,
        source: ConsistencySource::PendingAndRhi,
        vertex: RhiRenderer::unit_quad_vertex_payload(),
        uniform: FrameUniformPayload::Shape(RhiShapeRasterParams::new(
            viewport(),
            [x, 18.0, 10.0, 10.0],
            [0.8, 0.4, 0.2, 0.5],
            [3.0; 4],
            0.0,
        )),
        texture: None,
        scissor: None,
        samples,
    }
}

fn shadow_scene() -> ConsistencyScene {
    ConsistencyScene {
        name: "BoxShadow",
        kind: PipelineKind::BoxShadow,
        source: ConsistencySource::PendingAndRhi,
        vertex: RhiRenderer::unit_quad_vertex_payload(),
        uniform: FrameUniformPayload::Shadow(RhiShadowRasterParams::new(
            viewport(),
            [[35.0, 19.0], [45.0, 19.0], [45.0, 29.0], [35.0, 29.0]],
            [0.0, 0.0, 0.0, 0.5],
            [0.0; 4],
            [2.0, 2.0],
            [6.0, 6.0],
            false,
        )),
        texture: None,
        scissor: None,
        samples: vec![
            ConsistencySample::exact(
                40,
                24,
                [8, 16, 24, 255],
                ConsistencyTolerance::Filtered,
                "shadow body",
            ),
            ConsistencySample::range(
                36,
                24,
                [10, 20, 30, 255],
                [15, 30, 45, 255],
                ConsistencyTolerance::Analytic,
                "shadow soft edge",
            ),
            background(33, 24, "shadow outside"),
        ],
    }
}

fn blur_scene() -> ConsistencyScene {
    let full_region = RhiScissor {
        x: 0,
        y: 0,
        width: CONSISTENCY_EXTENT.width as i32,
        height: CONSISTENCY_EXTENT.height as i32,
    };
    let mut weights = [0.0f32; BLUR_WEIGHT_COUNT];
    weights[..3].copy_from_slice(&[0.25, 0.5, 0.25]);
    // 全目标代表避开既有三端子区域 UV 争议；恒定背景保证其它图元仍可独立覆盖。
    let pixel_count = CONSISTENCY_EXTENT.width as usize * CONSISTENCY_EXTENT.height as usize;
    let mut source = Vec::with_capacity(pixel_count * 4);
    for _ in 0..pixel_count {
        source.extend_from_slice(&CONSISTENCY_BACKGROUND);
    }
    let row_start = (34usize * CONSISTENCY_EXTENT.width as usize + 2) * 4;
    source[row_start..row_start + 16]
        .copy_from_slice(&[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 255]);
    ConsistencyScene {
        name: "BlurPass",
        kind: PipelineKind::BlurPass,
        source: ConsistencySource::DrawingBlur,
        vertex: super::blur::blur_region_vertices(CONSISTENCY_EXTENT, full_region),
        uniform: FrameUniformPayload::Blur(RhiBlurRasterParams::new(
            CONSISTENCY_EXTENT,
            CONSISTENCY_EXTENT,
            full_region,
            [1.0, 0.0],
            1,
            &weights,
        )),
        texture: Some(ConsistencyTexture {
            extent: CONSISTENCY_EXTENT,
            format: TextureFormat::Rgba8Unorm,
            bytes: source,
            sampler: SamplerDesc::linear_clamp(),
        }),
        scissor: None,
        samples: vec![
            ConsistencySample::exact(
                2,
                34,
                [132, 72, 12, 255],
                ConsistencyTolerance::Filtered,
                "blur neighborhood",
            ),
            ConsistencySample::exact(
                3,
                34,
                [64, 128, 64, 255],
                ConsistencyTolerance::Filtered,
                "blur convolution",
            ),
            ConsistencySample::exact(
                4,
                34,
                [0, 64, 128, 255],
                ConsistencyTolerance::Filtered,
                "blur direction",
            ),
        ],
    }
}

fn msdf_scene() -> ConsistencyScene {
    ConsistencyScene {
        name: "MsdfGlyphQuad",
        kind: PipelineKind::MsdfGlyphQuad,
        source: ConsistencySource::PendingAndRhi,
        vertex: FrameVertexPayload::position_uv_color_f32(super::mixed::textured_vertices_values(
            [[18.0, 34.0], [25.0, 34.0], [25.0, 38.0], [18.0, 38.0]],
            [1.0, 0.5, 0.25, 0.8],
            0.0,
            0.0,
            1.0,
            1.0,
        )),
        uniform: FrameUniformPayload::Msdf(RhiMsdfRasterParams::new(
            viewport(),
            RhiExtent::new(2, 1),
            4.0,
        )),
        texture: Some(ConsistencyTexture {
            extent: RhiExtent::new(2, 1),
            format: TextureFormat::Rgba8Unorm,
            bytes: vec![64, 64, 64, 255, 191, 191, 191, 255],
            sampler: SamplerDesc::linear_clamp(),
        }),
        scissor: None,
        samples: vec![
            ConsistencySample::exact(
                18,
                35,
                [207, 108, 61, 255],
                ConsistencyTolerance::Filtered,
                "MSDF inside",
            ),
            ConsistencySample::exact(
                21,
                35,
                [112, 70, 54, 255],
                ConsistencyTolerance::Analytic,
                "MSDF boundary",
            ),
            background(24, 35, "MSDF outside"),
        ],
    }
}

fn sector_scene() -> ConsistencyScene {
    ConsistencyScene {
        name: "Sector",
        kind: PipelineKind::Sector,
        source: ConsistencySource::PendingAndRhi,
        vertex: RhiRenderer::unit_quad_vertex_payload(),
        uniform: FrameUniformPayload::Sector(RhiSectorRasterParams::new(
            viewport(),
            [34.0, 34.0, 9.0, 9.0],
            [0.2, 0.8, 0.4, 0.75],
            [0.0, std::f32::consts::FRAC_PI_2],
        )),
        texture: None,
        scissor: None,
        samples: vec![
            ConsistencySample::exact(
                40,
                40,
                [42, 160, 88, 255],
                ConsistencyTolerance::Filtered,
                "sector interior",
            ),
            background(36, 40, "sector angular exterior"),
            background(42, 42, "sector radial exterior"),
        ],
    }
}

// 规范本身的闭集、ABI 与容差门禁放在根 tests 目录，供所有 API harness 共享。
#[cfg(test)]
#[path = "../../../tests/unit/draw/backend/rhi_renderer_consistency__tests.rs"]
mod tests;
