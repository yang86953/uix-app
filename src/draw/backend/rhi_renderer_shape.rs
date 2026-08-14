//! 通用 GPU Renderer 的圆角与描边矩形 lowering。

// 引入最终 damage 和薄 RHI 资源类型。
use crate::core::PresentDamage;
// 引入 RHI 计划执行所需的资源描述与句柄。
use crate::native::present::rhi::{
    BufferDesc, BufferHandle, BufferUsage, DrawPacket, GraphicsContextRhi, LoadAction,
    PipelineDesc, PipelineHandle, RhiViewport, pipeline_keys,
};

// 复用 renderer 主模块的计划类型和 shape payload。
use super::{
    FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef, RhiRenderer, RhiShapeRect,
};

// 为 shape shader 创建或复用单位 quad、pipeline 和常量 buffer。
impl RhiRenderer {
    // 准备固定 position float2 ABI 的 shape 资源。
    pub(super) fn ensure_shape_resources(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
    ) -> Result<(PipelineHandle, PipelineHandle, BufferHandle, BufferHandle), crate::core::Error>
    {
        // 首次使用时创建 SrcOver 与 Additive 两个固定 shape pipeline。
        let (pipeline, additive_pipeline) =
            if let Some((pipeline, additive_pipeline)) = self.shape_pipeline {
                // 复用已经登记的 shape pipeline。
                (pipeline, additive_pipeline)
            } else {
                // 只选择通用层定义的 shape pipeline key。
                let pipeline = context.create_pipeline(PipelineDesc {
                    key: pipeline_keys::SHAPE_RECT,
                })?;
                // 创建同 ABI 但使用 Additive blend 的 shape pipeline。
                let additive_pipeline = context.create_pipeline(PipelineDesc {
                    key: pipeline_keys::SHAPE_RECT_ADDITIVE,
                })?;
                // 缓存两个 shape pipeline 句柄。
                self.shape_pipeline = Some((pipeline, additive_pipeline));
                (pipeline, additive_pipeline)
            };
        // 单位 quad 使用六个 float2 顶点。
        let unit_vertices = [
            0.0f32, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0,
        ];
        // 首次使用时创建单位 quad buffer。
        let vertex_buffer = if let Some(buffer) = self.shape_vertex_buffer {
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
            context.update_buffer(buffer, 0, &RhiRenderer::encode_f32s(&unit_vertices))?;
            // 缓存单位 quad 句柄。
            self.shape_vertex_buffer = Some(buffer);
            buffer
        };
        // 首次使用时创建可被 shape 与 affine shadow 共用的 96 字节 uniform buffer。
        let uniform_buffer = if let Some(uniform) = self.shape_uniform {
            // 复用已有 uniform buffer。
            uniform
        } else {
            // RectConstants 占前五个 float4，末一个 float4 供 affine shadow 共用。
            let uniform = context.create_buffer(BufferDesc {
                size_bytes: 96,
                stride_bytes: 0,
                usage: BufferUsage::Uniform,
            })?;
            // 缓存 shape uniform 句柄。
            self.shape_uniform = Some(uniform);
            uniform
        };
        // 返回 SrcOver、Additive 及共享 buffer 资源。
        Ok((pipeline, additive_pipeline, vertex_buffer, uniform_buffer))
    }

