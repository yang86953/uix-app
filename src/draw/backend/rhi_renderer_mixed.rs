//! 通用 GPU Renderer 的混合 painter-order RHI lowering。
// 引入共享错误结果类型。
use crate::core::error::Result;
// 引入薄 RHI 的 command、resource 和 target 类型。
use crate::platform::presentation::rhi::{
    DrawBufferBindings, DrawPacket, DrawRange, DrawRasterState, DrawSamplingBinding,
    GraphicsDevice, LoadAction, RhiExtent, RhiTextureUpload, RhiViewport, SampledTextureBinding,
    TextureDesc, TextureFormat,
};
// 引入父 renderer 的帧计划和已完成 lowering 的 payload。
use super::{
    FramePlanCommand, FrameUniformPayload, FrameVertexPayload, RhiCoverageQuad, RhiGradientRect,
    RhiLineSegment, RhiMsdfQuad, RhiRenderer, RhiSampledQuad, RhiShadow, RhiShapeRect,
    RhiSolidMesh, RhiTexturedQuad,
};
// 将混合操作 ABI 约束检查拆到独立文件，保持执行器文件边界清晰。
#[path = "rhi_renderer_mixed_contract.rs"]
mod contract;
// 引入拆分后的统一约束检查入口。
use contract::check_op;
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
    pub(crate) scissor: Option<crate::platform::presentation::rhi::RhiScissor>,
}
// 保存一个已经完成几何 lowering 的混合 painter-order 操作。
pub(crate) enum RhiOp {
    // 保存实心 mesh 操作。
    Solid(RhiSolidMesh),
    // 保存解析抗锯齿线段操作。
    Line(RhiLineSegment),
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
pub(super) fn textured_vertices_values(
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
        device: &mut dyn GraphicsDevice,
    ) -> Result<(
        crate::platform::presentation::rhi::PipelineBinding,
        crate::platform::presentation::rhi::BufferHandle,
        crate::platform::presentation::rhi::BufferHandle,
    )> {
        // 首次使用时创建固定扇形 pipeline。
        let pipeline = if let Some(pipeline) = self.sector_pipeline {
            // 复用已经登记的扇形 pipeline。
            pipeline
        } else {
            // 只选择通用层定义的扇形 pipeline 语义。
            let pipeline =
                device.create_pipeline(crate::platform::presentation::rhi::PipelineDesc {
                    kind: crate::platform::presentation::rhi::PipelineKind::Sector,
                })?;
            // 缓存扇形 pipeline 句柄。
            self.sector_pipeline = Some(pipeline);
            pipeline
        };
        // 静态单位 quad 的内容由每个 FramePlan pass 显式上传。
        let unit_vertices = RhiRenderer::unit_quad_vertex_payload();
        // 首次使用时创建单位 quad buffer。
        let vertex_buffer = if let Some(buffer) = self.sector_vertex_buffer {
            // 复用已有单位 quad buffer。
            buffer
        } else {
            // 创建 position float2 ABI 的 vertex buffer。
            let buffer =
                device.create_buffer(crate::platform::presentation::rhi::BufferDesc::vertex(
                    // 保存共享类型化 payload 的精确容量。
                    unit_vertices.size_bytes(),
                    // 步长只来自共享 Sector 顶点 ABI。
                    crate::platform::presentation::rhi::PipelineKind::Sector
                        .contract()
                        .vertex
                        .stride_bytes(),
                ))?;
            // 缓存尚未写入本帧内容的单位 quad 句柄。
            self.sector_vertex_buffer = Some(buffer);
            buffer
        };
        // 首次使用时创建 64 字节 SectorConstants uniform buffer。
        let uniform_buffer = if let Some(uniform) = self.sector_uniform {
            // 复用已有扇形常量 buffer。
            uniform
        } else {
            // 共享常量布局由 viewport、矩形、颜色和角度四个 float4 组成。
            let uniform =
                device.create_buffer(crate::platform::presentation::rhi::BufferDesc::uniform(
                    // 常量容量只来自共享 Sector Uniform ABI。
                    crate::platform::presentation::rhi::PipelineKind::Sector
                        .contract()
                        .uniform
                        .size_bytes(),
                ))?;
            // 缓存扇形常量句柄。
            self.sector_uniform = Some(uniform);
            uniform
        };
        // 返回本次 sector draw 所需的固定资源。
        Ok((pipeline, vertex_buffer, uniform_buffer))
    }

