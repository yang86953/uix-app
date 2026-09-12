//! Drawing System 拥有的 API 无关全图元一致性规范场景。
//!
//! 本模块只冻结场景输入、采样点、期望和容差；它不创建原生资源，也不实现
//! 任一生产光栅算法。全部原生 GPU harness 必须复用这里的事实。

use super::{FrameUniformPayload, FrameVertexPayload, RhiOp, RhiRenderer};
use crate::core::Rect;
use crate::draw::Color;
use crate::draw::backend::gpu::pending::PendingNativeOp;
use crate::platform::presentation::rhi::{
    BLUR_WEIGHT_COUNT, PipelineKind, PipelineSampling, RhiBlurDirection, RhiBlurPassGeometry,
    RhiExtent, RhiGradientRasterParams, RhiMeshRasterParams, RhiMsdfRasterParams,
    RhiSampledRasterParams, RhiScissor, RhiSectorRasterParams, RhiShadowRasterParams,
    RhiShapeRasterParams, RhiTextureRegion, RhiViewport, SamplerDesc, TextureFormat,
};

// 使用四行四列固定画布隔离十二类 pipeline 与渐变边缘场景，同时只需一次真实 GPU 提交和回读。
pub(crate) const CONSISTENCY_EXTENT: RhiExtent = RhiExtent::new(64, 72);
// 所有场景共享一个不透明背景，便于同时区分 SrcOver、Additive、裁剪和 discard。
pub(crate) const CONSISTENCY_BACKGROUND: [u8; 4] = [16, 32, 48, 255];
// 当前 PipelineKind 的完整闭集；scene_for_pipeline 的穷尽 match 是新增变体门禁。
pub(crate) const CONSISTENCY_PIPELINES: [PipelineKind; 12] = [
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
    PipelineKind::LineSegment,
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

// 保存非零原点 Blur 子区域在水平与垂直两 pass 共用的规范输入和最终断言。
#[derive(Debug, Clone)]
pub(crate) struct ConsistencyBlurScenario {
    pub(crate) vertex: FrameVertexPayload,
    pub(crate) horizontal: FrameUniformPayload,
    pub(crate) vertical: FrameUniformPayload,
    pub(crate) texture: ConsistencyTexture,
    pub(crate) scissor: RhiScissor,
    pub(crate) horizontal_samples: Vec<ConsistencySample>,
    pub(crate) final_samples: Vec<ConsistencySample>,
}

// 保存真实 UI/Drawing 生产链验收的唯一输入与像素断言。
#[derive(Debug, Clone)]
pub(crate) struct ProductionChainScene {
    // 无窗口离屏目标仍使用 platform RHI 的共享范围值。
    pub(crate) extent: RhiExtent,
    // UI Canvas 接收的完整逻辑 frame，不泄漏原生 surface 类型。
    pub(crate) frame: Rect,
    // 由真实 UI WidgetRender 入口提交的稳定内部矩形。
    pub(crate) rect: Rect,
    // 使用不透明颜色避免把 alpha 舍入误判为调用链断裂。
    pub(crate) color: Color,
    // 期望与容差继续由 Drawing 一处持有，Vulkan 只执行和回读。
    pub(crate) samples: Vec<ConsistencySample>,
}

// 返回 UI → Drawing → FramePlan 生产链唯一共享验收场景。
pub(crate) fn production_chain_scene() -> ProductionChainScene {
    let extent = RhiExtent::new(80, 48);
    let color = Color::from_rgb(36, 144, 220);
    ProductionChainScene {
        extent,
        frame: Rect::new(0.0, 0.0, extent.width as f32, extent.height as f32),
        rect: Rect::new(8.0, 6.0, 16.0, 12.0),
        color,
        samples: vec![
            ConsistencySample::exact(
                12,
                10,
                [color.r, color.g, color.b, color.a],
                ConsistencyTolerance::Quantized,
                "UI Canvas opaque fill",
            ),
            ConsistencySample::exact(
                2,
                2,
                [0, 0, 0, 0],
                ConsistencyTolerance::Exact,
                "FramePlan transparent clear",
            ),
            ConsistencySample::exact(36, 7, [112, 143, 0, 255], ConsistencyTolerance::Filtered, "UI multistop first segment"),
            ConsistencySample::exact(44, 7, [0, 112, 143, 255], ConsistencyTolerance::Filtered, "UI multistop second segment"),
            ConsistencySample::exact(39, 19, [255, 0, 0, 255], ConsistencyTolerance::Quantized, "UI hard stop left"),
            ConsistencySample::exact(40, 19, [0, 0, 255, 255], ConsistencyTolerance::Quantized, "UI hard stop right"),
            ConsistencySample::exact(62, 12, [132, 8, 76, 255], ConsistencyTolerance::Filtered, "UI multiple shadows first layer on top"),
            ConsistencySample::exact(36, 33, [26, 74, 36, 255], ConsistencyTolerance::Filtered, "UI 45deg non-square gradient and opacity"),
            ConsistencySample::exact(33, 28, [16, 42, 46, 255], ConsistencyTolerance::Filtered, "UI multistop translucent partial arc"),
            ConsistencySample::exact(32, 28, [16, 32, 48, 255], ConsistencyTolerance::Exact, "UI multistop clipped corner"),
            ConsistencySample::exact(47, 34, [16, 32, 48, 255], ConsistencyTolerance::Exact, "UI multistop scissor"),
            ConsistencySample::exact(69, 32, [135, 135, 135, 255], ConsistencyTolerance::Filtered, "UI sixteenth stop bound"),
        ],
    }
}

// 用 Drawing 共享断言验证 Adapter 返回的紧密 RGBA8 回读。
pub(crate) fn validate_production_chain_readback(
    scene: &ProductionChainScene,
    pixels: &[u8],
) -> Result<usize, String> {
    let row_bytes = scene.extent.width as usize * 4;
    let expected_len = row_bytes * scene.extent.height as usize;
    if pixels.len() != expected_len {
        return Err(format!(
            "production-chain readback length {} does not match {expected_len}",
            pixels.len()
        ));
    }
    for sample in &scene.samples {
        if sample.x >= scene.extent.width || sample.y >= scene.extent.height {
            return Err(format!(
                "production-chain sample {} is outside {}x{}",
                sample.semantic, scene.extent.width, scene.extent.height
            ));
        }
        let offset = sample.y as usize * row_bytes + sample.x as usize * 4;
        let actual: [u8; 4] = pixels[offset..offset + 4]
            .try_into()
            .map_err(|_| "production-chain sample does not contain one RGBA pixel".to_string())?;
        if !sample.accepts(actual) {
            return Err(format!(
                "{} at ({}, {}): actual {actual:?}, expected {:?}..={:?}, tolerance {:?}({})",
                sample.semantic,
                sample.x,
                sample.y,
                sample.minimum,
                sample.maximum,
                sample.tolerance,
                sample.tolerance.amount(),
            ));
        }
    }
    Ok(scene.samples.len())
}

// 返回十二类 pipeline 的唯一规范场景；顺序同时固定真实 GPU 诊断输出。
pub(crate) fn canonical_scenes() -> Vec<ConsistencyScene> {
    let mut scenes = CONSISTENCY_PIPELINES.map(scene_for_pipeline).to_vec();
    scenes.extend(gradient_mask_scenes());
    scenes
}

// 在任一原生 harness 执行前验证闭集唯一性、共享 ABI、采样资源和采样点边界。
pub(crate) fn validate_canonical_scenes(scenes: &[ConsistencyScene]) -> Result<(), &'static str> {
    if scenes.len() < CONSISTENCY_PIPELINES.len() {
        return Err("canonical scene count does not match PipelineKind coverage");
    }
    for (index, scene) in scenes.iter().enumerate() {
        if index < CONSISTENCY_PIPELINES.len() && scene.kind != CONSISTENCY_PIPELINES[index] {
            return Err("canonical PipelineKind coverage is incomplete or reordered");
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
        PipelineKind::LineSegment => line_segment_scene(),
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
        PendingNativeOp::Line(_) => Some(PipelineKind::LineSegment),
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
        RhiOp::Line(_) => PipelineKind::LineSegment,
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

fn solid_rect_vertices(x: f32, y: f32, width: f32, height: f32) -> FrameVertexPayload {
    FrameVertexPayload::position_coverage_f32([
        x,
        y,
        1.0,
        x + width,
        y,
        1.0,
        x + width,
        y + height,
        1.0,
        x,
        y,
        1.0,
        x + width,
        y + height,
        1.0,
        x,
        y + height,
        1.0,
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
        vertex: solid_rect_vertices(2.0, 2.0, 12.0, 12.0),
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

// 部分覆盖率按独立圆心距离和 source-over 代数冻结；不以 CPU/GPU 相等代替正确性。
// 一组不透明线性端点、一组半透明径向端点，全部在同一不透明底色上合成。
fn gradient_mask_scenes() -> [ConsistencyScene; 2] {
    let linear = ConsistencyScene {
        name: "GradientRectLinearMaskBlend",
        kind: PipelineKind::GradientRect,
        source: ConsistencySource::PendingAndRhi,
        vertex: RhiRenderer::unit_quad_vertex_payload(),
        uniform: FrameUniformPayload::Gradient(RhiGradientRasterParams::new(
            viewport(),
            [[2.0, 50.0], [18.0, 50.0], [18.0, 62.0], [2.0, 62.0]],
            [1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0],
            [0.0, 1.0, 16.0, 12.0], [4.0; 4], [16.0, 12.0],
            [0.0, 0.0, 1.0, 1.0],
        )),
        texture: None,
        scissor: None,
        samples: vec![
            ConsistencySample::exact(10, 59, [53, 0, 202, 255], ConsistencyTolerance::Filtered, "linear opaque interior"),
            ConsistencySample::exact(3, 50, [61, 26, 41, 255], ConsistencyTolerance::Filtered, "linear opaque shallow arc"),
            ConsistencySample::exact(3, 51, [216, 1, 32, 255], ConsistencyTolerance::Filtered, "linear opaque near-full arc"),
            background(2, 50, "linear corner fully clipped"),
        ],
    };
    let radial = ConsistencyScene {
        name: "GradientRectRadialMaskBlend",
        kind: PipelineKind::GradientRect,
        source: ConsistencySource::PendingAndRhi,
        vertex: RhiRenderer::unit_quad_vertex_payload(),
        uniform: FrameUniformPayload::Gradient(RhiGradientRasterParams::new(
            viewport(),
            [[24.0, 50.0], [36.0, 50.0], [36.0, 62.0], [24.0, 62.0]],
            [1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 0.5],
            // outer=1 让圆角部分覆盖区处于渐变内部，而不被圆形 discard 掩盖。
            [1.0, 0.0, 1.0, 0.0], [3.0; 4], [12.0, 12.0],
            [0.0, 0.0, 1.0, 1.0],
        )),
        texture: None,
        scissor: None,
        samples: vec![
            ConsistencySample::exact(30, 53, [181, 3, 54, 255], ConsistencyTolerance::Filtered, "radial translucent interior"),
            ConsistencySample::exact(25, 50, [52, 19, 90, 255], ConsistencyTolerance::Filtered, "radial translucent partial arc"),
            background(24, 50, "radial corner fully clipped"),
            background(23, 56, "radial quad exterior"),
        ],
    };
    [linear, radial]
}

fn gradient_scene() -> ConsistencyScene {
    // 剪切 quad：device = TL + u·(8,0) + v·(4,8)，局部矩形为 8×8。
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
            // DiagonalTLBR + 局部宽高：t = (u + v) / 2，与 CPU
            // linear_gradient_t 在剪切变换下同源。
            [0.0, 2.0, 8.0, 8.0],
            // S4 圆角掩码开启：8x8 局部空间统一半径 3（归一化相邻和 3+5=8
            // 恰好相切），quad 自身单位矩形。
            [3.0; 4],
            [8.0, 8.0],
            [0.0, 0.0, 1.0, 1.0],
        )),
        texture: None,
        scissor: None,
        samples: vec![
            ConsistencySample::exact(
                39,
                5,
                [140, 0, 116, 255],
                ConsistencyTolerance::Filtered,
                "affine diagonal gradient inside mask",
            ),
            // 顶直边中点：local (4.25,0.5) 在掩码直线段内满覆盖，
            // t=(0.53125+0.0625)/2=0.296875 → (179,0,76)。
            ConsistencySample::exact(
                38,
                2,
                [179, 0, 76, 255],
                ConsistencyTolerance::Filtered,
                "gradient top straight edge keeps mask coverage",
            ),
            // 圆角像素：local 中心 (0.25,0.5) 距 tl 圆心 (3,3) 为 3.715，
            // SDF=+0.715 → 覆盖为 0，应保持背景。
            background(34, 2, "gradient rounded corner clipped"),
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

// 构造水平与垂直两 pass 共用的非零原点、非全尺寸 Blur 子区域。
pub(crate) fn blur_subregion_scenario() -> ConsistencyBlurScenario {
    // 选择不与其它规范图元重叠的左下单元格，并保留明确内外边界。
    let region = RhiTextureRegion::from_xy(2, 34, RhiExtent::new(10, 8));
    // 同尺寸 source/destination 通过唯一共享门禁生成 position、UV、scissor 与 uniform。
    let geometry = RhiBlurPassGeometry::new(CONSISTENCY_EXTENT, region, CONSISTENCY_EXTENT, region)
        .expect("canonical blur subregion must satisfy shared geometry contract");
    let mut weights = [0.0f32; BLUR_WEIGHT_COUNT];
    weights[..3].copy_from_slice(&[0.25, 0.5, 0.25]);
    // 纹理外部保持画布背景，子区域使用不同常量底色以暴露越域 taps。
    let pixel_count = CONSISTENCY_EXTENT.width as usize * CONSISTENCY_EXTENT.height as usize;
    let mut source = Vec::with_capacity(pixel_count * 4);
    for _ in 0..pixel_count {
        source.extend_from_slice(&CONSISTENCY_BACKGROUND);
    }
    // 填充完整源域，边界期望能区分“钳到域内”与“采到域外背景”。
    for y in 34usize..42 {
        for x in 2usize..12 {
            let offset = (y * CONSISTENCY_EXTENT.width as usize + x) * 4;
            source[offset..offset + 4].copy_from_slice(&[32, 64, 96, 255]);
        }
    }
    // 单个可区分脉冲同时验证水平、垂直和二维卷积方向。
    let impulse = (37usize * CONSISTENCY_EXTENT.width as usize + 6) * 4;
    source[impulse..impulse + 4].copy_from_slice(&[224, 192, 160, 255]);

    ConsistencyBlurScenario {
        vertex: FrameVertexPayload::position_uv_f32(geometry.vertex_values()),
        horizontal: FrameUniformPayload::Blur(geometry.raster_params(
            RhiBlurDirection::Horizontal,
            1,
            &weights,
        )),
        vertical: FrameUniformPayload::Blur(geometry.raster_params(
            RhiBlurDirection::Vertical,
            1,
            &weights,
        )),
        texture: ConsistencyTexture {
            extent: CONSISTENCY_EXTENT,
            format: TextureFormat::Rgba8Unorm,
            bytes: source,
            sampler: SamplerDesc::linear_clamp(),
        },
        scissor: geometry.destination_scissor(),
        horizontal_samples: vec![
            ConsistencySample::exact(
                6,
                37,
                [128, 128, 128, 255],
                ConsistencyTolerance::Filtered,
                "blur horizontal center",
            ),
            ConsistencySample::exact(
                5,
                37,
                [80, 96, 112, 255],
                ConsistencyTolerance::Filtered,
                "blur horizontal neighbor",
            ),
            ConsistencySample::exact(
                2,
                34,
                [32, 64, 96, 255],
                ConsistencyTolerance::Filtered,
                "blur horizontal source boundary",
            ),
            background(1, 34, "blur horizontal outside destination"),
        ],
        final_samples: vec![
            ConsistencySample::exact(
                6,
                37,
                [80, 96, 112, 255],
                ConsistencyTolerance::Filtered,
                "blur two-pass center",
            ),
            ConsistencySample::exact(
                5,
                37,
                [56, 80, 104, 255],
                ConsistencyTolerance::Filtered,
                "blur two-pass axial neighbor",
            ),
            ConsistencySample::exact(
                2,
                34,
                [32, 64, 96, 255],
                ConsistencyTolerance::Filtered,
                "blur two-pass source boundary",
            ),
            background(1, 34, "blur two-pass outside destination"),
        ],
    }
}

fn blur_scene() -> ConsistencyScene {
    // 单 pass 全图元场景直接复用两 pass 场景的水平输入与断言。
    let scenario = blur_subregion_scenario();
    ConsistencyScene {
        name: "BlurPass",
        kind: PipelineKind::BlurPass,
        source: ConsistencySource::DrawingBlur,
        vertex: scenario.vertex,
        uniform: scenario.horizontal,
        texture: Some(scenario.texture),
        scissor: Some(scenario.scissor),
        samples: scenario.horizontal_samples,
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

// 抗锯齿线段占用最后一个规范单元格，并覆盖内部、边缘与外部像素。
fn line_segment_scene() -> ConsistencyScene {
    ConsistencyScene {
        name: "LineSegment",
        kind: PipelineKind::LineSegment,
        source: ConsistencySource::PendingAndRhi,
        vertex: RhiRenderer::unit_quad_vertex_payload(),
        uniform: FrameUniformPayload::Sector(RhiSectorRasterParams::new(
            viewport(),
            [51.0, 35.0, 61.0, 45.0],
            [1.0, 0.25, 0.2, 0.8],
            [2.0, 0.0],
        )),
        texture: None,
        scissor: None,
        samples: vec![
            ConsistencySample::exact(
                56,
                40,
                [207, 57, 50, 255],
                ConsistencyTolerance::Filtered,
                "line interior",
            ),
            ConsistencySample::range(
                56,
                41,
                [135, 43, 43, 250],
                [165, 58, 58, 255],
                ConsistencyTolerance::Analytic,
                "line antialias edge",
            ),
            background(50, 45, "line exterior"),
        ],
    }
}
