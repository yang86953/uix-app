//! 通用 GPU Renderer 的混合 painter-order RHI lowering。
// 引入共享错误、damage 和结果类型。
use crate::core::error::Result;
// 引入最终计划使用的 damage。
use crate::core::PresentDamage;
// 引入薄 RHI 的 command、resource 和 target 类型。
use crate::native::present::rhi::{
    DrawPacket, GraphicsContextRhi, LoadAction, RhiExtent, RhiViewport, TextureDesc, TextureFormat,
};
// 引入父 renderer 的帧计划和已完成 lowering 的 payload。
use super::{
    FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef, RhiCoverageQuad, RhiGradientRect,
    RhiMsdfQuad, RhiRenderer, RhiSampledQuad, RhiShadow, RhiShapeRect, RhiSolidMesh,
    RhiTexturedQuad,
};
// 将混合操作 ABI 校验拆到独立文件，保持执行器文件边界清晰。
#[path = "rhi_renderer_mixed_validation.rs"]
mod validation;
// 引入拆分后的统一校验入口。
use validation::validate_op;
// 保存一个已经完成物理 lowering 的轴对齐扇形 payload。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RhiSector {
    // 保存扇形外接椭圆左上角物理坐标。
    pub(crate) x: f32,
    // 保存扇形外接椭圆左上角物理坐标。
    pub(crate) y: f32,
    // 保存扇形外接椭圆物理宽度。
    pub(crate) w: f32,
    // 保存扇形外接椭圆物理高度。
    pub(crate) h: f32,
    // 保存从 +x 方向开始、在 top-left 坐标系顺时针前进的起始角。
    pub(crate) start_angle: f32,
    // 保存正向填充扫过的弧度。
    pub(crate) sweep_angle: f32,
    // 保存已经规整过的直通颜色。
    pub(crate) rgba: [f32; 4],
    // 保存当前扇形的物理裁剪矩形。
    pub(crate) scissor: Option<crate::native::present::rhi::RhiScissor>,
}
// 保存一个已经完成几何 lowering 的混合 painter-order 操作。
pub(crate) enum RhiOp {
    // 保存实心 mesh 操作。
    Solid(RhiSolidMesh),
    // 保存颜色纹理 quad 操作。
    Textured(RhiTexturedQuad),
    // 保存一个已经存在的 sampled texture quad。
    Sampled(RhiSampledQuad),
    // 保存 R8 glyph coverage quad 操作。
    Coverage(RhiCoverageQuad),
    // 保存 RGBA8 MSDF glyph quad 操作。
    Msdf(RhiMsdfQuad),
    // 保存线性或径向渐变操作。
    Gradient(RhiGradientRect),
    // 保存圆角或描边矩形操作。
    Shape(RhiShapeRect),
    // 保存使用饱和加法 blend 的圆角或描边矩形操作。
    AdditiveShape(RhiShapeRect),
    // 保存原生轴对齐扇形操作。
    Sector(RhiSector),
    // 保存轴对齐阴影操作。
    Shadow(RhiShadow),
}
// 为混合 lowering 提供资源句柄的可选槽位访问。
fn required<T>(value: Option<T>, message: &'static str) -> Result<T> {
    // 缺失资源说明 renderer 在建计划前没有完成对应的 ensure 阶段。
    value.ok_or_else(|| super::rhi_invalid(message))
}
// 构造 float8 sampled quad 的统一顶点 ABI。
fn textured_vertices(quad: &RhiTexturedQuad) -> [f32; 48] {
    // 复用统一的 sampled quad 顶点 ABI。
    textured_vertices_values(quad.corners, quad.rgba, 0.0, 0.0, 1.0, 1.0)
}
// 为已经存在的离屏纹理构造相同的 sampled quad 顶点 ABI。
pub(super) fn sampled_vertices(quad: &RhiSampledQuad) -> [f32; 48] {
    // 保持图片上传和 Picture 合成使用完全相同的顶点布局。
    textured_vertices_values(quad.corners, quad.rgba, quad.u0, quad.v0, quad.u1, quad.v1)
}