    // 准备 position float2 单位 quad、共享四组常量和解析线段 pipeline。
    fn ensure_line_resources(
        &mut self,
        device: &mut dyn GraphicsDevice,
    ) -> Result<(
        crate::platform::presentation::rhi::PipelineBinding,
        crate::platform::presentation::rhi::BufferHandle,
        crate::platform::presentation::rhi::BufferHandle,
    )> {
        let pipeline = if let Some(pipeline) = self.line_pipeline {
            pipeline
        } else {
            let pipeline =
                device.create_pipeline(crate::platform::presentation::rhi::PipelineDesc {
                    kind: crate::platform::presentation::rhi::PipelineKind::LineSegment,
                })?;
            self.line_pipeline = Some(pipeline);
            pipeline
        };
        let unit_vertices = RhiRenderer::unit_quad_vertex_payload();
        let vertex_buffer = if let Some(buffer) = self.line_vertex_buffer {
            buffer
        } else {
            let buffer =
                device.create_buffer(crate::platform::presentation::rhi::BufferDesc::vertex(
                    unit_vertices.size_bytes(),
                    crate::platform::presentation::rhi::PipelineKind::LineSegment
                        .contract()
                        .vertex
                        .stride_bytes(),
                ))?;
            self.line_vertex_buffer = Some(buffer);
            buffer
        };
        let uniform_buffer = if let Some(buffer) = self.line_uniform {
            buffer
        } else {
            let buffer =
                device.create_buffer(crate::platform::presentation::rhi::BufferDesc::uniform(
                    crate::platform::presentation::rhi::PipelineKind::LineSegment
                        .contract()
                        .uniform
                        .size_bytes(),
                ))?;
            self.line_uniform = Some(buffer);
            buffer
        };
        Ok((pipeline, vertex_buffer, uniform_buffer))
    }
}