    // 将一个 shape 的固定常量和 draw packet 追加到既有 pass。
    pub(super) fn append_shape_commands(
        pass: &mut RenderPassPlan,
        viewport: RhiViewport,
        rect: &RhiShapeRect,
        pipeline: PipelineHandle,
        vertex_buffer: BufferHandle,
        uniform_buffer: BufferHandle,
    ) {
        // 选择当前矩形的物理裁剪。
        pass.push(FramePlanCommand::SetScissor(rect.scissor));
        // 上传保持 D3D11 16-byte 对齐的 RectConstants。
        pass.push(FramePlanCommand::UpdateBuffer {
            buffer: uniform_buffer,
            offset: 0,
            data: RhiRenderer::encode_f32s(&[
                viewport.width,
                viewport.height,
                0.0,
                0.0,
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                rect.rgba[0],
                rect.rgba[1],
                rect.rgba[2],
                rect.rgba[3],
                rect.radius[0],
                rect.radius[1],
                rect.radius[2],
                rect.radius[3],
                rect.half_stroke,
                0.0,
                0.0,
                0.0,
                // 保留 affine shadow 共用常量块的末尾 float4。
                0.0,
                0.0,
                0.0,
                0.0,
            ]),
        });
        // 追加当前矩形的单位 quad draw packet。
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

    // 执行一帧圆角/描边矩形 RHI 计划。
    pub(crate) fn execute_shape_rects(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        viewport: RhiViewport,
        load: LoadAction,
        target: RenderTargetRef,
        rects: &[RhiShapeRect],
    ) -> Result<(), crate::core::Error> {
        // 空列表不应伪造一次 present。
        if rects.is_empty() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid(
                "RhiRenderer cannot execute an empty shape list",
            ));
        }
        // 拒绝非有限 viewport，避免计划构造和 adapter 结果分叉。
        if !viewport.is_valid() {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid("RhiRenderer shape viewport is invalid"));
        }
        // 在创建资源前验证所有矩形的固定 shader 常量。
        for rect in rects {
            // 矩形几何必须是有限正值。
            if !rect.x.is_finite()
                || !rect.y.is_finite()
                || !rect.w.is_finite()
                || !rect.h.is_finite()
                || rect.w <= 0.0
                || rect.h <= 0.0
            {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid("RhiRenderer shape geometry is invalid"));
            }
            // 颜色、圆角和描边常量必须有限。
            if rect
                .rgba
                .iter()
                .chain(rect.radius.iter())
                .any(|value| !value.is_finite() || *value < 0.0)
                || !rect.half_stroke.is_finite()
                || rect.half_stroke < 0.0
            {
                // 不把负半径或负描边交给 shader。
                return Err(super::rhi_invalid(
                    "RhiRenderer shape constants are invalid",
                ));
            }
            // 显式 scissor 必须已经完成物理坐标 lowering。
            if rect.scissor.is_some_and(|scissor| !scissor.is_valid()) {
                // 返回稳定的参数错误。
                return Err(super::rhi_invalid("RhiRenderer shape scissor is invalid"));
            }
        }
        // 准备 shape pipeline、单位 quad 和常量 buffer。
        let (pipeline, _, vertex_buffer, uniform_buffer) = self.ensure_shape_resources(context)?;
        // 计划使用当前 context 的 surface 代际。
        let surface = context.token();
        // 创建 surface pass，并保留调用方的 load/clear 语义。
        let mut pass = RenderPassPlan::new(target, load);
        // 所有 shape 共享同一个物理 viewport。
        pass.push(FramePlanCommand::SetViewport(viewport));
        // 每个矩形只更新常量并保留 painter order。
        for rect in rects {
            // 在 draw 前设置当前矩形的裁剪。
            pass.push(FramePlanCommand::SetScissor(rect.scissor));
            // 上传完整 RectConstants，保持 D3D11 16-byte 对齐布局。
            pass.push(FramePlanCommand::UpdateBuffer {
                buffer: uniform_buffer,
                offset: 0,
                data: RhiRenderer::encode_f32s(&[
                    viewport.width,
                    viewport.height,
                    0.0,
                    0.0,
                    rect.x,
                    rect.y,
                    rect.w,
                    rect.h,
                    rect.rgba[0],
                    rect.rgba[1],
                    rect.rgba[2],
                    rect.rgba[3],
                    rect.radius[0],
                    rect.radius[1],
                    rect.radius[2],
                    rect.radius[3],
                    rect.half_stroke,
                    0.0,
                    0.0,
                    0.0,
                    // 保留 affine shadow 共用常量块的末尾 float4。
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                ]),
            });
            // 追加单位 quad 的非索引 shape draw packet。
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
        // 保留 shape painter order。
        plan.push_pass(pass);
        // surface 计划最终 present，texture 计划只执行离屏 submit。
        super::execute_plan_for_target(context, &plan, target)?;
        // 资源由 renderer 跨帧复用，不能在这里销毁。
        Ok(())
    }
}
