//! 通用 GPU Renderer 的迁移期 RHI lowering。
//! 本文件只负责把已经完成设备空间几何降级的 solid mesh 编码为
//! `FramePlan`；它不理解 widget、路径或任何原生 API 对象。

// 引入共享字节载荷和三角 mesh 的所有权类型。
use std::sync::Arc;

// 引入统一错误和最终 damage。
use crate::core::error::{Errc, Error, Result};
use crate::core::PresentDamage;
// 引入薄 RHI 的资源、能力和执行类型。
use crate::native::present::rhi::{
    pipeline_keys, BufferDesc, BufferHandle, BufferUsage, DrawPacket, GraphicsContextRhi,
    LoadAction, PipelineDesc, RhiExtent, RhiScissor, RhiViewport, SamplerDesc, SamplerHandle,
    TextureDesc, TextureFormat, TextureHandle,
};

// 引入当前目录中的有序帧计划类型。
use super::frame_plan::{FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef};

// 将覆盖率 lowering 拆到独立文件，保持通用 renderer 主文件在单文件行数边界内。
#[path = "rhi_renderer_coverage.rs"]
mod coverage;

// 将圆角/描边矩形 lowering 拆到独立文件，保持 renderer 主文件边界清晰。
#[path = "rhi_renderer_shape.rs"]
mod shape;

// 将阴影 lowering 拆到独立文件，复用 shape 的单位 quad 资源。
#[path = "rhi_renderer_shadow.rs"]
mod shadow;

// 将混合 painter-order lowering 拆到独立文件，支持一帧内多个 RHI 语义。
#[path = "rhi_renderer_mixed.rs"]
mod mixed;

// 将渐变计划拆到独立文件，保持 renderer 主文件行数边界。
#[path = "rhi_renderer_gradient.rs"]
mod gradient;

// 将已有纹理的无 present 合成片段拆到独立文件。
#[path = "rhi_renderer_sampled.rs"]
mod sampled;

// 将离屏多阶段 blur lowering 拆到独立文件，保持主 renderer 的资源边界清晰。
#[path = "rhi_renderer_blur.rs"]
mod blur;

// 将 MSDF 字形 lowering 拆到独立文件，保持公共 renderer 的资源边界清晰。
#[path = "rhi_renderer_msdf.rs"]
mod msdf;

// 重新导出阴影 quad，使 GPU submit 只依赖 renderer 的迁移载荷。
pub(crate) use shadow::RhiShadow;
// 重新导出混合 RHI 操作，使 Canvas lowering 只依赖 renderer 语义。
pub(crate) use mixed::{RhiOp, RhiSector};
// 重新导出 MSDF 字形 payload，使 GPU lowering 只依赖 renderer 语义。
pub(crate) use msdf::RhiMsdfQuad;