// 为通用 renderer 提供一个混合操作的单次 FramePlan 执行入口。
impl RhiRenderer {
    // 按封闭帧作用域执行 Surface 或 Offscreen 混合 RHI 计划。
    pub(crate) fn execute_ops(
        // 借用 renderer 资源缓存。
        &mut self,
        // 接收已经原子绑定角色、target 与 present 语义的帧。
        mut frame: super::RhiRendererFrame<'_>,
        // 接收物理 viewport。
        viewport: RhiViewport,
        // 接收目标 load 动作。
        load: LoadAction,
        // 接收已经按 painter order 排列的操作。
        operations: &[RhiOp],
        // 返回计划、资源清理或最终 present 的真实结果。
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
            // 统一检查当前操作的固定 ABI 约束。
            check_op(operation)?;
        }
        // SolidMesh 在物理坐标中统一生成一次 coverage 边带，容量与提交复用同一载荷。
        let solid_vertices: Vec<_> = operations
            .iter()
            .map(|operation| match operation {
                RhiOp::Solid(mesh) => Some(super::mesh::antialiased_vertices(&mesh.vertices)),
                _ => None,
            })
            .collect();
        let max_solid_bytes = solid_vertices
            .iter()
            .filter_map(|vertices| vertices.as_ref())
            .map(|vertices| vertices.len() * std::mem::size_of::<f32>())
            .max();
        // 只在当前帧出现 solid 时准备 solid 资源。
        let solid_resources = match max_solid_bytes {
            // 创建或复用 solid pipeline、vertex 和 uniform。
            Some(bytes) => Some(self.ensure_solid_resources(frame.device(), bytes)?),
            // 不含 solid 时保持空槽。
            None => None,
        };
        // 解析线段使用固定单位 quad 与独立覆盖率 pipeline。
        let line_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Line(_)))
        {
            Some(self.ensure_line_resources(frame.device())?)
        } else {
            None
        };
        // 三类 float8 sampled 操作共享同一 vertex buffer，必须在返回任何句柄前统一扩容。
        let coverage_count = operations
            .iter()
            .filter(|operation| matches!(operation, RhiOp::Coverage(_)))
            .count();
        let msdf_count = operations
            .iter()
            .filter(|operation| matches!(operation, RhiOp::Msdf(_)))
            .count();
        let sampled_vertex_bytes = coverage_count
            .max(msdf_count)
            .max(1)
            .checked_mul(6 * 8 * std::mem::size_of::<f32>())
            .ok_or_else(|| super::rhi_invalid("RhiRenderer sampled vertex capacity overflows"))?;
        // 只在当前帧出现 color texture 时准备 sampled 资源。
        let textured_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Textured(_) | RhiOp::Sampled(_)))
        {
            // 创建或复用 sampled pipeline、vertex、uniform 和 sampler。
            Some(
                self.ensure_textured_resources_with_capacity(frame.device(), sampled_vertex_bytes)?,
            )
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
            Some(self.ensure_additive_textured_pipeline(frame.device())?)
        } else {
            // 不含 Additive 时不创建额外资源。
            None
        };
        // 只在当前帧出现 coverage 时准备 R8 pipeline 和 point sampler。
        let coverage_resources = if coverage_count > 0 {
            // 创建或复用 coverage pipeline 和共享 float8 buffer。
            Some(self.ensure_coverage_resources(frame.device(), sampled_vertex_bytes)?)
        } else {
            // 不含 coverage 时保持空槽。
            None
        };
        // 只在当前帧出现 MSDF 时准备 RGBA8 MSDF pipeline 和资源。
        let msdf_resources = if msdf_count > 0 {
            // 创建或复用 MSDF pipeline、共享 float8 buffer、常量和线性 sampler。
            Some(self.ensure_msdf_resources(frame.device(), sampled_vertex_bytes)?)
        } else {
            // 不含 MSDF 时保持空槽。
            None
        };
        let gradient_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Gradient(_)))
        {
            // 创建或复用渐变 pipeline、unit quad 和 uniform。
            Some(self.ensure_gradient_resources(frame.device())?)
        } else {
            // 不含渐变时保持空槽。
            None
        };
        let shape_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Shape(_) | RhiOp::AdditiveShape(_)))
        {
            // 创建或复用 shape pipeline、unit quad 和 uniform。
            Some(self.ensure_shape_resources(frame.device())?)
        } else {
            // 不含 shape 时保持空槽。
            None
        };
        let shadow_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Shadow(_)))
        {
            // 创建或复用 shadow pipeline、unit quad 和 uniform。
            Some(self.ensure_shadow_resources(frame.device())?)
        } else {
            // 不含 shadow 时保持空槽。
            None
        };
        let sector_resources = if operations
            .iter()
            .any(|operation| matches!(operation, RhiOp::Sector(_)))
        {
            // 创建或复用 sector pipeline、unit quad 和 uniform。
            Some(self.ensure_sector_resources(frame.device())?)
        } else {
            // 不含扇形时保持空槽。
            None
        };
        // 为颜色纹理、coverage texture 和 MSDF atlas page 按操作序号保存 draw 句柄。
        let mut textures = vec![None; operations.len()];
        // 为 coverage 操作保存每个字形在 R8 atlas page 内的归一化 UV。
        let mut coverage_uvs = vec![[0.0, 0.0, 1.0, 1.0]; operations.len()];
        // 为 MSDF 操作保存每个字形在 page 内的归一化 UV。
        let mut msdf_uvs = vec![[0.0, 0.0, 1.0, 1.0]; operations.len()];
        // 为 MSDF 导数 AA 保存实际绑定 texture 的类型化物理尺寸。
        let mut msdf_texture_extents = vec![RhiExtent::new(1, 1); operations.len()];
        // 只记录本帧真正需要销毁的临时 texture，跨帧 atlas page 不进入此列表。
        let mut transient_textures = Vec::new();
        // 创建并上传所有源纹理，避免计划构造中途才发现资源错误。
        for (index, operation) in operations.iter().enumerate() {
            // 物理 1:1 coverage 优先走跨帧 R8 atlas。
            if let RhiOp::Coverage(quad) = operation {
                let (texture, uv, persistent) = match self
                    .ensure_coverage_texture(frame.device(), quad)
                {
                    Ok(result) => result,
                    Err(error) => {
                        let _ = RhiRenderer::destroy_textures(frame.device(), &transient_textures);
                        return Err(error);
                    }
                };
                textures[index] = Some(texture);
                coverage_uvs[index] = uv;
                if !persistent {
                    transient_textures.push(texture);
                }
                continue;
            }
            // MSDF 优先走跨帧 atlas，减少字形源纹理的反复创建和上传。
            if let RhiOp::Msdf(quad) = operation {
                // atlas 失败时先清理当前帧已有的临时 texture。
                let (texture, uv, texture_extent, persistent) =
                    match self.ensure_msdf_texture(frame.device(), quad) {
                        // 返回 atlas page 或本帧临时 texture 及其 UV。
                        Ok(result) => result,
                        // 保留原始资源错误，不伪造可提交的混合计划。
                        Err(error) => {
                            let _ = RhiRenderer::destroy_textures(
                                // 清理只借用 Device 资源生命周期。
                                frame.device(),
                                // 释放本帧已经创建的临时纹理。
                                &transient_textures,
                            );
                            return Err(error);
                        }
                    };
                // 记录 MSDF draw 需要的 page texture 与 placement UV。
                textures[index] = Some(texture);
                msdf_uvs[index] = uv;
                msdf_texture_extents[index] = texture_extent;
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
                // coverage 已在本轮循环开头进入跨帧 atlas。
                RhiOp::Coverage(_) => unreachable!("coverage atlas branch must continue"),
                // MSDF 已在本轮循环开头处理，不能重复进入临时 texture 分支。
                RhiOp::Msdf(_) => unreachable!("MSDF cache branch must continue"),
                // 其他操作没有临时 sampled source。
                _ => continue,
            };
            // 创建对应格式的临时纹理。
            let texture = match frame
                .device()
                .create_texture(TextureDesc::new(extent, format))
            {
                // 记录成功创建的资源。
                Ok(texture) => texture,
                // 创建失败时清理此前已创建的资源。
                Err(error) => {
                    let _ = RhiRenderer::destroy_textures(frame.device(), &transient_textures);
                    return Err(error);
                }
            };
            // 先记录为临时句柄，后续任一步失败都能统一清理。
            textures[index] = Some(texture);
            // 本帧结束后释放颜色或 coverage 临时 texture。
            transient_textures.push(texture);
            // 把资源、完整范围与载荷封闭成一次不可拆纹理上传。
            let upload = RhiTextureUpload::full(texture, extent, payload.as_ref());
            // 上传紧密排列的 source payload。
            if let Err(error) = frame.device().update_texture(upload) {
                // 释放当前帧已经创建的全部临时资源。
                let _ = RhiRenderer::destroy_textures(frame.device(), &transient_textures);
                // 保留上传失败的原始错误。
                return Err(error);
            }
        }
        // 创建不携带 target/load 的混合 pass 命令包。
        let mut pass = frame.new_pass();
        // 所有操作共享同一物理 viewport。
        // 只为本 pass 实际需要的 Gradient 建立一次静态顶点事实。
        if let Some((_, vertex_buffer, _)) = gradient_resources {
            // 先行上传共享类型化 unit quad。
            pass.push(FramePlanCommand::UploadVertex {
                // 绑定 Gradient 静态 vertex buffer。
                buffer: vertex_buffer,
                // 使用统一 float2 payload 工厂。
                data: RhiRenderer::unit_quad_vertex_payload(),
            });
        }
        // 只为本 pass 实际需要的 Shape 建立一次静态顶点事实。
        if let Some((_, _, vertex_buffer, _)) = shape_resources {
            // 先行上传共享类型化 unit quad。
            pass.push(FramePlanCommand::UploadVertex {
                // 绑定 Shape 静态 vertex buffer。
                buffer: vertex_buffer,
                // 使用统一 float2 payload 工厂。
                data: RhiRenderer::unit_quad_vertex_payload(),
            });
        }
        // 只为本 pass 实际需要的 Shadow 建立一次静态顶点事实。
        if let Some((_, vertex_buffer, _)) = shadow_resources {
            // 先行上传共享类型化 unit quad。
            pass.push(FramePlanCommand::UploadVertex {
                // 绑定 Shadow 静态 vertex buffer。
                buffer: vertex_buffer,
                // 使用统一 float2 payload 工厂。
                data: RhiRenderer::unit_quad_vertex_payload(),
            });
        }
        // 只为本 pass 实际需要的 Sector 建立一次静态顶点事实。
        if let Some((_, vertex_buffer, _)) = sector_resources {
            // 先行上传共享类型化 unit quad。
            pass.push(FramePlanCommand::UploadVertex {
                // 绑定 Sector 静态 vertex buffer。
                buffer: vertex_buffer,
                // 使用统一 float2 payload 工厂。
                data: RhiRenderer::unit_quad_vertex_payload(),
            });
        }
        // 只为本 pass 实际需要的 Line 建立一次静态顶点事实。
        if let Some((_, vertex_buffer, _)) = line_resources {
            pass.push(FramePlanCommand::UploadVertex {
                buffer: vertex_buffer,
                data: RhiRenderer::unit_quad_vertex_payload(),
            });
        }
        // 按原始操作顺序追加 command，不能按 pipeline 类型重排。
        let mut index = 0usize;
        while index < operations.len() {
            let operation = &operations[index];
            // 逐类取得对应资源并编码固定 ABI。
            match operation {
                // 编码 solid mesh。
                RhiOp::Solid(mesh) => {
                    // 取得 solid 资源槽。
                    let (pipeline, vertex_buffer, uniform_buffer) = required(
                        solid_resources,
                        "RhiRenderer mixed solid resources are missing",
                    )?;
                    let vertices = solid_vertices[index].as_ref().ok_or_else(|| {
                        super::rhi_state("RhiRenderer mixed solid coverage payload is missing")
                    })?;
                    // 上传当前 mesh 的类型化 position + coverage 顶点。
                    pass.push(FramePlanCommand::UploadVertex {
                        buffer: vertex_buffer,
                        data: FrameVertexPayload::position_coverage_f32(vertices.clone()),
                    });
                    // 上传类型化 MeshConstants。
                    pass.push(FramePlanCommand::UploadUniform {
                        buffer: uniform_buffer,
                        data: FrameUniformPayload::Mesh(RhiRenderer::mesh_uniform(
                            viewport, mesh.rgba,
                        )),
                    });
                    // 追加当前 mesh draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket::new(
                        pipeline,
                        // 绑定当前 mesh 的顶点与 uniform 资源。
                        DrawBufferBindings::new(vertex_buffer, uniform_buffer),
                        // Mesh draw 不使用采样资源。
                        DrawSamplingBinding::none(),
                        // Draw 自有当前 mesh 的 viewport 与 scissor 栅格事实。
                        DrawRasterState::new(viewport, mesh.scissor),
                        // Mesh 使用封闭的非索引顶点范围。
                        DrawRange::vertices((vertices.len() / 3) as u32),
                    )));
                }
                // 编码解析抗锯齿线段。
                RhiOp::Line(line) => {
                    let (pipeline, vertex_buffer, uniform_buffer) = required(
                        line_resources,
                        "RhiRenderer mixed line resources are missing",
                    )?;
                    // 复用四组 float4 的解析图元 ABI：rect 槽保存两端点，参数槽保存线宽。
                    pass.push(FramePlanCommand::UploadUniform {
                        buffer: uniform_buffer,
                        data: FrameUniformPayload::Sector(RhiRenderer::sector_uniform(
                            viewport,
                            [line.start[0], line.start[1], line.end[0], line.end[1]],
                            line.rgba,
                            [line.width, 0.0],
                        )),
                    });
                    pass.push(FramePlanCommand::Draw(DrawPacket::new(
                        pipeline,
                        DrawBufferBindings::new(vertex_buffer, uniform_buffer),
                        DrawSamplingBinding::none(),
                        DrawRasterState::new(viewport, line.scissor),
                        DrawRange::vertices(6),
                    )));
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
                    // 上传类型化 float8 quad 顶点。
                    pass.push(FramePlanCommand::UploadVertex {
                        buffer: vertex_buffer,
                        data: FrameVertexPayload::position_uv_color_f32(textured_vertices(quad)),
                    });
                    // 上传类型化 viewport uniform。
                    pass.push(FramePlanCommand::UploadUniform {
                        buffer: uniform_buffer,
                        data: FrameUniformPayload::Sampled(RhiRenderer::sampled_uniform(viewport)),
                    });
                    // 原子绑定当前颜色纹理与共享 sampler。
                    // 追加当前图片 draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket::new(
                        pipeline,
                        // 绑定当前图片 quad 的顶点与 uniform 资源。
                        DrawBufferBindings::new(vertex_buffer, uniform_buffer),
                        // DrawPacket 直接拥有当前图片的采样绑定。
                        DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
                            texture, sampler, pipeline,
                        )),
                        // Draw 自有当前 textured quad 的 viewport 与 scissor 栅格事实。
                        DrawRasterState::new(viewport, quad.scissor),
                        // 图片 quad 使用封闭的六顶点非索引范围。
                        DrawRange::vertices(6),
                    )));
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
                    // 上传类型化 float8 quad 顶点。
                    pass.push(FramePlanCommand::UploadVertex {
                        buffer: vertex_buffer,
                        data: FrameVertexPayload::position_uv_color_f32(sampled_vertices(quad)),
                    });
                    // 上传类型化 viewport uniform。
                    pass.push(FramePlanCommand::UploadUniform {
                        buffer: uniform_buffer,
                        data: FrameUniformPayload::Sampled(RhiRenderer::surface_sampled_uniform(
                            viewport,
                            quad.surface_corner_radius,
                            quad.surface_shadow_fill,
                            quad.surface_shadow_fill_range,
                        )),
                    });
                    // 原子绑定已经存在的 Picture texture 与共享 sampler。
                    // 追加当前 Picture draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket::new(
                        pipeline,
                        // 绑定当前 Sampled quad 的顶点与 uniform 资源。
                        DrawBufferBindings::new(vertex_buffer, uniform_buffer),
                        // DrawPacket 直接拥有当前 Picture 的采样绑定。
                        DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
                            quad.texture,
                            sampler,
                            pipeline,
                        )),
                        // Draw 自有当前 sampled quad 的 viewport 与 scissor 栅格事实。
                        DrawRasterState::new(viewport, quad.scissor),
                        // Sampled quad 使用封闭的六顶点非索引范围。
                        DrawRange::vertices(6),
                    )));
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
                    // 合并连续且共享 atlas page 与裁剪的 coverage，保持原 painter order。
                    let mut batch_end = index + 1;
                    while let Some(RhiOp::Coverage(next)) = operations.get(batch_end) {
                        if textures[batch_end] != Some(texture) || next.scissor != quad.scissor {
                            break;
                        }
                        batch_end += 1;
                    }
                    let mut vertices = Vec::with_capacity((batch_end - index) * 48);
                    for batch_index in index..batch_end {
                        let RhiOp::Coverage(batch_quad) = &operations[batch_index] else {
                            unreachable!("coverage batch contains a non-coverage operation");
                        };
                        vertices.extend_from_slice(&RhiRenderer::coverage_quad_vertices_with_uv(
                            batch_quad,
                            coverage_uvs[batch_index],
                        ));
                    }
                    // 一次上传完整连续批次，避免每个字形重复更新共享 vertex buffer。
                    pass.push(FramePlanCommand::UploadVertex {
                        buffer: vertex_buffer,
                        data: FrameVertexPayload::position_uv_color_f32(vertices),
                    });
                    // 上传类型化 viewport uniform。
                    pass.push(FramePlanCommand::UploadUniform {
                        buffer: uniform_buffer,
                        data: FrameUniformPayload::Sampled(RhiRenderer::sampled_uniform(viewport)),
                    });
                    // 原子绑定当前 R8 coverage texture 与点采样 sampler。
                    // 追加当前 glyph draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket::new(
                        pipeline,
                        // 绑定当前 Coverage quad 的顶点与 uniform 资源。
                        DrawBufferBindings::new(vertex_buffer, uniform_buffer),
                        // DrawPacket 直接拥有当前 coverage 的采样绑定。
                        DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
                            texture, sampler, pipeline,
                        )),
                        // Draw 自有当前 coverage quad 的 viewport 与 scissor 栅格事实。
                        DrawRasterState::new(viewport, quad.scissor),
                        // 连续 coverage 字形共享一次 draw。
                        DrawRange::vertices(((batch_end - index) * 6) as u32),
                    )));
                    index = batch_end - 1;
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
                    // 只合并连续且共享 atlas page、裁剪与 uniform 的字形，严格保持 painter order。
                    let mut batch_end = index + 1;
                    while let Some(RhiOp::Msdf(next)) = operations.get(batch_end) {
                        if textures[batch_end] != Some(texture)
                            || next.scissor != quad.scissor
                            || next.range.to_bits() != quad.range.to_bits()
                            || msdf_texture_extents[batch_end] != msdf_texture_extents[index]
                        {
                            break;
                        }
                        batch_end += 1;
                    }
                    let mut vertices = Vec::with_capacity((batch_end - index) * 48);
                    for batch_index in index..batch_end {
                        let RhiOp::Msdf(batch_quad) = &operations[batch_index] else {
                            unreachable!("MSDF batch contains a non-MSDF operation");
                        };
                        vertices.extend_from_slice(&RhiRenderer::msdf_quad_vertices_with_uv(
                            batch_quad,
                            msdf_uvs[batch_index],
                        ));
                    }
                    // 一次上传完整连续批次，避免每个字形重复更新共享 vertex buffer。
                    pass.push(FramePlanCommand::UploadVertex {
                        buffer: vertex_buffer,
                        data: FrameVertexPayload::position_uv_color_f32(vertices),
                    });
                    // 上传类型化 viewport、source extent 和 MSDF range 常量。
                    pass.push(FramePlanCommand::UploadUniform {
                        buffer: uniform_buffer,
                        data: FrameUniformPayload::Msdf(RhiRenderer::msdf_uniform(
                            viewport,
                            quad,
                            msdf_texture_extents[index],
                        )),
                    });
                    // 原子绑定当前 RGBA8 MSDF texture 和线性 sampler。
                    // 追加当前 MSDF 字形 draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket::new(
                        pipeline,
                        // 绑定当前 MSDF quad 的顶点与 uniform 资源。
                        DrawBufferBindings::new(vertex_buffer, uniform_buffer),
                        // DrawPacket 直接拥有当前 MSDF 的采样绑定。
                        DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
                            texture, sampler, pipeline,
                        )),
                        // Draw 自有当前 MSDF quad 的 viewport 与 scissor 栅格事实。
                        DrawRasterState::new(viewport, quad.scissor),
                        // 连续字形共享一次 draw，同时保留各自的三角形顺序。
                        DrawRange::vertices(((batch_end - index) * 6) as u32),
                    )));
                    // 跳过已由当前 draw 覆盖的其余连续字形。
                    index = batch_end - 1;
                }
                // 编码渐变矩形。
                RhiOp::Gradient(gradient) => {
                    // 取得渐变资源槽。
                    let (pipeline, vertex_buffer, uniform_buffer) = required(
                        gradient_resources,
                        "RhiRenderer mixed gradient resources are missing",
                    )?;
                    // 选择当前渐变的裁剪。
                    // 上传类型化仿射 GradientConstants。
                    pass.push(FramePlanCommand::UploadUniform {
                        buffer: uniform_buffer,
                        data: FrameUniformPayload::Gradient(RhiRenderer::gradient_uniform(
                            viewport, gradient,
                        )),
                    });
                    // 追加当前渐变 draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket::new(
                        pipeline,
                        // 绑定当前 Gradient quad 的顶点与 uniform 资源。
                        DrawBufferBindings::new(vertex_buffer, uniform_buffer),
                        // Gradient draw 不使用采样资源。
                        DrawSamplingBinding::none(),
                        // Draw 自有当前 gradient 的 viewport 与 scissor 栅格事实。
                        DrawRasterState::new(viewport, gradient.scissor),
                        // Gradient quad 使用封闭的六顶点非索引范围。
                        DrawRange::vertices(6),
                    )));
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
                    // 上传类型化 SectorConstants。
                    pass.push(FramePlanCommand::UploadUniform {
                        buffer: uniform_buffer,
                        data: FrameUniformPayload::Sector(RhiRenderer::sector_uniform(
                            viewport,
                            [sector.x, sector.y, sector.w, sector.h],
                            sector.rgba,
                            [sector.start_angle, sector.sweep_angle],
                        )),
                    });
                    // 追加当前扇形 draw packet。
                    pass.push(FramePlanCommand::Draw(DrawPacket::new(
                        pipeline,
                        // 绑定当前 Sector quad 的顶点与 uniform 资源。
                        DrawBufferBindings::new(vertex_buffer, uniform_buffer),
                        // Sector draw 不使用采样资源。
                        DrawSamplingBinding::none(),
                        // Draw 自有当前 sector 的 viewport 与 scissor 栅格事实。
                        DrawRasterState::new(viewport, sector.scissor),
                        // Sector quad 使用封闭的六顶点非索引范围。
                        DrawRange::vertices(6),
                    )));
                }
                // 编码保留设备四角的仿射阴影。
                RhiOp::Shadow(shadow) => {
                    // 取得 shadow 资源槽。
                    let (pipeline, vertex_buffer, uniform_buffer) = required(
                        shadow_resources,
                        "RhiRenderer mixed shadow resources are missing",
                    )?;
                    // 复用独立 Shadow 路径的类型化 ABI 与 command lowering。
                    RhiRenderer::append_shadow_commands(
                        // 追加到当前 mixed pass，保持 painter order。
                        &mut pass,
                        // 传入当前物理视口。
                        viewport,
                        // 传入已经通过 mixed 契约门禁的载荷。
                        shadow,
                        // 传入固定 BoxShadow pipeline。
                        pipeline,
                        // 传入 Shadow 自己的单位 quad。
                        vertex_buffer,
                        // 传入 Shadow 自己的常量资源。
                        uniform_buffer,
                    );
                }
            }
            index += 1;
        }
        // 将当前 target pass 追加到封闭帧唯一拥有的计划中并保留操作顺序。
        frame.push_pass(load, pass);
        // 封闭帧决定 Surface present 或 Offscreen submit，调用方不再传布尔选择器。
        let execution = frame.execute();
        // 计划结束后只释放本次混合 lowering 创建的临时纹理。
        let cleanup = RhiRenderer::destroy_textures(frame.device(), &transient_textures);
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
}
