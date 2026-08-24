//! GpuBackend 的迁移期 FramePlan 提交边界。

// 引入当前 backend 的错误、逻辑 damage 和类型。
use crate::core::{DamageRegion, Errc, Error};
// 引入 FramePlan 的 surface/texture target 引用。
use crate::draw::backend::frame_plan::RenderTargetRef;
// 引入薄 RHI 的 load/color 类型。
use crate::platform::presentation::rhi::{LoadAction, RhiColor, TextureHandle};

// 引入当前 GpuBackend 类型。
use super::GpuBackend;
// 引入 native queue 的 scroll 变体，保证 soft 后的目标搬移不被重排。
use super::super::pending::PendingNativeOp;

// 为 GpuBackend 提供 FrameEncoder retained target 的最终合成提交。
impl GpuBackend {
    // 尝试把有序 boundary 前的主 surface native queue 写入 retained texture。
    pub(super) fn try_flush_main_segment_rhi(&mut self) -> Result<bool, Error> {
        // 完全空的队列不能形成可观察的 retained boundary。
        if self.active_offscreen.is_some()
            || self.rhi_renderer.is_none()
            || (self.surface.canvas.pending_native.is_empty()
                && self.surface.pending_clear_rects.is_empty()
                && self.surface.pending_scroll_copies.is_empty()
                && !self.surface.canvas.soft_has_content
                && !self.surface.needs_gpu_clear)
        {
            // 让调用方继续检查其它 RHI 纵切或返回 typed failure。
            return Ok(false);
        }
        // soft 内容已经按 blend 分段；只有 soft 后 scroll 仍缺少可重排边界。
        if self.surface.canvas.soft_has_content
            && self
                .surface
                .canvas
                .pending_native
                .iter()
                .any(|operation| matches!(operation, PendingNativeOp::ScrollCopy(_)))
        {
            // 保持队列未消费，交由主 surface typed 门禁拒绝不可重排语义。
            return Ok(false);
        }
        // 先确保当前 surface generation 对应的 retained texture。
        let Some(target) = self.try_rhi_surface_target()? else {
            // 缺少组合 RHI 时保持队列未消费并交回 typed 边界。
            return Ok(false);
        };
        // 有序 main segment 不能落到易失 swapchain sentinel。
        let RenderTargetRef::Texture(retained_texture) = target else {
            // 返回稳定状态错误，避免静默破坏 painter order。
            return Err(Error::new(
                Errc::InvalidState,
                "RHI main segment target did not resolve to a retained texture",
            ));
        };
        // 记录本次 boundary 是否包含 scroll/局部清理，即使它们最终为空操作。
        let had_surface_scroll = !self.surface.pending_scroll_copies.is_empty();
        // 记录本次 boundary 是否包含局部清理记录。
        let had_surface_clear = !self.surface.pending_clear_rects.is_empty();
        // 记录新 retained target 是否需要在没有绘制命令时单独透明初始化。
        let had_full_clear = self.surface.needs_gpu_clear;
        // 先按记录顺序搬移 retained 旧像素，再处理局部清理和 native draw。
        if !self.surface.pending_scroll_copies.is_empty() {
            // 使用同一代际的采样纹理句柄建立 TextureMove。
            let retained_texture_handle = self.ensure_rhi_surface_texture()?;
            // 不可表达的 scroll 必须整段回退，不能静默丢失旧像素。
            if !self.try_apply_rhi_surface_scroll_copies(retained_texture_handle)? {
                // 保留原始记录，交给调用方执行安全 typed 边界。
                return Ok(false);
            }
        }
        // 先把 begin_frame 记录的局部清理落入 retained texture。
        if !self.surface.pending_clear_rects.is_empty() {
            // ClearRect 失败时保持 retained 状态，禁止切换 direct swapchain 路径。
            if !self.try_clear_rhi_surface_rects(retained_texture)? {
                // 不消费尚未成功写入的局部清理记录。
                return Ok(false);
            }
            // 只有 RHI 清理成功后才消费这些记录。
            self.surface.pending_clear_rects.clear();
            // retained target 已经完成本代际的首次有效写入。
            self.surface.needs_gpu_clear = false;
        }
        // 根据 retained target 当前代际选择首帧清理或保留旧像素。
        let load = if self.surface.needs_gpu_clear {
            // 新代际必须先透明初始化再写 native queue。
            LoadAction::Clear(RhiColor::transparent())
        } else {
            // 同一代际继续保留之前已经交付的内容。
            LoadAction::Load
        };
        // 只有真正的空新帧需要透明 dummy；已有 scroll/局部 clear 会自行形成提交。
        let clear_only_submitted = had_full_clear
            && !had_surface_scroll
            && !had_surface_clear
            && self.surface.canvas.pending_native.is_empty()
            && !self.surface.canvas.soft_has_content;
        // 空新帧也必须在 retained texture 内初始化，不能回到 direct swapchain clear。
        if clear_only_submitted {
            // 同时借用 owner-thread context、renderer cache 与 surface canvas。
            let (gpu_ctx, rhi_renderer, surface) =
                (&mut self.gpu_ctx, &mut self.rhi_renderer, &mut self.surface);
            // 前置能力检查已经保证 renderer cache 存在。
            let Some(renderer) = rhi_renderer.as_mut() else {
                // 状态若在借用前发生破坏，保持原子 lowering 失败。
                return Ok(false);
            };
            // retained target 只能由当前组合 RHI context 初始化。
            // 缺失已验证 owner 时返回 typed failure，不触碰 adapter 高层入口。
            let context = gpu_ctx.rhi_context()?;
            // 冻结 retained texture 与主 drawable 共用的物理范围。
            let extent = context.surface_ref().token().extent;
            // 透明 clear 只提交到 retained texture，不获取或呈现 swapchain。
            surface.canvas.submit_rhi_clear_only(
                // 复用当前通用 renderer cache。
                renderer,
                // 离屏初始化只取得 Device 角色。
                context.device(),
                // 使用冻结的 retained texture 物理范围。
                extent,
                // 使用前面计算出的透明 Clear load。
                load,
                // 写入唯一 retained target。
                retained_texture,
            )?;
        }
        // 在短借用范围内完成 ordered native lowering；soft 段稍后独立合成。
        let native_submitted = if self.surface.canvas.pending_native.is_empty() {
            // 没有 native 前缀时保留 false，让 soft 段使用初始 load action。
            false
        } else {
            // 同时借用 owner-thread context、renderer cache 和 surface canvas。
            let (gpu_ctx, rhi_renderer, surface) =
                (&mut self.gpu_ctx, &mut self.rhi_renderer, &mut self.surface);
            // 前置条件已经检查了 renderer cache。
            let Some(renderer) = rhi_renderer.as_mut() else {
                // 缺少 renderer 时保持原队列并交回 typed boundary。
                return Ok(false);
            };
            // 只有组合 RHI context 能执行 retained texture pass。
            // 当前 adapter 的组合 RHI 已在构造期验证。
            let context = gpu_ctx.rhi_context()?;
            // 冻结 retained texture 与主 drawable 共用的物理范围。
            let extent = context.surface_ref().token().extent;
            // 保留 native queue 内部的 painter order，并且不触发 swapchain present。
            surface.canvas.submit_rhi_native_prefix(
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
        // 未覆盖的 queue 不应被伪装成 retained 提交成功。
        if !native_submitted && !self.surface.canvas.pending_native.is_empty() {
            // 调用方会保留清理标记并返回稳定 typed failure。
            return Ok(false);
        }
        // native 前缀成功后，后续 soft tile 必须以 Load 继续写入同一目标。
        if native_submitted {
            // native pass 的 load action 已经初始化或保留了 retained target。
            self.surface.needs_gpu_clear = false;
        }
        // 把透明 CPU soft segment 作为同一 retained target 的 SrcOver 采样段合成。
        if self.surface.canvas.soft_has_content {
            // 非 1:1 DPR、空目标和 adapter 不支持时保持原子失败边界。
            let soft_load = if self.surface.needs_gpu_clear {
                LoadAction::Clear(RhiColor::transparent())
            } else {
                LoadAction::Load
            };
            if !self.try_upload_rhi_surface_soft(retained_texture, soft_load)? {
                // 尚未消费 native/soft staging，调用方可安全销毁 retained target。
                return Ok(false);
            }
            // soft 采样成功后该代际已经拥有确定像素。
            self.surface.needs_gpu_clear = false;
        }
        // 局部清理、scroll、native 和 soft 任一成功都形成 pending present。
        if !native_submitted
            && !self.surface.canvas.soft_has_content
            && self.surface.pending_clear_rects.is_empty()
            && self.surface.pending_scroll_copies.is_empty()
            && !had_surface_scroll
            && !had_surface_clear
            && !clear_only_submitted
        {
            // 理论上只可能由未来扩展触发，保留空边界的 typed failure。
            return Ok(false);
        }
        // RHI pass 成功后取消全清，并等待后续 ordered operation 或最终 present。
        self.surface.needs_gpu_clear = false;
        // 当前 pass 的 load action 已覆盖初始化语义。
        self.surface.pending_clear_rects.clear();
        // 标记主 retained target 有待最终合成的内容。
        self.rhi_surface_frame_pending_present = true;
        // 提交 boundary 后消费已落入 RHI 的 native queue。
        self.surface.canvas.commit_presented_frame();
        // 告知调用方当前 boundary 已安全写入 retained texture。
        Ok(true)
    }

    // 将已经写入 retained texture 的主帧合成到 swapchain 并完成统一 present。
    pub(super) fn try_present_pending_rhi_frame(
        &mut self,
        damage: &DamageRegion,
    ) -> Result<bool, Error> {
        // 没有 FrameEncoder pending 标记时不改变既有 present 尝试顺序。
        if !self.rhi_surface_frame_pending_present {
            // 让调用方继续检查其它 RHI 路径。
            return Ok(false);
        }
        // pending 标记与 retained texture 必须成对存在。
        let texture = self.rhi_surface_texture.ok_or_else(|| {
            // 资源生命周期破坏时返回明确状态错误，不把旧 swapchain 当作目标。
            Error::new(
                Errc::InvalidState,
                "FrameEncoder pending present lost its retained surface texture",
            )
        })?;
        // 沿用统一 damage tracker，确保 surface generation 和 present image 一致。
        let caps = self.gpu_ctx.caps();
        // 从同一 owner 读取当前 surface 元数据与可写 swapchain image 身份。
        let (present_surface, present_image) =
            (self.gpu_ctx.present_surface(), self.gpu_ctx.present_image());
        // 计算最终 sampled composite 的 damage 语义。
        let prepared_damage = self.present_damage_tracker.prepare(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        let (damage_plan, damage_commit) = prepared_damage.into_parts();
        // 先完成唯一 swapchain composite，成功后才提交 tracker 和 canvas 状态。
        self.present_rhi_surface_texture(texture, damage_plan.present_damage)?;
        // 最终合成成功后消费 retained surface 的初始化标记。
        self.surface.needs_gpu_clear = false;
        // 清空已经由 retained FramePlan load action 替代的局部清理记录。
        self.surface.pending_clear_rects.clear();
        // 只有实际 present 成功才推进 damage tracker。
        self.present_damage_tracker.commit_prepared(damage_commit);
        // 保持与其他 RHI present 路径相同的 canvas 消费和软资源 aging 语义。
        let mut used_soft = self.surface.canvas.finish_presented_frame();
        // 同一最终 present 后推进 Picture 软回退资源的生命周期。
        for offscreen in self.offscreens.iter_mut().flatten() {
            // 任一 Picture 软资源被使用都应保留 backend 级使用标记。
            used_soft |= offscreen.canvas.age_soft_fallback_after_present();
        }
        // 记录本次提交是否实际使用了软回退资源。
        self.soft_used_in_last_present = used_soft;
        // 没有软资源时取消空闲回收截止时间。
        if !self.has_soft_fallback_allocation() {
            // 让下一帧从干净的软资源状态开始。
            self.soft_fallback_idle_deadline = None;
        }
        // 清除已完成最终合成的 FrameEncoder pending 标记。
        self.rhi_surface_frame_pending_present = false;
        // 告知 RenderBackend 本次 present 已经由 retained RHI 完成。
        Ok(true)
    }
}

// 为剩余 native RHI 专用队列提供最终 present 尝试。
impl GpuBackend {
    // 尝试用 FramePlan 完成当前主 surface 的圆角/描边矩形最终 present。
    pub(super) fn try_present_rhi_shapes(&mut self, damage: &DamageRegion) -> Result<bool, Error> {
        // shape RHI 纵切只覆盖主 surface，离屏和局部 clear 保留兼容路径。
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
        // shape pass 先绑定跨帧 retained target，避免依赖 swapchain 保留性。
        let Some(target) = self.try_rhi_surface_target()? else {
            // 没有组合 RHI 时继续走既有兼容路径。
            return Ok(false);
        };
        // 迁移期主 surface target 必须是 retained texture。
        let RenderTargetRef::Texture(retained_texture) = target else {
            // 保持 target 生命周期边界显式可检查。
            return Err(Error::new(
                Errc::InvalidState,
                "RHI retained shape target did not resolve to a texture",
            ));
        };
        // 只有需要全清时才能把旧 clear 语义映射为 pass load action。
        let load = if self.surface.needs_gpu_clear {
            // 首帧主 surface 的透明初始化。
            LoadAction::Clear(RhiColor::transparent())
        } else {
            // 保留原生 backbuffer 的已有像素。
            LoadAction::Load
        };
        // 先生成与兼容 present 一致的 damage 计划。
        let caps = self.gpu_ctx.caps();
        // 从同一 owner 读取当前 surface 元数据与可写 swapchain image 身份。
        let (present_surface, present_image) =
            (self.gpu_ctx.present_surface(), self.gpu_ctx.present_image());
        // 计算本次提交要携带的 damage 语义。
        let prepared_damage = self.present_damage_tracker.prepare(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        // 将 owner-thread 借用限制在 retained shape 绘制调用内。
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
            // 已验证 owner 丢失时返回 typed failure，不能恢复旧路径。
            let context = gpu_ctx.rhi_context()?;
            // 冻结 retained texture 与主 drawable 共用的物理范围。
            let extent = context.surface_ref().token().extent;
            // 将纯 shape 队列 lowering 为 FramePlan；不支持的操作返回 false。
            surface.canvas.submit_rhi_shapes(
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
        let (damage_plan, damage_commit) = prepared_damage.into_parts();
        // retained shape 内容必须先合成到 swapchain 才能报告最终 present 成功。
        self.present_rhi_surface_texture(
            TextureHandle::from_raw(retained_texture.raw()),
            damage_plan.present_damage,
        )?;
        // FramePlan present 成功后才推进清理、damage 和 canvas commit。
        // 只有最终合成成功后才消费主 surface 的全清状态。
        self.surface.needs_gpu_clear = false;
        // 清空已经由 pass load action 替代的全清标记。
        self.surface.pending_clear_rects.clear();
        // 只有 present 成功才提交 damage tracker 状态。
        self.present_damage_tracker.commit_prepared(damage_commit);
        // 保持与兼容路径相同的软资源 aging 语义。
        // 提交成功后清理 surface canvas 的已消费队列。
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
    // 结束主 surface 的剩余 RHI 提交实现。
}
// 为剩余 native RHI 队列开启独立的提交实现块。
impl GpuBackend {
    // 尝试用 FramePlan 完成当前主 surface 的轴对齐阴影最终 present。
    pub(super) fn try_present_rhi_shadows(&mut self, damage: &DamageRegion) -> Result<bool, Error> {
        // 阴影 RHI 纵切只覆盖主 surface，离屏和局部 clear 保留兼容路径。
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
        // shadow pass 先绑定跨帧 retained target，避免依赖 swapchain 保留性。
        let Some(target) = self.try_rhi_surface_target()? else {
            // 没有组合 RHI 时继续走既有兼容路径。
            return Ok(false);
        };
        // 迁移期主 surface target 必须是 retained texture。
        let RenderTargetRef::Texture(retained_texture) = target else {
            // 保持 target 生命周期边界显式可检查。
            return Err(Error::new(
                Errc::InvalidState,
                "RHI retained shadow target did not resolve to a texture",
            ));
        };
        // 只有需要全清时才能把旧 clear 语义映射为 pass load action。
        let load = if self.surface.needs_gpu_clear {
            // 首帧主 surface 的透明初始化。
            LoadAction::Clear(RhiColor::transparent())
        } else {
            // 保留原生 backbuffer 的已有像素。
            LoadAction::Load
        };
        // 先生成与兼容 present 一致的 damage 计划。
        let caps = self.gpu_ctx.caps();
        // 从同一 owner 读取当前 surface 元数据与可写 swapchain image 身份。
        let (present_surface, present_image) =
            (self.gpu_ctx.present_surface(), self.gpu_ctx.present_image());
        // 计算本次提交要携带的 damage 语义。
        let prepared_damage = self.present_damage_tracker.prepare(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        // 将 owner-thread 借用限制在 retained shadow 绘制调用内。
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
            // 已验证 owner 丢失时返回 typed failure，不能恢复旧路径。
            let context = gpu_ctx.rhi_context()?;
            // 冻结 retained texture 与主 drawable 共用的物理范围。
            let extent = context.surface_ref().token().extent;
            // 将纯阴影队列 lowering 为 FramePlan；不支持的队列返回 false。
            surface.canvas.submit_rhi_shadows(
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
        let (damage_plan, damage_commit) = prepared_damage.into_parts();
        // retained shadow 内容必须先合成到 swapchain 才能报告最终 present 成功。
        self.present_rhi_surface_texture(
            TextureHandle::from_raw(retained_texture.raw()),
            damage_plan.present_damage,
        )?;
        // FramePlan present 成功后才推进清理、damage 和 canvas commit。
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

    // 尝试用 FramePlan 完成当前主 surface 的 glyph coverage 队列最终 present。
    pub(super) fn try_present_rhi_glyphs(&mut self, damage: &DamageRegion) -> Result<bool, Error> {
        // glyph RHI 纵切只覆盖主 surface，离屏和局部 clear 保留兼容路径。
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
        // glyph pass 先绑定跨帧 retained target，避免依赖 swapchain 保留性。
        let Some(target) = self.try_rhi_surface_target()? else {
            // 没有组合 RHI 时继续走既有兼容路径。
            return Ok(false);
        };
        // 迁移期主 surface target 必须是 retained texture。
        let RenderTargetRef::Texture(retained_texture) = target else {
            // 保持 target 生命周期边界显式可检查。
            return Err(Error::new(
                Errc::InvalidState,
                "RHI retained glyph target did not resolve to a texture",
            ));
        };
        // 只有需要全清时才能把旧 clear 语义映射为 pass load action。
        let load = if self.surface.needs_gpu_clear {
            // 首帧主 surface 的透明初始化。
            LoadAction::Clear(RhiColor::transparent())
        } else {
            // 保留原生 backbuffer 的已有像素。
            LoadAction::Load
        };
        // 先生成与兼容 present 一致的 damage 计划。
        let caps = self.gpu_ctx.caps();
        // 从同一 owner 读取当前 surface 元数据与可写 swapchain image 身份。
        let (present_surface, present_image) =
            (self.gpu_ctx.present_surface(), self.gpu_ctx.present_image());
        // 计算本次提交要携带的 damage 语义。
        let prepared_damage = self.present_damage_tracker.prepare(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        // 将 owner-thread 借用限制在 retained glyph 绘制调用内。
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
            // 已验证 owner 丢失时返回 typed failure，不能恢复旧路径。
            let context = gpu_ctx.rhi_context()?;
            // 冻结 retained texture 与主 drawable 共用的物理范围。
            let extent = context.surface_ref().token().extent;
            // 将纯 glyph 队列 lowering 为 FramePlan；不支持的操作返回 false。
            surface.canvas.submit_rhi_glyphs(
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
        let (damage_plan, damage_commit) = prepared_damage.into_parts();
        // retained glyph 内容必须先合成到 swapchain 才能报告最终 present 成功。
        self.present_rhi_surface_texture(
            TextureHandle::from_raw(retained_texture.raw()),
            damage_plan.present_damage,
        )?;
        // FramePlan present 成功后才推进清理、damage 和 canvas commit。
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

    // 尝试用 FramePlan 完成当前主 surface 的纯图片队列最终 present。
    pub(super) fn try_present_rhi_textured(
        &mut self,
        damage: &DamageRegion,
    ) -> Result<bool, Error> {
        // 图片 RHI 纵切同样只覆盖主 surface，离屏和局部 clear 保留兼容路径。
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
        // textured pass 先绑定跨帧 retained target，避免依赖 swapchain 保留性。
        let Some(target) = self.try_rhi_surface_target()? else {
            // 没有组合 RHI 时继续走既有兼容路径。
            return Ok(false);
        };
        // 迁移期主 surface target 必须是 retained texture。
        let RenderTargetRef::Texture(retained_texture) = target else {
            // 保持 target 生命周期边界显式可检查。
            return Err(Error::new(
                Errc::InvalidState,
                "RHI retained textured target did not resolve to a texture",
            ));
        };
        // 只有需要全清时才能把旧 clear 语义映射为 pass load action。
        let load = if self.surface.needs_gpu_clear {
            // 首帧主 surface 的透明初始化。
            LoadAction::Clear(RhiColor::transparent())
        } else {
            // 保留原生 backbuffer 的已有像素。
            LoadAction::Load
        };
        // 先生成与兼容 present 一致的 damage 计划。
        let caps = self.gpu_ctx.caps();
        // 从同一 owner 读取当前 surface 元数据与可写 swapchain image 身份。
        let (present_surface, present_image) =
            (self.gpu_ctx.present_surface(), self.gpu_ctx.present_image());
        let prepared_damage = self.present_damage_tracker.prepare(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        // 将 owner-thread 借用限制在 retained textured 绘制调用内。
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
            // 已验证 owner 丢失时返回 typed failure，不能恢复旧路径。
            let context = gpu_ctx.rhi_context()?;
            // 冻结 retained texture 与主 drawable 共用的物理范围。
            let extent = context.surface_ref().token().extent;
            // 将纯图片队列 lowering 为 FramePlan；不支持的操作返回 false。
            surface.canvas.submit_rhi_textured(
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
        let (damage_plan, damage_commit) = prepared_damage.into_parts();
        // retained textured 内容必须先合成到 swapchain 才能报告最终 present 成功。
        self.present_rhi_surface_texture(
            TextureHandle::from_raw(retained_texture.raw()),
            damage_plan.present_damage,
        )?;
        // FramePlan present 成功后才推进清理、damage 和 canvas commit。
        // 只有最终合成成功后才消费主 surface 的全清状态。
        self.surface.needs_gpu_clear = false;
        // 清空已经由 pass load action 替代的全清标记。
        self.surface.pending_clear_rects.clear();
        self.present_damage_tracker.commit_prepared(damage_commit);
        // 保持与兼容路径相同的软资源 aging 语义。
        let used_soft = self.surface.canvas.finish_presented_frame();
        self.soft_used_in_last_present = used_soft;
        if !self.has_soft_fallback_allocation() {
            self.soft_fallback_idle_deadline = None;
        }
        // 告知调用方本次 present 已经由 RHI 完成。
        Ok(true)
    }

    // 尝试用 FramePlan 完成当前主 surface 的纯渐变队列最终 present。
    pub(super) fn try_present_rhi_gradients(
        &mut self,
        damage: &DamageRegion,
    ) -> Result<bool, Error> {
        // 渐变 RHI 纵切只覆盖主 surface，离屏和局部 clear 保留兼容路径。
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
        // gradient pass 先绑定跨帧 retained target，避免依赖 swapchain 保留性。
        let Some(target) = self.try_rhi_surface_target()? else {
            // 没有组合 RHI 时继续走既有兼容路径。
            return Ok(false);
        };
        // 迁移期主 surface target 必须是 retained texture。
        let RenderTargetRef::Texture(retained_texture) = target else {
            // 保持 target 生命周期边界显式可检查。
            return Err(Error::new(
                Errc::InvalidState,
                "RHI retained gradient target did not resolve to a texture",
            ));
        };
        // 只有需要全清时才能把旧 clear 语义映射为 pass load action。
        let load = if self.surface.needs_gpu_clear {
            // 首帧主 surface 的透明初始化。
            LoadAction::Clear(RhiColor::transparent())
        } else {
            // 保留原生 backbuffer 的已有像素。
            LoadAction::Load
        };
        // 先生成与兼容 present 一致的 damage 计划。
        let caps = self.gpu_ctx.caps();
        // 从同一 owner 读取当前 surface 元数据与可写 swapchain image 身份。
        let (present_surface, present_image) =
            (self.gpu_ctx.present_surface(), self.gpu_ctx.present_image());
        let prepared_damage = self.present_damage_tracker.prepare(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        // 将 owner-thread 借用限制在 retained gradient 绘制调用内。
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
            // 已验证 owner 丢失时返回 typed failure，不能恢复旧路径。
            let context = gpu_ctx.rhi_context()?;
            // 冻结 retained texture 与主 drawable 共用的物理范围。
            let extent = context.surface_ref().token().extent;
            // 将纯渐变队列 lowering 为 FramePlan；不支持的操作返回 false。
            surface.canvas.submit_rhi_gradients(
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
        let (damage_plan, damage_commit) = prepared_damage.into_parts();
        // retained gradient 内容必须先合成到 swapchain 才能报告最终 present 成功。
        self.present_rhi_surface_texture(
            TextureHandle::from_raw(retained_texture.raw()),
            damage_plan.present_damage,
        )?;
        // FramePlan present 成功后才推进清理、damage 和 canvas commit。
        // 只有最终合成成功后才消费主 surface 的全清状态。
        self.surface.needs_gpu_clear = false;
        // 清空已经由 pass load action 替代的全清标记。
        self.surface.pending_clear_rects.clear();
        self.present_damage_tracker.commit_prepared(damage_commit);
        // 保持与兼容路径相同的软资源 aging 语义。
        let used_soft = self.surface.canvas.finish_presented_frame();
        self.soft_used_in_last_present = used_soft;
        if !self.has_soft_fallback_allocation() {
            self.soft_fallback_idle_deadline = None;
        }
        // 告知调用方本次 present 已经由 RHI 完成。
        Ok(true)
    }
    // 结束剩余 native RHI 队列的提交实现块。
}