// 保存一个已经完成几何 lowering 的 mesh 和其裁剪矩形。
#[derive(Debug, Clone)]
pub(crate) struct RhiSolidMesh {
    // 保存交错排列的 xy 三角列表。
    pub(crate) vertices: Arc<[f32]>,
    // 保存与现有 GPU mesh 语义一致的直通颜色。
    pub(crate) rgba: [f32; 4],
    // 保存物理坐标 scissor。
    pub(crate) scissor: Option<RhiScissor>,
}
// 保存一个已经完成几何 lowering 的采样 quad 和其上传纹理。
#[derive(Debug, Clone)]
pub(crate) struct RhiTexturedQuad {
    // 保存左上角物理坐标。
    pub(crate) x: f32,
    // 保存左上角物理坐标。
    pub(crate) y: f32,
    // 保存目标物理宽度。
    pub(crate) w: f32,
    // 保存目标物理高度。
    pub(crate) h: f32,
    // 保存设备空间四角，允许图片采样承载任意可逆仿射变换。
    pub(crate) corners: [[f32; 2]; 4],
    // 保存对 premultiplied 采样结果执行的 tint。
    pub(crate) rgba: [f32; 4],
    // 保存是否使用 premultiplied additive blend，而不是 SrcOver。
    pub(crate) additive: bool,
    // 保存紧密排列的 BGRA premultiplied 源像素。
    pub(crate) pixels: Arc<[u32]>,
    // 保存源纹理宽度。
    pub(crate) pixel_w: u32,
    // 保存源纹理高度。
    pub(crate) pixel_h: u32,
    // 保存当前 quad 的物理裁剪矩形。
    pub(crate) scissor: Option<RhiScissor>,
}
// 保存一个已经存在的 sampled texture quad，供 Picture/离屏纹理合成复用。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RhiSampledQuad {
    // 保存左上角物理坐标。
    pub(crate) x: f32,
    // 保存左上角物理坐标。
    pub(crate) y: f32,
    // 保存目标物理宽度。
    pub(crate) w: f32,
    // 保存目标物理高度。
    pub(crate) h: f32,
    // 保存设备空间四角，Picture texture 与普通图片共用同一几何 ABI。
    pub(crate) corners: [[f32; 2]; 4],
    // 保存对 sampled 结果执行的 tint。
    pub(crate) rgba: [f32; 4],
    // 保存是否使用 Additive blend pipeline。
    pub(crate) additive: bool,
    // 保存已经由 owner 管理的 sampled texture。
    pub(crate) texture: TextureHandle,
    // 保存源纹理左上角的归一化 UV。
    pub(crate) u0: f32,
    // 保存源纹理左上角的归一化 UV。
    pub(crate) v0: f32,
    // 保存源纹理右下角的归一化 UV。
    pub(crate) u1: f32,
    // 保存源纹理右下角的归一化 UV。
    pub(crate) v1: f32,
    // 保存当前 quad 的物理裁剪矩形。
    pub(crate) scissor: Option<RhiScissor>,
}
// 保存一个已经完成几何 lowering 的 R8 字形覆盖率 quad。
#[derive(Debug, Clone)]
pub(crate) struct RhiCoverageQuad {
    // 保存左上角物理坐标。
    pub(crate) x: f32,
    // 保存左上角物理坐标。
    pub(crate) y: f32,
    // 保存目标物理宽度。
    pub(crate) w: f32,
    // 保存目标物理高度。
    pub(crate) h: f32,
    // 保存设备空间四角，覆盖率 quad 可承载旋转和剪切。
    pub(crate) corners: [[f32; 2]; 4],
    // 保存旧 glyph coverage shader 使用的直通颜色。
    pub(crate) rgba: [f32; 4],
    // 保存紧密排列的单通道覆盖率。
    pub(crate) coverage: Arc<[u8]>,
    // 保存源覆盖率宽度。
    pub(crate) pixel_w: u32,
    // 保存源覆盖率高度。
    pub(crate) pixel_h: u32,
    // 保存当前 quad 的物理裁剪矩形。
    pub(crate) scissor: Option<RhiScissor>,
}

// 保存一个已经完成几何 lowering 的线性或径向渐变矩形。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RhiGradientRect {
    // 保存渐变 AABB 左上角物理坐标。
    pub(crate) x: f32,
    // 保存渐变 AABB 左上角物理坐标。
    pub(crate) y: f32,
    // 保存渐变 AABB 物理宽度。
    pub(crate) w: f32,
    // 保存渐变 AABB 物理高度。
    pub(crate) h: f32,
    // 保存目标渐变 quad 的物理四角，shader 以其恢复仿射局部坐标。
    pub(crate) corners: [[f32; 2]; 4],
    // 保存渐变起始颜色。
    pub(crate) color_a: [f32; 4],
    // 保存渐变结束颜色。
    pub(crate) color_b: [f32; 4],
    // 保存 mode、方向或半径参数，布局与固定 shader ABI 对齐。
    pub(crate) params: [f32; 4],
    // 保存当前渐变的物理裁剪矩形。
    pub(crate) scissor: Option<RhiScissor>,
}

// 保存一个已经完成几何 lowering 的圆角或描边矩形。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RhiShapeRect {
    // 保存矩形左上角物理坐标。
    pub(crate) x: f32,
    // 保存矩形左上角物理坐标。
    pub(crate) y: f32,
    // 保存矩形物理宽度。
    pub(crate) w: f32,
    // 保存矩形物理高度。
    pub(crate) h: f32,
    // 保存已经按 UI 颜色语义规整的直通颜色。
    pub(crate) rgba: [f32; 4],
    // 保存左上、右上、右下、左下圆角半径。
    pub(crate) radius: [f32; 4],
    // 保存描边半宽，填充时为零。
    pub(crate) half_stroke: f32,
    // 保存当前矩形的物理裁剪矩形。
    pub(crate) scissor: Option<RhiScissor>,
}

