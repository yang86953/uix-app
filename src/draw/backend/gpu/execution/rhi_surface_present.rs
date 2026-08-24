//! 主 surface 的 solid/mixed RHI 最终 present lowering。

// 引入最终提交 damage、错误和主 surface 访问类型。
use crate::core::{DamageRegion, Errc, Error};
// 引入统一 FramePlan 的目标类型。
use crate::draw::backend::frame_plan::RenderTargetRef;
// 引入薄 RHI 的 pass/load 和纹理目标类型。
use crate::platform::presentation::rhi::{LoadAction, RhiColor, TextureHandle};

// 引入当前 GPU backend owner。
use super::GpuBackend;

// 为 GpuBackend 提供 solid/mixed retained surface 的最终提交边界。
impl GpuBackend {
    // 尝试用单个 FramePlan 保持多种 native 操作的 painter order 并完成 present。
    pub(super) fn try_present_rhi_mixed(&mut self, damage: &DamageRegion) -> Result<bool, Error> {
        // 混合 RHI 纵切只覆盖主 surface，离屏和局部 clear 继续走兼容路径。
        if self.active_offscreen.is_some()
            || self.rhi_renderer.is_none()
            || !self.surface.pending_clear_rects.is_empty()
            || !self.surface.pending_scroll_copies.is_empty()
            || self.surface.canvas.soft_has_content
            || self.surface.canvas.pending_native.is_empty()
        {
            // 不改变现有状态，交回兼容 presenter。
            return Ok(false);
        }
        // 先准备跨帧 retained target，禁止把 swapchain backbuffer 当作持久画布。
        let Some(target) = self.try_rhi_surface_target()? else {
            // 没有组合 RHI 时继续走既有兼容路径。
            return Ok(false);
        };
        // 迁移期主 surface target 必须是新建的 retained texture。
        let RenderTargetRef::Texture(retained_texture) = target else {
            // 保留稳定状态错误，防止未来 target 解析改变后静默写错对象。
            return Err(Error::new(
                Errc::InvalidState,
                "RHI retained surface target did not resolve to a texture",
            ));
        };
        // 只有需要全清时才能把旧 clear 语义映射为 pass load action。
        let load = if self.surface.needs_gpu_clear {
            // 首帧主 surface 的透明初始化。
            LoadAction::Clear(RhiColor::transparent())
        } else {
            // 保留 retained texture 的已有像素。
            LoadAction::Load
        };
        // 先生成与兼容 present 相同的 damage 计划。
        let caps = self.gpu_ctx.caps();
        // 读取当前 surface 身份。
        let present_surface = self.gpu_ctx.present_surface();
        // 从同一 GPU recipe owner 读取当前可写 swapchain image 身份。
        let present_image = self.gpu_ctx.present_image();
        // 计算本次提交携带的 damage 语义。
        let prepared_damage = self.present_damage_tracker.prepare(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        let (damage_plan, damage_commit) = prepared_damage.into_parts();
        // 将 owner-thread 借用限制在 retained 绘制调用内。
        let submitted = {
            // 分开借用 owner-thread context、renderer cache 和主 surface。
            let (gpu_ctx, rhi_renderer, surface) =
                (&mut self.gpu_ctx, &mut self.rhi_renderer, &mut self.surface);
            // 前置条件已经检查了 renderer cache。
            let Some(renderer) = rhi_renderer.as_mut() else {
                // 没有 RHI cache 时回到兼容路径。
                return Ok(false);
            };
            // 只有 native context 暴露组合 RHI 才能执行 FramePlan。
            // 已验证 owner 丢失时返回 typed failure，不能回退 legacy 路径。
            let context = gpu_ctx.rhi_context()?;
            // 冻结 retained texture 与主 drawable 共用的物理范围。
            let extent = context.surface_ref().token().extent;
            // 将混合 pending queue lowering 为保序 FramePlan。
            surface.canvas.submit_rhi_mixed(
                renderer,
                // 离屏 lowering 只取得 Device 角色。
                context.device(),
                // 使用冻结的 retained texture 物理范围。
                extent,
                load,
                // 只允许写入显式 retained texture。
                retained_texture,
            )?
        };
        // 不支持的队列没有触碰 present 状态，继续兼容路径。
        if !submitted {
            return Ok(false);
        }
        // 先把 retained texture 合成到 swapchain，再进入统一成功提交边界。
        self.present_rhi_surface_texture(
            TextureHandle::from_raw(retained_texture.raw()),
            damage_plan.present_damage,
        )?;
        // 只有最终合成成功后才消费主 surface 的全清状态。
        self.surface.needs_gpu_clear = false;
        // 清空已经由 pass load action 替代的全清标记。
        self.surface.pending_clear_rects.clear();
        // 只有 present 成功才提交 damage tracker 状态。
        self.present_damage_tracker.commit_prepared(damage_commit);
        // 保持与兼容路径相同的软资源 aging 语义。
        let used_soft = self.surface.canvas.finish_presented_frame();
        // 记录本帧是否使用了软回退内容。
        self.soft_used_in_last_present = used_soft;
        // 没有软资源时取消空闲回收截止时间。
        if !self.has_soft_fallback_allocation() {
            // 让下一帧从干净的软资源状态开始。
            self.soft_fallback_idle_deadline = None;
        }
        // 告知调用方本次 present 已经由 RHI 完成。
        Ok(true)
    }

    // 尝试用 FramePlan 完成当前主 surface 的纯 solid 最终 present。
    pub(super) fn try_present_rhi_solid(&mut self, damage: &DamageRegion) -> Result<bool, Error> {
        // RHI 纵切只覆盖主 surface，离屏和复杂 ordered boundary 仍走兼容路径。
        if self.active_offscreen.is_some()
            || self.rhi_renderer.is_none()
            || !self.surface.pending_clear_rects.is_empty()
            || !self.surface.pending_scroll_copies.is_empty()
            || self.surface.canvas.soft_has_content
            || self.surface.canvas.pending_native.is_empty()
        {
            // 不改变现有状态，交回兼容 presenter。
            return Ok(false);
        }
        // 先准备跨帧 retained target，避免 solid pass 直接写入易失 swapchain。
        let Some(target) = self.try_rhi_surface_target()? else {
            // 没有组合 RHI 时继续走既有兼容路径。
            return Ok(false);
        };
        // 迁移期主 surface target 必须是 retained texture。
        let RenderTargetRef::Texture(retained_texture) = target else {
            // 防止 target 解析未来变化后静默破坏生命周期边界。
            return Err(Error::new(
                Errc::InvalidState,
                "RHI retained solid target did not resolve to a texture",
            ));
        };
        // 只有需要全清时才能把旧 clear 语义等价映射为 pass load action。
        let load = if self.surface.needs_gpu_clear {
            // 首帧主 surface 的透明初始化。
            LoadAction::Clear(RhiColor::transparent())
        } else {
            // 保留 retained texture 的已有像素。
            LoadAction::Load
        };
        // 在 execute 之前生成与兼容 present 相同的 damage 计划。
        let caps = self.gpu_ctx.caps();
        // 读取当前 surface 身份。
        let present_surface = self.gpu_ctx.present_surface();
        // 从同一 GPU recipe owner 读取当前可写 swapchain image 身份。
        let present_image = self.gpu_ctx.present_image();
        // 计算本次提交携带的 damage 语义。
        let prepared_damage = self.present_damage_tracker.prepare(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        let (damage_plan, damage_commit) = prepared_damage.into_parts();
        // 将 owner-thread 借用限制在 retained solid 绘制调用内。
        let submitted = {
            // 分别借用 context、renderer cache 和 canvas，保持 owner-thread 组合借用。
            let (gpu_ctx, rhi_renderer, surface) =
                (&mut self.gpu_ctx, &mut self.rhi_renderer, &mut self.surface);
            // 前置条件已经检查了 renderer cache。
            let Some(renderer) = rhi_renderer.as_mut() else {
                // 没有 RHI cache 时回到兼容路径。
                return Ok(false);
            };
            // 只有 native context 暴露组合 RHI 才能执行 FramePlan。
            // 已验证 owner 丢失时返回 typed failure，不能回退 legacy 路径。
            let context = gpu_ctx.rhi_context()?;
            // 冻结 retained texture 与主 drawable 共用的物理范围。
            let extent = context.surface_ref().token().extent;
            // 将纯 solid 队列 lowering 为 FramePlan；不支持的操作返回 false。
            surface.canvas.submit_rhi_solid(
                renderer,
                // 离屏 lowering 只取得 Device 角色。
                context.device(),
                // 使用冻结的 retained texture 物理范围。
                extent,
                load,
                // 只允许写入显式 retained texture。
                retained_texture,
            )?
        };
        // 不支持的队列没有触碰 present 状态，继续兼容路径。
        if !submitted {
            return Ok(false);
        }
        // retained solid 内容必须先合成到 swapchain 才能报告最终 present 成功。
        self.present_rhi_surface_texture(
            TextureHandle::from_raw(retained_texture.raw()),
            damage_plan.present_damage,
        )?;
        // 只有最终合成成功后才消费主 surface 的全清状态。
        self.surface.needs_gpu_clear = false;
        // 清空已经由 retained pass load action 替代的 clear 标记。
        self.surface.pending_clear_rects.clear();
        // 只有 present 成功才提交 damage tracker 状态。
        self.present_damage_tracker.commit_prepared(damage_commit);
        // 保持与兼容路径相同的软资源 aging 语义。
        let used_soft = self.surface.canvas.finish_presented_frame();
        // 记录本帧是否使用了软回退内容。
        self.soft_used_in_last_present = used_soft;
        // 没有软资源时取消空闲回收截止时间。
        if !self.has_soft_fallback_allocation() {
            // 让下一帧从干净的软资源状态开始。
            self.soft_fallback_idle_deadline = None;
        }
        // 告知调用方本次 present 已经由 RHI 完成。
        Ok(true)
    }
}