// 构造任意四角、UV 和 tint 交错排列的六顶点数组。
fn textured_vertices_values(
    corners: [[f32; 2]; 4],
    color: [f32; 4],
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
) -> [f32; 48] {
    // 返回左上、右上、右下、左上、右下、左下两个三角形。
    [
        corners[0][0],
        corners[0][1],
        u0,
        v0,
        color[0],
        color[1],
        color[2],
        color[3],
        corners[1][0],
        corners[1][1],
        u1,
        v0,
        color[0],
        color[1],
        color[2],
        color[3],
        corners[2][0],
        corners[2][1],
        u1,
        v1,
        color[0],
        color[1],
        color[2],
        color[3],
        corners[0][0],
        corners[0][1],
        u0,
        v0,
        color[0],
        color[1],
        color[2],
        color[3],
        corners[2][0],
        corners[2][1],
        u1,
        v1,
        color[0],
        color[1],
        color[2],
        color[3],
        corners[3][0],
        corners[3][1],
        u0,
        v1,
        color[0],
        color[1],
        color[2],
        color[3],
    ]
}

// 为通用 renderer 创建或复用扇形的固定资源。
impl RhiRenderer {
    // 准备 position float2 单位 quad、SectorConstants 和 sector pipeline。
    fn ensure_sector_resources(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<(
        crate::native::present::rhi::PipelineHandle,
        crate::native::present::rhi::BufferHandle,
        crate::native::present::rhi::BufferHandle,
    )> {
        // 首次使用时创建固定扇形 pipeline。
        let pipeline = if let Some(pipeline) = self.sector_pipeline {
            // 复用已经登记的扇形 pipeline。
            pipeline
        } else {
            // 只选择通用层定义的扇形 pipeline key。
            let pipeline = context.create_pipeline(crate::native::present::rhi::PipelineDesc {
                key: crate::native::present::rhi::pipeline_keys::SECTOR,
            })?;
            // 缓存扇形 pipeline 句柄。
            self.sector_pipeline = Some(pipeline);
            pipeline
        };
        // 单位 quad 使用六个 float2 顶点。
        let unit_vertices = [
            0.0f32, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0,
        ];
        // 首次使用时创建单位 quad buffer。
        let vertex_buffer = if let Some(buffer) = self.sector_vertex_buffer {
            // 复用已有单位 quad buffer。
            buffer
        } else {
            // 创建 position float2 ABI 的 vertex buffer。
            let buffer = context.create_buffer(crate::native::present::rhi::BufferDesc {
                size_bytes: unit_vertices.len() * std::mem::size_of::<f32>(),
                stride_bytes: (2 * std::mem::size_of::<f32>()) as u32,
                usage: crate::native::present::rhi::BufferUsage::Vertex,
            })?;
            // 首次绑定前上传单位 quad。
            context.update_buffer(buffer, 0, &RhiRenderer::encode_f32s(&unit_vertices))?;
            // 缓存单位 quad 句柄。
            self.sector_vertex_buffer = Some(buffer);
            buffer
        };
        // 首次使用时创建 64 字节 SectorConstants uniform buffer。
        let uniform_buffer = if let Some(uniform) = self.sector_uniform {
            // 复用已有扇形常量 buffer。
            uniform
        } else {
            // D3D11 常量布局由 viewport、矩形、颜色和角度四个 float4 组成。
            let uniform = context.create_buffer(crate::native::present::rhi::BufferDesc {
                size_bytes: crate::native::present::rhi::SECTOR_UNIFORM_BYTES,
                stride_bytes: 0,
                usage: crate::native::present::rhi::BufferUsage::Uniform,
            })?;
            // 缓存扇形常量句柄。
            self.sector_uniform = Some(uniform);
            uniform
        };
        // 返回本次 sector draw 所需的固定资源。
        Ok((pipeline, vertex_buffer, uniform_buffer))
    }
}

// 为通用 renderer 提供一个混合操作的单次 FramePlan 执行入口。
impl RhiRenderer {
    // 按调用方要求执行最终 present 或无 present 的混合 RHI 计划。
    pub(crate) fn execute_ops(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        viewport: RhiViewport,
        load: LoadAction,
        target: RenderTargetRef,
        operations: &[RhiOp],
        present: bool,
    ) -> Result<()> {
        if operations.is_empty() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid(
                "RhiRenderer cannot execute empty mixed ops",
            ));
        }
        if !viewport.is_valid() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid("RhiRenderer mixed viewport is invalid"));
        }
        for operation in operations {
            // 统一验证当前操作。
            validate_op(operation)?;
        }
        let max_solid_bytes = operations
            .iter()
            .filter_map(|operation| match operation {
                RhiOp::Solid(mesh) => Some(mesh.vertices.len() * std::mem::size_of::<f32>()),
                _ => None,
            })
            .max();
        // 只在当前帧出现 solid 时准备 solid 资源。
        let solid_resources = match max_solid_bytes {
            // 创建或复用 solid pipeline、vertex 和 uniform。
            Some(bytes) => Some(self.ensure_solid_resources(context, bytes)?),
            // 不含 solid 时保持空槽。
            None => None,
        };
        // 只在当前帧出现 color texture 时准备 sampled 资源。
        let textured_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Textured(_) | RhiOp::Sampled(_)))
        {
            // 创建或复用 sampled pipeline、vertex、uniform 和 sampler。
            Some(self.ensure_textured_resources(context)?)
        } else {
            // 不含颜色纹理时保持空槽。
            None
        };
        // 只在当前帧出现 Additive image 时创建独立 pipeline。
        let additive_pipeline = if operations.iter().any(|operation| {
            matches!(operation, RhiOp::Textured(quad) if quad.additive)
                || matches!(operation, RhiOp::Sampled(quad) if quad.additive)
        }) {
            // 保持 Additive 和 SrcOver 的 blend 状态分离。
            Some(self.ensure_additive_textured_pipeline(context)?)
        } else {
            // 不含 Additive 时不创建额外资源。
            None
        };
        // 只在当前帧出现 coverage 时准备 R8 pipeline 和 point sampler。
        let coverage_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Coverage(_)))
        {
            // 创建或复用 coverage pipeline 和共享 float8 buffer。
            Some(self.ensure_coverage_resources(context)?)
        } else {
            // 不含 coverage 时保持空槽。
            None
        };
        // 只在当前帧出现 MSDF 时准备 RGBA8 MSDF pipeline 和资源。
        let msdf_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Msdf(_)))
        {
            // 创建或复用 MSDF pipeline、共享 float8 buffer、常量和线性 sampler。
            Some(self.ensure_msdf_resources(context)?)
        } else {
            // 不含 MSDF 时保持空槽。
            None
        };
        let gradient_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Gradient(_)))
        {
            // 创建或复用渐变 pipeline、unit quad 和 uniform。
            Some(self.ensure_gradient_resources(context)?)
        } else {
            // 不含渐变时保持空槽。
            None
        };
        let shape_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Shape(_) | RhiOp::AdditiveShape(_)))
        {
            // 创建或复用 shape pipeline、unit quad 和 uniform。
            Some(self.ensure_shape_resources(context)?)
        } else {
            // 不含 shape 时保持空槽。
            None
        };
        let shadow_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Shadow(_)))
        {
            // 创建或复用 shadow pipeline、unit quad 和 uniform。
            Some(self.ensure_shadow_resources(context)?)
        } else {
            // 不含 shadow 时保持空槽。
            None
        };
        let sector_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Sector(_)))
        {
            // 创建或复用 sector pipeline、unit quad 和 uniform。
            Some(self.ensure_sector_resources(context)?)
        } else {
            // 不含扇形时保持空槽。
            None
        };
        // 为颜色纹理、coverage texture 和 MSDF atlas page 按操作序号保存 draw 句柄。
        let mut textures = vec![None; operations.len()];
        // 为 MSDF 操作保存每个字形在 page 内的归一化 UV。
        let mut msdf_uvs = vec![[0.0, 0.0, 1.0, 1.0]; operations.len()];
        // 为 MSDF 导数 AA 保存实际绑定 texture 的物理尺寸。
        let mut msdf_texture_sizes = vec![[1.0, 1.0]; operations.len()];
        // 只记录本帧真正需要销毁的临时 texture，跨帧 atlas page 不进入此列表。
        let mut transient_textures = Vec::new();
        // 创建并上传所有源纹理，避免计划构造中途才发现资源错误。
        for (index, operation) in operations.iter().enumerate() {
            // MSDF 优先走跨帧 atlas，减少字形源纹理的反复创建和上传。
            if let RhiOp::Msdf(quad) = operation {
                // atlas 失败时先清理当前帧已有的临时 texture。
                let (texture, uv, texture_size, persistent) =
                    match self.ensure_msdf_texture(context, quad) {
                        // 返回 atlas page 或本帧临时 texture 及其 UV。
                        Ok(result) => result,
                        // 保留原始资源错误，不伪造可提交的混合计划。
                        Err(error) => {
                            let _ = RhiRenderer::destroy_textures(context, &transient_textures);
                            return Err(error);
                        }
                    };
                // 记录 MSDF draw 需要的 page texture 与 placement UV。
                textures[index] = Some(texture);
                msdf_uvs[index] = uv;
                msdf_texture_sizes[index] = texture_size;
                // 超出 atlas 预算的单帧 texture 由本次执行边界负责回收。
                if !persistent {
                    transient_textures.push(texture);
                }
                // 继续处理下一个 painter-order operation。
                continue;
            }
            // 只处理需要上传的两类 texture source。
            let (extent, format, payload) = match operation {
                // 准备 BGRA 源纹理。
                RhiOp::Textured(quad) => (
                    RhiExtent::new(quad.pixel_w, quad.pixel_h),
                    TextureFormat::Bgra8Unorm,
                    RhiRenderer::encode_u32s(quad.pixels.as_ref()),
                ),
                // 准备 R8 coverage 源纹理。
                RhiOp::Coverage(quad) => (
                    RhiExtent::new(quad.pixel_w, quad.pixel_h),
                    TextureFormat::R8Unorm,
                    RhiRenderer::encode_coverage(quad.coverage.as_ref()),
                ),
                // MSDF 已在本轮循环开头处理，不能重复进入临时 texture 分支。
                RhiOp::Msdf(_) => unreachable!("MSDF cache branch must continue"),
                // 其他操作没有临时 sampled source。
                _ => continue,
            };
            // 创建对应格式的临时纹理。
            let texture = match context.create_texture(TextureDesc { extent, format }) {
                // 记录成功创建的资源。
                Ok(texture) => texture,
                // 创建失败时清理此前已创建的资源。
                Err(error) => {
                    let _ = RhiRenderer::destroy_textures(context, &transient_textures);
                    return Err(error);
                }
            };
            // 先记录为临时句柄，后续任一步失败都能统一清理。
            textures[index] = Some(texture);
            // 本帧结束后释放颜色或 coverage 临时 texture。
            transient_textures.push(texture);
            // 上传紧密排列的 source payload。
            if let Err(error) = context.update_texture(texture, extent, &payload) {
                // 释放当前帧已经创建的全部临时资源。
                let _ = RhiRenderer::destroy_textures(context, &transient_textures);
                // 保留上传失败的原始错误。
                return Err(error);
            }
        }
        // 计划使用当前 context 的 surface 代际作为执行代际检查。
        let surface = context.token();
        // 创建一个显式 surface 或 offscreen pass。
        let mut pass = RenderPassPlan::new(target, load);
        // 所有操作共享同一物理 viewport。
        pass.push(FramePlanCommand::SetViewport(viewport));
        // 按原始操作顺序追加 command，不能按 pipeline 类型重排。
        for (index, operation) in operations.iter().enumerate() {
            // 逐类取得对应资源并编码固定 ABI。
            match operation {
                // 编码 solid mesh。
                RhiOp::Solid(mesh) => {
                    // 取得 solid 资源槽。
                    let (pipeline, vertex_buffer, uniform_buffer) = required(
                        solid_resources,
                        "RhiRenderer mixed solid resources are missing",
                    )?;
                    // 选择当前 mesh 的裁剪。
                    pass.push(FramePlanCommand::SetScissor(mesh.scissor));
                    // 上传当前 mesh 顶点。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: vertex_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_f32s(mesh.vertices.as_ref()),
                    });
                    // 上传 MeshConstants。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: uniform_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_f32s(&[
                            viewport.width,
                            viewport.height,
                            0.0,
                            0.0,
                            mesh.rgba[0],
                            mesh.rgba[1],
                            mesh.rgba[2],
                            mesh.rgba[3],
                        ]),
                    });
                    // 追加当前 mesh draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket {
                        pipeline,
                        vertex_buffer,
                        index_buffer: None,
                        uniform_buffer: Some(uniform_buffer),
                        vertex_count: (mesh.vertices.len() / 2) as u32,
                        index_count: 0,
                        first_vertex: 0,
                        first_index: 0,
                        base_vertex: 0,
                    }));
                }
                // 编码颜色纹理 quad。
                RhiOp::Textured(quad) => {
                    // 取得 sampled 资源槽。
                    let (src_pipeline, vertex_buffer, uniform_buffer, sampler) = required(
                        textured_resources,
                        "RhiRenderer mixed textured resources are missing",
                    )?;
                    // 取得当前纹理句柄。
                    let texture = required(
                        textures[index],
                        "RhiRenderer mixed textured source is missing",
                    )?;
                    // Additive quad 使用独立 pipeline。
                    let pipeline = if quad.additive {
                        required(
                            additive_pipeline,
                            "RhiRenderer mixed additive pipeline is missing",
                        )?
                    } else {
                        src_pipeline
                    };
                    // 选择当前图片的裁剪。
                    pass.push(FramePlanCommand::SetScissor(quad.scissor));
                    // 上传 float8 quad 顶点。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: vertex_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_f32s(&textured_vertices(quad)),
                    });
                    // 上传 viewport uniform。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: uniform_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_f32s(&[
                            viewport.width,
                            viewport.height,
                            0.0,
                            0.0,
                        ]),
                    });
                    // 绑定当前颜色纹理。
                    pass.push(FramePlanCommand::BindTexture {
                        slot: 0,
                        texture,
                        sampler,
                    });
                    // 追加当前图片 draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket {
                        pipeline,
                        vertex_buffer,
                        index_buffer: None,
                        uniform_buffer: Some(uniform_buffer),
                        vertex_count: 6,
                        index_count: 0,
                        first_vertex: 0,
                        first_index: 0,
                        base_vertex: 0,
                    }));
                }
                // 编码已经存在的 sampled texture quad。
                RhiOp::Sampled(quad) => {
                    // 取得 sampled 资源槽。
                    let (src_pipeline, vertex_buffer, uniform_buffer, sampler) = required(
                        textured_resources,
                        "RhiRenderer mixed sampled resources are missing",
                    )?;
                    // Additive sampled quad 使用独立 pipeline。
                    let pipeline = if quad.additive {
                        required(
                            additive_pipeline,
                            "RhiRenderer mixed sampled additive pipeline is missing",
                        )?
                    } else {
                        src_pipeline
                    };
                    // 选择当前 Picture quad 的裁剪。
                    pass.push(FramePlanCommand::SetScissor(quad.scissor));
                    // 上传 float8 quad 顶点。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: vertex_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_f32s(&sampled_vertices(quad)),
                    });
                    // 上传 viewport uniform。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: uniform_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_f32s(&[
                            viewport.width,
                            viewport.height,
                            0.0,
                            0.0,
                        ]),
                    });
                    // 绑定已经存在的 Picture texture。
                    pass.push(FramePlanCommand::BindTexture {
                        slot: 0,
                        texture: quad.texture,
                        sampler,
                    });
                    // 追加当前 Picture draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket {
                        pipeline,
                        vertex_buffer,
                        index_buffer: None,
                        uniform_buffer: Some(uniform_buffer),
                        vertex_count: 6,
                        index_count: 0,
                        first_vertex: 0,
                        first_index: 0,
                        base_vertex: 0,
                    }));
                }
                // 编码 R8 glyph coverage quad。
                RhiOp::Coverage(quad) => {
                    // 取得 coverage 资源槽。
                    let (pipeline, vertex_buffer, uniform_buffer, sampler) = required(
                        coverage_resources,
                        "RhiRenderer mixed coverage resources are missing",
                    )?;
                    // 取得当前 coverage texture。
                    let texture = required(
                        textures[index],
                        "RhiRenderer mixed coverage source is missing",
                    )?;
                    // 选择当前 glyph 的裁剪。
                    pass.push(FramePlanCommand::SetScissor(quad.scissor));
                    // 上传 coverage quad 顶点。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: vertex_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_f32s(&RhiRenderer::coverage_quad_vertices(quad)),
                    });
                    // 上传 viewport uniform。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: uniform_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_f32s(&[
                            viewport.width,
                            viewport.height,
                            0.0,
                            0.0,
                        ]),
                    });
                    // 绑定当前 R8 coverage texture。
                    pass.push(FramePlanCommand::BindTexture {
                        slot: 0,
                        texture,
                        sampler,
                    });
                    // 追加当前 glyph draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket {
                        pipeline,
                        vertex_buffer,
                        index_buffer: None,
                        uniform_buffer: Some(uniform_buffer),
                        vertex_count: 6,
                        index_count: 0,
                        first_vertex: 0,
                        first_index: 0,
                        base_vertex: 0,
                    }));
                }
                // 编码 RGBA8 MSDF 字形 quad。
                RhiOp::Msdf(quad) => {
                    // 取得 MSDF 资源槽。
                    let (pipeline, vertex_buffer, uniform_buffer, sampler) = required(
                        msdf_resources,
                        "RhiRenderer mixed MSDF resources are missing",
                    )?;
                    // 取得当前 MSDF texture。
                    let texture =
                        required(textures[index], "RhiRenderer mixed MSDF source is missing")?;
                    // 选择当前 MSDF 字形的裁剪。
                    pass.push(FramePlanCommand::SetScissor(quad.scissor));
                    // 上传支持旋转和剪切以及 atlas UV 的 MSDF quad 顶点。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: vertex_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_f32s(&RhiRenderer::msdf_quad_vertices_with_uv(
                            quad,
                            msdf_uvs[index],
                        )),
                    });
                    // 上传 viewport、source extent 和 MSDF range 常量。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: uniform_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_msdf_constants(
                            viewport,
                            quad,
                            msdf_texture_sizes[index],
                        ),
                    });
                    // 绑定当前 RGBA8 MSDF texture 和线性 sampler。
                    pass.push(FramePlanCommand::BindTexture {
                        slot: 0,
                        texture,
                        sampler,
                    });
                    // 追加当前 MSDF 字形 draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket {
                        pipeline,
                        vertex_buffer,
                        index_buffer: None,
                        uniform_buffer: Some(uniform_buffer),
                        vertex_count: 6,
                        index_count: 0,
                        first_vertex: 0,
                        first_index: 0,
                        base_vertex: 0,
                    }));
                }
                // 编码渐变矩形。
                RhiOp::Gradient(gradient) => {
                    // 取得渐变资源槽。
                    let (pipeline, vertex_buffer, uniform_buffer) = required(
                        gradient_resources,
                        "RhiRenderer mixed gradient resources are missing",
                    )?;
                    // 选择当前渐变的裁剪。
                    pass.push(FramePlanCommand::SetScissor(gradient.scissor));
                    // 上传 96 字节仿射 GradientConstants。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: uniform_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_gradient_constants(viewport, gradient),
                    });
                    // 追加当前渐变 draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket {
                        pipeline,
                        vertex_buffer,
                        index_buffer: None,
                        uniform_buffer: Some(uniform_buffer),
                        vertex_count: 6,
                        index_count: 0,
                        first_vertex: 0,
                        first_index: 0,
                        base_vertex: 0,
                    }));
                }
                // 编码圆角/描边矩形。
                RhiOp::Shape(rect) | RhiOp::AdditiveShape(rect) => {
                    // 取得 shape 资源槽。
                    let (normal_pipeline, additive_pipeline, vertex_buffer, uniform_buffer) =
                        required(
                            shape_resources,
                            "RhiRenderer mixed shape resources are missing",
                        )?;
                    // AdditiveShape 只切换 blend pipeline，几何和常量 ABI 保持一致。
                    let pipeline = if matches!(operation, RhiOp::AdditiveShape(_)) {
                        additive_pipeline
                    } else {
                        normal_pipeline
                    };
                    // 复用 shape 模块的固定常量和 draw packet lowering。
                    RhiRenderer::append_shape_commands(
                        &mut pass,
                        viewport,
                        rect,
                        pipeline,
                        vertex_buffer,
                        uniform_buffer,
                    );
                }
                // 编码轴对齐原生扇形。
                RhiOp::Sector(sector) => {
                    // 取得 sector 资源槽。
                    let (pipeline, vertex_buffer, uniform_buffer) = required(
                        sector_resources,
                        "RhiRenderer mixed sector resources are missing",
                    )?;
                    // 选择当前扇形的裁剪。
                    pass.push(FramePlanCommand::SetScissor(sector.scissor));
                    // 上传 64 字节 SectorConstants。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: uniform_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_f32s(&[
                            viewport.width,
                            viewport.height,
                            0.0,
                            0.0,
                            sector.x,
                            sector.y,
                            sector.w,
                            sector.h,
                            sector.rgba[0],
                            sector.rgba[1],
                            sector.rgba[2],
                            sector.rgba[3],
                            sector.start_angle,
                            sector.sweep_angle,
                            0.0,
                            0.0,
                        ]),
                    });
                    // 追加当前扇形 draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket {
                        pipeline,
                        vertex_buffer,
                        index_buffer: None,
                        uniform_buffer: Some(uniform_buffer),
                        vertex_count: 6,
                        index_count: 0,
                        first_vertex: 0,
                        first_index: 0,
                        base_vertex: 0,
                    }));
                }
                // 编码保留设备四角的仿射阴影。
                RhiOp::Shadow(shadow) => {
                    // 取得 shadow 资源槽。
                    let (pipeline, vertex_buffer, uniform_buffer) = required(
                        shadow_resources,
                        "RhiRenderer mixed shadow resources are missing",
                    )?;
                    // 选择当前阴影的裁剪。
                    pass.push(FramePlanCommand::SetScissor(shadow.scissor));
                    // 上传 AffineShadowConstants。
                    pass.push(FramePlanCommand::UpdateBuffer {
                        buffer: uniform_buffer,
                        offset: 0,
                        data: RhiRenderer::encode_f32s(&[
                            viewport.width,
                            viewport.height,
                            0.0,
                            0.0,
                            shadow.corners[0][0],
                            shadow.corners[0][1],
                            shadow.corners[1][0] - shadow.corners[0][0],
                            shadow.corners[1][1] - shadow.corners[0][1],
                            shadow.rgba[0],
                            shadow.rgba[1],
                            shadow.rgba[2],
                            shadow.rgba[3],
                            shadow.radius[0],
                            shadow.radius[1],
                            shadow.radius[2],
                            shadow.radius[3],
                            shadow.corners[3][0] - shadow.corners[0][0],
                            shadow.corners[3][1] - shadow.corners[0][1],
                            shadow.blur_x,
                            shadow.blur_y,
                            shadow.w,
                            shadow.h,
                            if shadow.ambient { 1.0 } else { 0.0 },
                            0.0,
                        ]),
                    });
                    // 追加当前阴影 draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket {
                        pipeline,
                        vertex_buffer,
                        index_buffer: None,
                        uniform_buffer: Some(uniform_buffer),
                        vertex_count: 6,
                        index_count: 0,
                        first_vertex: 0,
                        first_index: 0,
                        base_vertex: 0,
                    }));
                }
            }
        }
        // 创建一个保留操作顺序的单 pass 计划。
        let mut plan = FramePlan::new(surface, damage);
        // 追加当前 target pass。
        plan.push_pass(pass);
        // 选择 surface 最终 present 或当前帧片段的无 present 提交边界。
        let execution = if present {
            // 普通主帧计划由该调用直接完成唯一最终 present。
            super::execute_plan_for_target(context, &plan, target)
        } else {
            // 编码帧和 Picture 计划只提交，最终 present 由外层统一完成。
            super::execute_plan_without_present(context, &plan, target)
        };
        // 计划结束后只释放本次混合 lowering 创建的临时纹理。
        let cleanup = RhiRenderer::destroy_textures(context, &transient_textures);
        // 优先保留执行错误，再报告资源清理错误。
        match (execution, cleanup) {
            // 计划失败时返回原始执行错误。
            (Err(error), _) => Err(error),
            // 计划成功但清理失败时不能伪造完整成功。
            (Ok(()), Err(error)) => Err(error),
            // 计划和资源清理均成功。
            (Ok(()), Ok(())) => Ok(()),
        }
    }
    // 执行一个已经存在的 sampled texture quad，不重新上传源纹理。
    pub(crate) fn execute_sampled_quad(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        viewport: RhiViewport,
        load: LoadAction,
        target: RenderTargetRef,
        quad: RhiSampledQuad,
    ) -> Result<()> {
        // 将单个 Picture 合成操作复用同一套 FramePlan 资源和清理边界。
        let operation = RhiOp::Sampled(quad);
        // 仍由混合执行器负责能力校验、submit 和临时资源生命周期。
        self.execute_ops(
            context,
            damage,
            viewport,
            load,
            target,
            std::slice::from_ref(&operation),
            true,
        )
    }
}
