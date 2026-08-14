//! 通用 GPU Renderer 的 R8 字形 coverage lowering。

// 引入共享字节载荷。
use std::sync::Arc;

// 引入最终 damage 和薄 RHI 资源类型。
use crate::core::PresentDamage;
// 引入 RHI 计划执行所需的资源描述与句柄。
use crate::native::present::rhi::{
    BufferHandle, DrawPacket, GraphicsContextRhi, LoadAction, PipelineDesc, PipelineHandle,
    RhiExtent, SamplerDesc, SamplerHandle, TextureDesc, TextureFormat, pipeline_keys,
};

// 复用 renderer 主模块的计划类型和 coverage payload。
use super::{
    FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef, RhiCoverageQuad, RhiRenderer,
};

// 为 coverage shader 创建或复用 R8 专用的 pipeline、buffer 和 sampler。
impl RhiRenderer {
    // 准备与普通 sampled quad 共用顶点/viewport ABI 的 coverage 资源。
    pub(super) fn ensure_coverage_resources(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<(PipelineHandle, BufferHandle, BufferHandle, SamplerHandle), crate::core::Error>
    {
        // 复用普通 sampled quad 的 float8 顶点 buffer 和 viewport uniform。
        let (_, vertex_buffer, uniform_buffer, _) = self.ensure_textured_resources(context)?;
        // 首次使用时创建 R8 coverage 专用 pipeline。
        let pipeline = if let Some(pipeline) = self.coverage_pipeline {
            // 复用已经登记的 coverage pipeline。
            pipeline
        } else {
            // 选择只由 RHI 契约定义的 coverage pipeline key。
            let pipeline = context.create_pipeline(PipelineDesc {
                key: pipeline_keys::GLYPH_COVERAGE_QUAD,
            })?;
            // 缓存 coverage pipeline 句柄。
            self.coverage_pipeline = Some(pipeline);
            pipeline
        };
        // 首次使用时创建点采样 sampler，保持旧 R8 atlas 的像素边界语义。
        let sampler = if let Some(sampler) = self.coverage_sampler {
            // 复用已登记的点采样 sampler。
            sampler
        } else {
            // coverage 采样不跨 glyph 像素做线性插值。
            let sampler = context.create_sampler(SamplerDesc { linear: false })?;
            // 缓存 coverage sampler 句柄。
            self.coverage_sampler = Some(sampler);
            sampler
        };
        // 返回 coverage draw 所需的固定资源。
        Ok((pipeline, vertex_buffer, uniform_buffer, sampler))
    }

    // 把单通道 coverage 编码为紧密 R8 上传载荷。
    pub(super) fn encode_coverage(values: &[u8]) -> Arc<[u8]> {
        // 保持 coverage 原始字节值，不进行颜色或 alpha 转换。
        Arc::from(values.to_vec())
    }

    // 生成 position/uv/color float8 的两个三角形。
    pub(super) fn coverage_quad_vertices(quad: &RhiCoverageQuad) -> [f32; 48] {
        // 保存顶点颜色，交给旧 glyph shader 做量化和 premultiply。
        let color = quad.rgba;
        // 返回左上、右上、右下、左下组成的三角列表。
        [
            quad.corners[0][0],
            quad.corners[0][1],
            0.0,
            0.0,
            color[0],
            color[1],
            color[2],
            color[3],
            quad.corners[1][0],
            quad.corners[1][1],
            1.0,
            0.0,
            color[0],
            color[1],
            color[2],
            color[3],
            quad.corners[2][0],
            quad.corners[2][1],
            1.0,
            1.0,
            color[0],
            color[1],
            color[2],
            color[3],
            quad.corners[0][0],
            quad.corners[0][1],
            0.0,
            0.0,
            color[0],
            color[1],
            color[2],
            color[3],
            quad.corners[2][0],
            quad.corners[2][1],
            1.0,
            1.0,
            color[0],
            color[1],
            color[2],
            color[3],
            quad.corners[3][0],
            quad.corners[3][1],
            0.0,
            1.0,
            color[0],
            color[1],
            color[2],
            color[3],
        ]
    }

    // 执行一帧 R8 字形 coverage quad RHI 计划。
    pub(crate) fn execute_coverage_quads(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        viewport: super::RhiViewport,
        load: LoadAction,
        target: RenderTargetRef,
        quads: &[RhiCoverageQuad],
    ) -> Result<(), crate::core::Error> {
        // 空列表不应伪造一次 present。
        if quads.is_empty() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid(
                "RhiRenderer cannot execute an empty coverage list",
            ));
        }
        // 拒绝非有限 viewport，避免计划构造和 adapter 结果分叉。
        if !viewport.is_valid() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid(
                "RhiRenderer coverage viewport is invalid",
            ));
        }
        // 在创建任何 native texture 前验证所有 coverage quad 的静态载荷。
        for quad in quads {
            // 目标矩形和颜色必须是有限的正值。
            if !quad.x.is_finite()
                || !quad.y.is_finite()
                || !quad.w.is_finite()
                || !quad.h.is_finite()
                || quad.w <= 0.0
                || quad.h <= 0.0
                || quad
                    .corners
                    .iter()
                    .flatten()
                    .any(|coordinate| !coordinate.is_finite())
                || quad.rgba.iter().any(|channel| !channel.is_finite())
            {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid(
                    "RhiRenderer coverage quad geometry is invalid",
                ));
            }
            // 覆盖率纹理尺寸必须严格为正。
            if quad.pixel_w == 0 || quad.pixel_h == 0 {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid("RhiRenderer coverage extent is empty"));
            }
            // 防止 coverage 数量乘法溢出或 payload 与描述不一致。
            let pixel_count = (quad.pixel_w as usize)
                .checked_mul(quad.pixel_h as usize)
                .ok_or_else(|| super::rhi_invalid("RhiRenderer coverage extent overflows"))?;
            if quad.coverage.len() != pixel_count {
                // 只接受紧密排列的 R8 coverage，避免行步长被误解。
                return Err(super::rhi_invalid(
                    "RhiRenderer coverage payload length is invalid",
                ));
            }
            // 显式 scissor 必须已经完成物理坐标 lowering。
            if quad.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid(
                    "RhiRenderer coverage scissor is invalid",
                ));
            }
        }
        // 准备 coverage pipeline、共享 vertex/uniform 和点采样 sampler。
        let (pipeline, vertex_buffer, uniform_buffer, sampler) =
            self.ensure_coverage_resources(context)?;
        // 为本次帧逐项创建、上传并记录临时 R8 texture。
        let mut textures = Vec::with_capacity(quads.len());
        for quad in quads {
            // 创建只含 shader resource view 的 R8 coverage texture。
            let texture = match context.create_texture(TextureDesc {
                extent: RhiExtent::new(quad.pixel_w, quad.pixel_h),
                format: TextureFormat::R8Unorm,
            }) {
                // 资源成功创建后进入统一清理列表。
                Ok(texture) => texture,
                // 创建失败时先释放已创建资源，再返回原始错误。
                Err(error) => {
                    let _ = Self::destroy_textures(context, &textures);
                    return Err(error);
                }
            };
            // 上传紧密的 R8 coverage 字节。
            let upload = Self::encode_coverage(quad.coverage.as_ref());
            // 资源上传失败时不能把半成品 texture 留在 adapter。
            if let Err(error) =
                context.update_texture(texture, RhiExtent::new(quad.pixel_w, quad.pixel_h), &upload)
            {
                // 把当前失败资源加入清理列表。
                textures.push(texture);
                // 尝试释放所有已经创建的 coverage 资源。
                let _ = Self::destroy_textures(context, &textures);
                // 保留上传失败的真实错误。
                return Err(error);
            }
            // 记录上传完成且可以进入 FramePlan 的 coverage texture。
            textures.push(texture);
        }
        // 计划使用当前 context 的 surface 代际。
        let surface = context.token();
        // 创建 surface pass，并保留调用方的 load/clear 语义。
        let mut pass = RenderPassPlan::new(target, load);
        // 所有 coverage quad 共享同一个物理 viewport。
        pass.push(FramePlanCommand::SetViewport(viewport));
        // 每个 glyph 以独立 texture binding 和 scissor 保留 painter order。
        for (quad, texture) in quads.iter().zip(textures.iter().copied()) {
            // 生成当前 glyph 的顶点数据。
            let vertices = Self::coverage_quad_vertices(quad);
            // 在 draw 前设置当前 glyph 的裁剪。
            pass.push(FramePlanCommand::SetScissor(quad.scissor));
            // 上传当前 glyph 的顶点数据。
            pass.push(FramePlanCommand::UpdateBuffer {
                buffer: vertex_buffer,
                offset: 0,
                data: super::RhiRenderer::encode_f32s(&vertices),
            });
            // 上传当前 pass 的物理 viewport uniform。
            pass.push(FramePlanCommand::UpdateBuffer {
                buffer: uniform_buffer,
                offset: 0,
                data: super::RhiRenderer::encode_f32s(&[viewport.width, viewport.height, 0.0, 0.0]),
            });
            // 绑定当前 R8 coverage texture 和点采样 sampler。
            pass.push(FramePlanCommand::BindTexture {
                slot: 0,
                texture,
                sampler,
            });
            // 追加六顶点的非索引 coverage quad draw packet。
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
        // 创建计划并追加唯一 surface pass。
        let mut plan = FramePlan::new(surface, damage);
        // 保留 glyph painter order。
        plan.push_pass(pass);
        // surface 计划最终 present，texture 计划只执行离屏 submit。
        let execution = super::execute_plan_for_target(context, &plan, target);
        // 计划结束后释放本次 glyph 的临时 texture。
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
