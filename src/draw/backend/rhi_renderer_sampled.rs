//! 已有纹理 sampled quad 的无 present 合成片段。

// 引入共享错误和 damage 类型。
use crate::core::error::Result;
// 引入当前片段的 damage 语义。
use crate::core::PresentDamage;
// 引入薄 RHI 的采样与执行类型。
use crate::native::present::rhi::{
    DrawPacket, DrawRange, GraphicsContextRhi, GraphicsDevice, GraphicsSurface, LoadAction,
    RhiViewport, SampledTextureBinding, TextureHandle,
};

// 引入父 renderer 的计划、target、资源载荷和执行器。
use super::{
    FramePlanCommand, FrameUniformPayload, FrameVertexPayload, RenderPassPlan, RenderTargetRef,
    RhiOp, RhiRenderer, RhiSampledQuad,
};

// 为 RhiRenderer 提供不触发 present 的已有纹理合成入口。
impl RhiRenderer {
    // 执行一组引用同一 retained texture 的 sampled quad 并触发唯一最终 present。
    #[cfg_attr(feature = "test-harness", allow(dead_code))]
    pub(crate) fn execute_sampled_quads(
        &mut self,
        context: &mut dyn GraphicsContextRhi,
        damage: PresentDamage,
        viewport: RhiViewport,
        load: LoadAction,
        quads: &[RhiSampledQuad],
    ) -> Result<()> {
        // 普通最终合成不插入额外 surface 观察者。
        self.execute_sampled_quads_with_present_hook(
            // 保留当前组合 context。
            context,
            // 保留最终 damage。
            damage,
            // 保留物理 viewport。
            viewport,
            // 保留 pass load 动作。
            load,
            // 保留全部 damage quad。
            quads,
            // 普通路径使用空观察钩子。
            &mut |_| {},
        )
    }

    // 执行 sampled surface 合成并在唯一 submit 后、present 前调用观察者。
    pub(crate) fn execute_sampled_quads_with_present_hook(
        // 借用 renderer 资源缓存。
        &mut self,
        // 借用当前组合 context owner。
        context: &mut dyn GraphicsContextRhi,
        // 接收最终 damage。
        damage: PresentDamage,
        // 接收物理 viewport。
        viewport: RhiViewport,
        // 接收 pass load 动作。
        load: LoadAction,
        // 接收全部有序 sampled quad。
        quads: &[RhiSampledQuad],
        // 接收只允许观察已提交 Surface 的 owner-thread 钩子。
        before_present: &mut dyn FnMut(&mut dyn GraphicsSurface),
        // 返回 lowering、提交或 present 的真实结果。
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
        // Surface 变体原子绑定 context、damage 与 submit-before-present 钩子。
        let frame = super::RhiRendererFrame::surface_with_present_hook(
            // 借用当前唯一组合 context。
            context,
            // 保存最终 present damage。
            damage,
            // 钩子只能存在于 Surface 帧。
            before_present,
        );
        // 复用统一混合执行器，保持一次 acquire、submit 与最终 present。
        self.execute_ops(
            // 传入无法表示 Offscreen 钩子的封闭帧。
            frame,
            // 保留物理 viewport。
            viewport,
            // 保留 pass load 动作。
            load,
            // 传入已经排序的 sampled 操作。
            &operations,
        )
    }

    // 执行一个已经存在的 sampled texture quad，但不触发 surface present。
    pub(crate) fn execute_sampled_quad_without_present(
        &mut self,
        device: &mut dyn GraphicsDevice,
        viewport: RhiViewport,
        load: LoadAction,
        target: TextureHandle,
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
        if quad.additive && !device.device_capabilities().additive_blend {
            // 不在不支持的 adapter 上伪造 blending 结果。
            return Err(super::rhi_invalid(
                "RhiRenderer sampled segment lacks additive blend",
            ));
        }
        // 准备可复用的 sampled pipeline、vertex、uniform 和 sampler。
        let (pipeline, vertex_buffer, uniform_buffer, sampler) =
            self.ensure_textured_resources(device)?;
        // 为 Additive source 选择独立 blend pipeline。
        let pipeline = if quad.additive {
            // 保持 Additive 与 SrcOver 的固定 pipeline 分离。
            self.ensure_additive_textured_pipeline(device)?
        } else {
            // 默认 Picture source 使用 premultiplied SrcOver。
            pipeline
        };
        // 创建不触发 present 的显式纹理 pass。
        let mut pass = RenderPassPlan::new(RenderTargetRef::Texture(target), load);
        // 设置当前目标的物理 viewport。
        pass.push(FramePlanCommand::SetViewport(viewport));
        // 选择当前 sampled quad 的裁剪。
        pass.push(FramePlanCommand::SetScissor(quad.scissor));
        // 上传已经存在纹理的类型化 sampled quad 顶点。
        pass.push(FramePlanCommand::UploadVertex {
            buffer: vertex_buffer,
            data: FrameVertexPayload::position_uv_color_f32(super::mixed::sampled_vertices(&quad)),
        });
        // 上传当前目标的类型化 viewport uniform。
        pass.push(FramePlanCommand::UploadUniform {
            buffer: uniform_buffer,
            data: FrameUniformPayload::Sampled(Self::sampled_uniform(viewport)),
        });
        // 原子绑定已经存在的 Picture texture 和共享 sampler。
        pass.push(FramePlanCommand::BindSampledTexture(
            // 离屏路径与 surface 路径共享同一个固定槽位契约。
            SampledTextureBinding::for_pipeline(
                // 传递当前 sampled 纹理身份。
                quad.texture,
                // 传递共享 sampled 采样器身份。
                sampler,
                // sampled 绑定与后续 DrawPacket 使用同一个 pipeline。
                pipeline,
            ),
        ));
        // 追加六顶点的非索引 sampled draw packet。
        pass.push(FramePlanCommand::Draw(DrawPacket {
            pipeline,
            vertex_buffer,
            uniform_buffer: Some(uniform_buffer),
            // Sampled quad 使用封闭的六顶点非索引范围。
            range: DrawRange::vertices(6),
        }));
        // 创建只拥有 Device 与纹理目标的封闭帧。
        let mut frame = super::RhiRendererFrame::offscreen(device, target);
        // 创建不含任何 Surface 生命周期的计划。
        let mut plan = frame.plan();
        // 追加唯一 sampled pass。
        plan.push_pass(pass);
        // 只 submit 当前 segment，最终 present 由外层帧边界负责。
        frame.execute(&plan)
    }
}