// 持有通用 RHI lowering 需要的可复用资源句柄。
#[derive(Debug, Default)]
pub(crate) struct RhiRenderer {
    // 缓存 solid mesh pipeline。
    solid_pipeline: Option<crate::native::present::rhi::PipelineHandle>,
    // 缓存可写 vertex buffer。
    vertex_buffer: Option<BufferHandle>,
    // 保存 vertex buffer 当前容量。
    vertex_capacity: usize,
    // 缓存 MeshConstants uniform buffer。
    solid_uniform: Option<BufferHandle>,
    // 缓存采样 quad pipeline。
    textured_pipeline: Option<crate::native::present::rhi::PipelineHandle>,
    // 缓存采样 quad 的 Additive pipeline。
    additive_textured_pipeline: Option<crate::native::present::rhi::PipelineHandle>,
    // 缓存 R8 glyph coverage pipeline。
    coverage_pipeline: Option<crate::native::present::rhi::PipelineHandle>,
    // 缓存可写采样 quad vertex buffer。
    textured_vertex_buffer: Option<BufferHandle>,
    // 保存采样 quad vertex buffer 当前容量。
    textured_vertex_capacity: usize,
    // 缓存采样 quad 的 viewport uniform buffer。
    textured_uniform: Option<BufferHandle>,
    // 缓存线性 clamp sampler。
    textured_sampler: Option<SamplerHandle>,
    // 缓存点采样 sampler，保持 R8 glyph coverage 的旧像素语义。
    coverage_sampler: Option<SamplerHandle>,
    // 缓存 MSDF 字形 pipeline。
    msdf_pipeline: Option<crate::native::present::rhi::PipelineHandle>,
    // 缓存 MSDF 字形常量 uniform buffer。
    msdf_uniform: Option<BufferHandle>,
    // 缓存 MSDF 字形的线性 clamp sampler。
    msdf_sampler: Option<SamplerHandle>,
    // 保存跨帧复用的 MSDF RGBA8 atlas pages 与字形 placement。
    msdf_atlas_pages: Vec<msdf::MsdfAtlasPage>,
    // 保存 MSDF atlas 的内容索引，避免每帧重复创建和上传字形纹理。
    msdf_atlas_cache: std::collections::HashMap<msdf::MsdfCacheKey, msdf::MsdfAtlasEntry>,
    // 缓存渐变 pipeline。
    gradient_pipeline: Option<crate::native::present::rhi::PipelineHandle>,
    // 缓存单位 quad vertex buffer。
    gradient_vertex_buffer: Option<BufferHandle>,
    // 缓存渐变常量 uniform buffer。
    gradient_uniform: Option<BufferHandle>,
    // 缓存 SrcOver 与 Additive 圆角/描边矩形 pipeline。
    shape_pipeline: Option<(
        crate::native::present::rhi::PipelineHandle,
        crate::native::present::rhi::PipelineHandle,
    )>,
    // 缓存圆角/描边矩形单位 quad vertex buffer。
    shape_vertex_buffer: Option<BufferHandle>,
    // 缓存圆角/描边矩形常量 uniform buffer。
    shape_uniform: Option<BufferHandle>,
    // 缓存阴影 pipeline。
    shadow_pipeline: Option<crate::native::present::rhi::PipelineHandle>,
    // 缓存原生扇形 pipeline、单位 quad 和常量 uniform。
    sector_pipeline: Option<crate::native::present::rhi::PipelineHandle>,
    sector_vertex_buffer: Option<BufferHandle>,
    sector_uniform: Option<BufferHandle>,
    // 缓存 separable blur pipeline。
    blur_pipeline: Option<crate::native::present::rhi::PipelineHandle>,
    // 缓存 blur 区域 quad 的 float2 vertex buffer。
    blur_vertex_buffer: Option<BufferHandle>,
    // 缓存 BlurConstants uniform buffer。
    blur_uniform: Option<BufferHandle>,
    // 缓存 blur 的线性 clamp sampler。
    blur_sampler: Option<SamplerHandle>,
}

