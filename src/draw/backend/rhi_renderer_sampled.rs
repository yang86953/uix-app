//! 已有纹理 sampled quad 的无 present 合成片段。

// 引入共享错误和 damage 类型。
use crate::core::error::Result;
// 引入当前片段的 damage 语义。
use crate::core::PresentDamage;
// 引入薄 RHI 的采样与执行类型。
use crate::native::present::rhi::{DrawPacket, GraphicsContextRhi, LoadAction, RhiViewport};

// 引入父 renderer 的计划、target、资源载荷和执行器。
use super::{
    FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef, RhiOp, RhiRenderer,
    RhiSampledQuad,
};

// 为 RhiRenderer 提供不触发 present 的已有纹理合成入口。
impl RhiRenderer {
    // 执行一组引用同一 retained texture 的 sampled quad 并触发唯一最终 present。
    pub(crate) fn execute_sampled_quads(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        viewport: RhiViewport,
        load: LoadAction,
        target: RenderTargetRef,
        quads: &[RhiSampledQuad],
    ) -> Result<()> {
        // 空操作不能伪装成已经更新并提交 surface。
        if quads.is_empty() {
            // 使用稳定参数错误阻止空 present。
            return Err(super::rhi_invalid(
                "RhiRenderer sampled surface composite requires at least one quad",
            ));
        }
        // 将逐 damage rect quad 降为同一 painter-order sampled 操作队列。
        let operations = quads
            .iter()
            // sampled quad 是纯值 ABI，可以安全复制到本次计划。
            .copied()
            // 每个 quad 保留自己的 scissor。
            .map(RhiOp::Sampled)
            // 物化为 execute_ops 同步消费的稳定切片。
            .collect::<Vec<_>>();
        // 复用统一混合执行器，保持一次 acquire、submit 与最终 present。
        self.execute_ops(context, damage, viewport, load, target, &operations, true)
    }

    // 执行一个已经存在的 sampled texture quad，但不触发 surface present。
    pub(crate) fn execute_sampled_quad_without_present(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        viewport: RhiViewport,
        load: LoadAction,
        target: RenderTargetRef,
        quad: RhiSampledQuad,
    ) -> Result<()> {
        // sampled quad 必须具有有效的物理 viewport 和目标 geometry。
        if !viewport.is_valid()
            || !quad.x.is_finite()
            || !quad.y.is_finite()
            || !quad.w.is_finite()
            || !quad.h.is_finite()
            || quad.w <= 0.0
            || quad.h <= 0.0
            || quad.texture.raw() == 0
        {
            // 返回稳定的参数错误。
            return Err(super::rhi_invalid(
                "RhiRenderer sampled segment ABI is invalid",
            ));
        }
        // Additive 需要 adapter 明确提供对应 blend capability。
        if quad.additive && !context.capabilities().additive_blend {
            // 不在不支持的 adapter 上伪造 blending 结果。
            return Err(super::rhi_invalid(
                "RhiRenderer sampled segment lacks additive blend",
            ));
        }
        // 准备可复用的 sampled pipeline、vertex、uniform 和 sampler。
        let (pipeline, vertex_buffer, uniform_buffer, sampler) =
            self.ensure_textured_resources(context)?;
        // 为 Additive source 选择独立 blend pipeline。
        let pipeline = if quad.additive {
            // 保持 Additive 与 SrcOver 的固定 pipeline 分离。
            self.ensure_additive_textured_pipeline(context)?
        } else {
            // 默认 Picture source 使用 premultiplied SrcOver。
            pipeline
        };
        // 计划使用当前 context 的 surface 代际。
        let surface = context.token();
        // 创建不触发 present 的 surface 或离屏 pass。
        let mut pass = RenderPassPlan::new(target, load);
        // 设置当前目标的物理 viewport。
        pass.push(FramePlanCommand::SetViewport(viewport));
        // 选择当前 sampled quad 的裁剪。
        pass.push(FramePlanCommand::SetScissor(quad.scissor));
        // 上传已经存在纹理的 sampled quad 顶点。
        pass.push(FramePlanCommand::UpdateBuffer {
            buffer: vertex_buffer,
            offset: 0,
            data: Self::encode_f32s(&super::mixed::sampled_vertices(&quad)),
        });
        // 上传当前目标的 viewport uniform。
        pass.push(FramePlanCommand::UpdateBuffer {
            buffer: uniform_buffer,
            offset: 0,
            data: Self::encode_f32s(&[viewport.width, viewport.height, 0.0, 0.0]),
        });
        // 绑定已经存在的 Picture texture 和共享 sampler。
        pass.push(FramePlanCommand::BindTexture {
            slot: 0,
            texture: quad.texture,
            sampler,
        });
        // 追加六顶点的非索引 sampled draw packet。
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
        // 创建计划并保留当前 target 的有序执行语义。
        let mut plan = FramePlan::new(surface, damage);
        // 追加唯一 sampled pass。
        plan.push_pass(pass);
        // 只 submit 当前 segment，最终 present 由外层帧边界负责。
        super::execute_plan_without_present(context, &plan, target)
    }
}