// 为通用 RHI renderer 提供资源准备和 FramePlan 构造。
impl RhiRenderer {
    // 确保 solid mesh 的 pipeline、vertex buffer 和 uniform buffer 已存在。
    fn ensure_solid_resources(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        vertex_bytes: usize,
    ) -> Result<(
        crate::native::present::rhi::PipelineHandle,
        BufferHandle,
        BufferHandle,
    )> {
        // 首次使用时创建固定的 solid mesh pipeline。
        let solid_pipeline = if let Some(pipeline) = self.solid_pipeline {
            // 复用已登记的 pipeline。
            pipeline
        } else {
            // 只选择通用层定义的有限 pipeline key。
            let pipeline = context.create_pipeline(PipelineDesc {
                key: pipeline_keys::SOLID_MESH,
            })?;
            // 缓存 pipeline 句柄。
            self.solid_pipeline = Some(pipeline);
            pipeline
        };
        // 按当前最大 mesh 扩容 vertex buffer。
        let vertex_buffer = if self.vertex_capacity >= vertex_bytes {
            // 复用已有容量。
            self.vertex_buffer
                .ok_or_else(|| rhi_state("solid vertex buffer cache is empty"))?
        } else {
            // 旧帧已经在进入本函数前结束，扩容前可以检查式销毁旧 buffer。
            if let Some(previous) = self.vertex_buffer.take() {
                // 失败时保留 typed error，不把旧资源静默泄漏为成功。
                context.destroy_buffer(previous)?;
            }
            // 创建按 float2 顶点 ABI 绑定的 vertex buffer。
            let buffer = context.create_buffer(BufferDesc {
                size_bytes: vertex_bytes.max(8),
                stride_bytes: (2 * std::mem::size_of::<f32>()) as u32,
                usage: BufferUsage::Vertex,
            })?;
            // 记录新容量和句柄。
            self.vertex_capacity = vertex_bytes.max(8);
            self.vertex_buffer = Some(buffer);
            buffer
        };
        // 首次使用时创建 32 字节 MeshConstants uniform buffer。
        let solid_uniform = if let Some(uniform) = self.solid_uniform {
            // 复用已有 uniform。
            uniform
        } else {
            // D3D11 及其他 adapter 都按 16 字节常量布局对齐。
            let uniform = context.create_buffer(BufferDesc {
                size_bytes: 32,
                stride_bytes: 0,
                usage: BufferUsage::Uniform,
            })?;
            // 缓存 uniform 句柄。
            self.solid_uniform = Some(uniform);
            uniform
        };
        // 返回本次计划需要的资源句柄。
        Ok((solid_pipeline, vertex_buffer, solid_uniform))
    }

    // 确保采样 quad 的 pipeline、vertex/uniform buffer 和 sampler 已存在。
    fn ensure_textured_resources(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<(
        crate::native::present::rhi::PipelineHandle,
        BufferHandle,
        BufferHandle,
        SamplerHandle,
    )> {
        // 首次使用时创建固定的 sampled quad pipeline。
        let textured_pipeline = if let Some(pipeline) = self.textured_pipeline {
            // 复用已经登记的 pipeline。
            pipeline
        } else {
            // 只选择通用层定义的有限 sampled quad key。
            let pipeline = context.create_pipeline(PipelineDesc {
                key: pipeline_keys::TEXTURED_QUAD,
            })?;
            // 缓存 pipeline 句柄。
            self.textured_pipeline = Some(pipeline);
            pipeline
        };
        // 一个 quad 固定使用六个 float8 顶点。
        let quad_bytes = 6 * 8 * std::mem::size_of::<f32>();
        // 首次使用或容量异常时创建采样 quad vertex buffer。
        let textured_vertex_buffer = if self.textured_vertex_capacity >= quad_bytes {
            // 复用已有容量。
            self.textured_vertex_buffer
                .ok_or_else(|| rhi_state("textured vertex buffer cache is empty"))?
        } else {
            // 创建按 position/uv/color float8 ABI 绑定的 vertex buffer。
            let buffer = context.create_buffer(BufferDesc {
                size_bytes: quad_bytes,
                stride_bytes: (8 * std::mem::size_of::<f32>()) as u32,
                usage: BufferUsage::Vertex,
            })?;
            // 记录容量和句柄。
            self.textured_vertex_capacity = quad_bytes;
            self.textured_vertex_buffer = Some(buffer);
            buffer
        };
        // 首次使用时创建 16 字节 viewport uniform buffer。
        let textured_uniform = if let Some(uniform) = self.textured_uniform {
            // 复用已有 uniform。
            uniform
        } else {
            // sampled quad 的 VS 只读取 viewport.xy 和 padding.xy。
            let uniform = context.create_buffer(BufferDesc {
                size_bytes: 16,
                stride_bytes: 0,
                usage: BufferUsage::Uniform,
            })?;
            // 缓存 uniform 句柄。
            self.textured_uniform = Some(uniform);
            uniform
        };
        // 首次使用时创建线性 clamp sampler。
        let textured_sampler = if let Some(sampler) = self.textured_sampler {
            // 复用已有 sampler。
            sampler
        } else {
            // 图片缩放需要线性过滤，边缘不能采样到邻接资源。
            let sampler = context.create_sampler(SamplerDesc { linear: true })?;
            // 缓存 sampler 句柄。
            self.textured_sampler = Some(sampler);
            sampler
        };
        // 返回本次计划需要的固定资源。
        Ok((
            textured_pipeline,
            textured_vertex_buffer,
            textured_uniform,
            textured_sampler,
        ))
    }

    // 确保 Additive sampled quad 复用同一顶点资源但使用独立 blend pipeline。
    fn ensure_additive_textured_pipeline(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<crate::native::present::rhi::PipelineHandle> {
        // 已有 pipeline 时直接复用，避免每个图片 quad 重复登记资源。
        if let Some(pipeline) = self.additive_textured_pipeline {
            // 返回已经创建的加法 pipeline。
            return Ok(pipeline);
        }
        // 只选择通用层定义的 Additive sampled quad key。
        let pipeline = context.create_pipeline(PipelineDesc {
            key: pipeline_keys::TEXTURED_QUAD_ADDITIVE,
        })?;
        // 缓存 pipeline 句柄，后续帧保持同一资源身份。
        self.additive_textured_pipeline = Some(pipeline);
        // 返回刚刚创建的 pipeline。
        Ok(pipeline)
    }

    // 确保渐变 pipeline、单位 quad vertex buffer 和常量 buffer 已存在。
    fn ensure_gradient_resources(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<(
        crate::native::present::rhi::PipelineHandle,
        BufferHandle,
        BufferHandle,
    )> {
        // 首次使用时创建固定的渐变 pipeline。
        let gradient_pipeline = if let Some(pipeline) = self.gradient_pipeline {
            // 复用已经登记的 pipeline。
            pipeline
        } else {
            // 只选择通用层定义的有限 gradient key。
            let pipeline = context.create_pipeline(PipelineDesc {
                key: pipeline_keys::GRADIENT_RECT,
            })?;
            // 缓存 pipeline 句柄。
            self.gradient_pipeline = Some(pipeline);
            pipeline
        };
        // 单位 quad 使用六个 float2 顶点。
        let unit_vertices = [
            0.0f32, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0,
        ];
        // 首次使用时创建单位 quad buffer。
        let gradient_vertex_buffer = if let Some(buffer) = self.gradient_vertex_buffer {
            // 复用已有 vertex buffer。
            buffer
        } else {
            // 创建位置 float2 ABI 的默认 vertex buffer。
            let buffer = context.create_buffer(BufferDesc {
                size_bytes: unit_vertices.len() * std::mem::size_of::<f32>(),
                stride_bytes: (2 * std::mem::size_of::<f32>()) as u32,
                usage: BufferUsage::Vertex,
            })?;
            // 首次绑定前上传单位 quad。
            context.update_buffer(buffer, 0, &Self::encode_f32s(&unit_vertices))?;
            // 缓存单位 quad 句柄。
            self.gradient_vertex_buffer = Some(buffer);
            buffer
        };
        // 首次使用时创建 96 字节 affine GradientConstants uniform buffer。
        let gradient_uniform = if let Some(uniform) = self.gradient_uniform {
            // 复用已有 uniform。
            uniform
        } else {
            // GradientConstants = viewport、origin/edge_x、edge_y、两色与参数。
            let uniform = context.create_buffer(BufferDesc {
                size_bytes: 96,
                stride_bytes: 0,
                usage: BufferUsage::Uniform,
            })?;
            // 缓存渐变 uniform 句柄。
            self.gradient_uniform = Some(uniform);
            uniform
        };
        // 返回本次计划需要的渐变资源。
        Ok((gradient_pipeline, gradient_vertex_buffer, gradient_uniform))
    }

    // 把 f32 切片编码为当前 host 的紧密字节载荷。
    fn encode_f32s(values: &[f32]) -> Arc<[u8]> {
        // 预留精确容量，避免资源上传前再次扩容。
        let mut bytes = Vec::with_capacity(values.len() * std::mem::size_of::<f32>());
        // 使用 native endian，adapter 与 CPU 在同一 host 上解释 uniform/vertex。
        for value in values {
            // 保持每个 float 的原始 IEEE 字节表示。
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        // 转为不可变共享载荷交给 FramePlan。
        Arc::from(bytes)
    }

    // 把 u32 BGRA 像素编码成纹理上传所需的原生字节序列。
    fn encode_u32s(values: &[u32]) -> Arc<[u8]> {
        // 预留精确容量，避免上传前再次扩容。
        let mut bytes = Vec::with_capacity(values.len() * std::mem::size_of::<u32>());
        // D3D11 B8G8R8A8_UNORM 在 Windows 小端内存中与该布局一致。
        for value in values {
            // 保持每个 packed pixel 的原始字节表示。
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        // 转为不可变共享载荷交给资源上传。
        Arc::from(bytes)
    }

    // 构造并执行一帧 solid mesh RHI 计划。
    pub(crate) fn execute_solid_meshes(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        viewport: RhiViewport,
        load: LoadAction,
        target: RenderTargetRef,
        meshes: &[RhiSolidMesh],
    ) -> Result<()> {
        // 空列表不应伪造一次 present。
        if meshes.is_empty() {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("RhiRenderer cannot execute an empty mesh list"));
        }
        // 拒绝非有限 viewport，避免计划构造和 adapter 结果分叉。
        if !viewport.is_valid() {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("RhiRenderer viewport is invalid"));
        }
        // 计算本帧需要的最大顶点字节数。
        let max_vertex_bytes = meshes
            .iter()
            .map(|mesh| mesh.vertices.len() * std::mem::size_of::<f32>())
            .max()
            .unwrap_or(0);
        // 准备可复用的 RHI 资源。
        let (pipeline, vertex_buffer, uniform_buffer) =
            self.ensure_solid_resources(context, max_vertex_bytes)?;
        // 计划使用当前 context 的 surface 代际。
        let surface = context.token();
        // 创建 surface pass，并保留调用方的 load/clear 语义。
        let mut pass = RenderPassPlan::new(target, load);
        // 统一设置物理 viewport。
        pass.push(FramePlanCommand::SetViewport(viewport));
        // 为每个 mesh 保留 painter order 和独立 scissor。
        for mesh in meshes {
            // 顶点必须是完整的 xy 三角列表。
            if mesh.vertices.len() < 6 || mesh.vertices.len() % 2 != 0 {
                // 返回稳定的参数错误。
                return Err(rhi_invalid("RhiRenderer solid mesh vertex ABI is invalid"));
            }
            // 三角列表必须包含完整的三个顶点一组。
            let vertex_count = (mesh.vertices.len() / 2) as u32;
            if !vertex_count.is_multiple_of(3) {
                // 返回稳定的参数错误。
                return Err(rhi_invalid(
                    "RhiRenderer solid mesh is not triangle-aligned",
                ));
            }
            // 在 draw 前设置当前 mesh 的 clip。
            pass.push(FramePlanCommand::SetScissor(mesh.scissor));
            // 上传当前 mesh 的 vertex 数据。
            pass.push(FramePlanCommand::UpdateBuffer {
                buffer: vertex_buffer,
                offset: 0,
                data: Self::encode_f32s(mesh.vertices.as_ref()),
            });
            // MeshConstants = viewport.xy、padding.xy、color.rgba。
            pass.push(FramePlanCommand::UpdateBuffer {
                buffer: uniform_buffer,
                offset: 0,
                data: Self::encode_f32s(&[
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
            // 追加一个非索引 solid mesh draw packet。
            pass.push(FramePlanCommand::Draw(DrawPacket {
                pipeline,
                vertex_buffer,
                index_buffer: None,
                uniform_buffer: Some(uniform_buffer),
                vertex_count,
                index_count: 0,
                first_vertex: 0,
                first_index: 0,
                base_vertex: 0,
            }));
        }
        // 交给唯一的 FramePlan present 边界执行。
        let plan = {
            // 创建计划并追加唯一 surface pass。
            let mut plan = FramePlan::new(surface, damage);
            // 保留 pass 的严格 painter order。
            plan.push_pass(pass);
            plan
        };
        // surface 计划最终 present，texture 计划只执行离屏 submit。
        execute_plan_for_target(context, &plan, target)?;
        // 资源由 renderer 跨帧复用，不能在这里销毁。
        Ok(())
    }

    // 清理本次图片帧临时创建的 texture，并保留第一个资源错误。
    fn destroy_textures(
        context: &mut dyn GraphicsContextRhi,
        textures: &[TextureHandle],
    ) -> Result<()> {
        // 逐个销毁资源，不能因一个失败而留下后续资源。
        let mut first_error = None;
        // 只在计划已经结束后释放本帧 texture。
        for texture in textures {
            // 保留首个错误，同时继续尝试释放剩余资源。
            if let Err(error) = context.destroy_texture(*texture) {
                // 不把后续错误覆盖掉导致定位困难。
                first_error.get_or_insert(error);
            }
        }
        // 没有错误时返回资源清理成功。
        first_error.map_or(Ok(()), Err)
    }

    // 构造并执行一帧 BGRA premultiplied 图片 quad RHI 计划。
    pub(crate) fn execute_textured_quads(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        viewport: RhiViewport,
        load: LoadAction,
        target: RenderTargetRef,
        quads: &[RhiTexturedQuad],
    ) -> Result<()> {
        // 空列表不应伪造一次 present。
        if quads.is_empty() {
            // 返回稳定的参数错误。
            return Err(rhi_invalid(
                "RhiRenderer cannot execute an empty texture list",
            ));
        }
        // 拒绝非有限 viewport，避免计划构造和 adapter 结果分叉。
        if !viewport.is_valid() {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("RhiRenderer textured viewport is invalid"));
        }
        // 在创建任何 native texture 前验证所有 quad 的静态载荷。
        for quad in quads {
            // 目标矩形和 tint 必须是有限的正值。
            if !quad.x.is_finite()
                || !quad.y.is_finite()
                || !quad.w.is_finite()
                || !quad.h.is_finite()
                || quad.w <= 0.0
                || quad.h <= 0.0
                || quad.rgba.iter().any(|channel| !channel.is_finite())
            {
                // 返回稳定的参数错误。
                return Err(rhi_invalid("RhiRenderer textured quad geometry is invalid"));
            }
            // 纹理尺寸必须严格为正。
            if quad.pixel_w == 0 || quad.pixel_h == 0 {
                // 返回稳定的参数错误。
                return Err(rhi_invalid("RhiRenderer texture extent is empty"));
            }
            // 防止像素数量乘法溢出或 payload 与描述不一致。
            let pixel_count = (quad.pixel_w as usize)
                .checked_mul(quad.pixel_h as usize)
                .ok_or_else(|| rhi_invalid("RhiRenderer texture extent overflows"))?;
            if quad.pixels.len() != pixel_count {
                // 返回稳定的参数错误。
                return Err(rhi_invalid("RhiRenderer texture payload length is invalid"));
            }
            // 显式 scissor 必须已经完成物理坐标 lowering。
            if quad.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(rhi_invalid("RhiRenderer textured scissor is invalid"));
            }
        }
        // 准备采样 quad 的固定 RHI 资源。
        let (pipeline, vertex_buffer, uniform_buffer, sampler) =
            self.ensure_textured_resources(context)?;
        // 只有当前帧出现 Additive quad 时才创建对应的 blend pipeline。
        let additive_pipeline = if quads.iter().any(|quad| quad.additive) {
            // 为加法图片建立独立 pipeline，保持 SrcOver 与 Additive 不混淆。
            Some(self.ensure_additive_textured_pipeline(context)?)
        } else {
            // 纯 SrcOver 帧不需要额外的 pipeline。
            None
        };
        // 为本次帧逐项创建、上传并记录临时 texture。
        let mut textures = Vec::with_capacity(quads.len());
        for quad in quads {
            // 创建与源像素布局一致的 BGRA texture。
            let texture = match context.create_texture(TextureDesc {
                extent: RhiExtent::new(quad.pixel_w, quad.pixel_h),
                format: TextureFormat::Bgra8Unorm,
            }) {
                // 资源成功创建后进入统一清理列表。
                Ok(texture) => texture,
                // 创建失败时先释放已创建资源，再返回原始错误。
                Err(error) => {
                    let _ = Self::destroy_textures(context, &textures);
                    return Err(error);
                }
            };
            // 上传纹理前把 packed pixels 转为紧密字节载荷。
            let upload = Self::encode_u32s(quad.pixels.as_ref());
            // 资源上传失败时不能把半成品 texture 留在 adapter。
            if let Err(error) =
                context.update_texture(texture, RhiExtent::new(quad.pixel_w, quad.pixel_h), &upload)
            {
                // 把当前失败资源加入清理列表。
                textures.push(texture);
                // 尝试释放所有已经创建的图片资源。
                let _ = Self::destroy_textures(context, &textures);
                // 保留上传失败的真实错误。
                return Err(error);
            }
            // 记录上传完成且可以进入 FramePlan 的 texture。
            textures.push(texture);
        }
        // 计划使用当前 context 的 surface 代际。
        let surface = context.token();
        // 创建 surface pass，并保留调用方的 load/clear 语义。
        let mut pass = RenderPassPlan::new(target, load);
        // 所有图片共享同一个物理 viewport。
        pass.push(FramePlanCommand::SetViewport(viewport));
        // 每个 quad 以独立的 texture binding 和 scissor 保留 painter order。
        for (quad, texture) in quads.iter().zip(textures.iter().copied()) {
            // 每个 quad 根据其 blend 事实选择固定 pipeline。
            let quad_pipeline = if quad.additive {
                // Additive quad 必须已经准备好独立 pipeline。
                additive_pipeline
                    .ok_or_else(|| rhi_state("RhiRenderer additive textured pipeline is missing"))?
            } else {
                // 默认图片使用 premultiplied SrcOver pipeline。
                pipeline
            };
            // 生成 position/uv/color float8 的两个三角形。
            let vertices = [
                quad.x,
                quad.y,
                0.0,
                0.0,
                quad.rgba[0],
                quad.rgba[1],
                quad.rgba[2],
                quad.rgba[3],
                quad.x + quad.w,
                quad.y,
                1.0,
                0.0,
                quad.rgba[0],
                quad.rgba[1],
                quad.rgba[2],
                quad.rgba[3],
                quad.x + quad.w,
                quad.y + quad.h,
                1.0,
                1.0,
                quad.rgba[0],
                quad.rgba[1],
                quad.rgba[2],
                quad.rgba[3],
                quad.x,
                quad.y,
                0.0,
                0.0,
                quad.rgba[0],
                quad.rgba[1],
                quad.rgba[2],
                quad.rgba[3],
                quad.x + quad.w,
                quad.y + quad.h,
                1.0,
                1.0,
                quad.rgba[0],
                quad.rgba[1],
                quad.rgba[2],
                quad.rgba[3],
                quad.x,
                quad.y + quad.h,
                0.0,
                1.0,
                quad.rgba[0],
                quad.rgba[1],
                quad.rgba[2],
                quad.rgba[3],
            ];
            // 在 draw 前设置当前图片的裁剪。
            pass.push(FramePlanCommand::SetScissor(quad.scissor));
            // 上传当前 quad 的顶点数据。
            pass.push(FramePlanCommand::UpdateBuffer {
                buffer: vertex_buffer,
                offset: 0,
                data: Self::encode_f32s(&vertices),
            });
            // 上传当前 pass 的物理 viewport uniform。
            pass.push(FramePlanCommand::UpdateBuffer {
                buffer: uniform_buffer,
                offset: 0,
                data: Self::encode_f32s(&[viewport.width, viewport.height, 0.0, 0.0]),
            });
            // 绑定当前纹理和共享 sampler。
            pass.push(FramePlanCommand::BindTexture {
                slot: 0,
                texture,
                sampler,
            });
            // 追加六顶点的非索引采样 quad draw packet。
            pass.push(FramePlanCommand::Draw(DrawPacket {
                pipeline: quad_pipeline,
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
        // 创建计划并追加唯一 surface pass。
        let mut plan = FramePlan::new(surface, damage);
        // 保留图片 painter order。
        plan.push_pass(pass);
        // surface 计划最终 present，texture 计划只执行离屏 submit。
        let execution = execute_plan_for_target(context, &plan, target);
        // 计划结束后释放本次图片的临时 texture。
        let cleanup = Self::destroy_textures(context, &textures);
        // 优先返回绘制或 present 失败；否则报告资源清理失败。
        match (execution, cleanup) {
            // 计划失败时保留原始执行错误。
            (Err(error), _) => Err(error),
            // 计划成功但清理失败时仍不能伪造完整成功。
            (Ok(_), Err(error)) => Err(error),
            // 计划与资源清理均成功。
            (Ok(_), Ok(())) => Ok(()),
        }
    }
}
// 按目标类型选择最终 present 或离屏 submit 执行边界。
fn execute_plan_for_target(
    context: &mut dyn GraphicsContextRhi,
    plan: &FramePlan,
    target: RenderTargetRef,
) -> Result<()> {
    // surface 计划必须保留唯一 acquire/submit/present 语义。
    if matches!(target, RenderTargetRef::Surface) {
        // 该日志只在已经取得组合 RHI context 后触发，可用于区分真实 RHI 提交与兼容路径。
        tracing::debug!("Graphics RHI FramePlan submit: target={target:?}");
        // 丢弃成功提交的 FrameCommit，调用方只关心 lowering 是否成功。
        plan.execute_on_context(context)?;
    } else {
        // 离屏计划同样经过组合 RHI context，但不触发主 surface present。
        tracing::debug!("Graphics RHI FramePlan submit: target={target:?}");
        // 离屏计划只执行 render pass 与 submit，不触发主 surface present。
        plan.execute_offscreen_on_context(context)?;
    }
    // 返回统一的 lowering 成功结果。
    Ok(())
}
// 按目标类型选择不触发 present 的 surface segment 或离屏 submit 边界。
fn execute_plan_without_present(
    context: &mut dyn GraphicsContextRhi,
    plan: &FramePlan,
    target: RenderTargetRef,
) -> Result<()> {
    // surface segment 只 acquire/submit，最终 present 由外层帧边界统一完成。
    if matches!(target, RenderTargetRef::Surface) {
        // 丢弃 segment 的提交身份，调用方只关心 command 是否落入当前 surface。
        plan.execute_surface_segment_on_context(context)?;
    } else {
        // texture target 仍使用无 present 的离屏执行边界。
        plan.execute_offscreen_on_context(context)?;
    }
    // 返回统一的 lowering 成功结果。
    Ok(())
}

// 生成 lowering 阶段的参数错误。
fn rhi_invalid(message: &'static str) -> Error {
    // 统一使用 InvalidArgument，避免与 native platform failure 混淆。
    Error::new(Errc::InvalidArgument, message)
}
// 生成 lowering 阶段的状态错误。
fn rhi_state(message: &'static str) -> Error {
    // 资源缓存状态破坏属于 renderer 内部状态错误。
    Error::new(Errc::InvalidState, message)
}
