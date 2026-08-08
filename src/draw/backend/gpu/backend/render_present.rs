//! GPU 后端最终 present 的有序状态机。

// 引入主 surface present 所需的错误和 damage 类型。
use crate::core::{DamageRegion, Errc, Error};
// 引入 RenderBackend trait，使 present 状态机可以结束 active offscreen。
use crate::draw::backend::contract::RenderBackend;
// 引入兼容 swapchain present 帧描述。
use crate::native::present::PresentFrame;
// 引入薄 RHI device 维护原语，确保最终提交前检查设备健康状态。
use crate::native::present::rhi::GraphicsDevice;

// 引入当前 GPU backend owner。
use super::GpuBackend;

// 为 GpuBackend 提供 RenderBackend::present 的完整实现。
impl GpuBackend {
    // 按 RHI、滚动、局部清理、native、soft 的顺序完成唯一最终 present。
    pub(super) fn present_impl(&mut self, damage: &DamageRegion) -> Result<(), Error> {
        // 活跃 Picture 必须先结束，保证主 surface 不会与离屏 target 交错。
        if self.active_offscreen.is_some() {
            self.end_offscreen_paint();
        }
        // 在任何 RHI lowering 或兼容提交前执行 owner-thread device preflight。
        if let Some(context) = self.gpu_ctx.rhi_context() {
            // device lost 必须在最终 present 前按 typed error 暴露给恢复 FSM。
            GraphicsDevice::maintain(context)?;
        }
        // 读取 draw-time deferred error，并保持当前帧 dirty。
        if let Some(error) = self.surface.canvas.take_deferred_error() {
            self.surface.needs_gpu_clear = true;
            return Err(error);
        }
        // 读取 FrameEncoder 或即时绘制记录的失败，并保持当前帧 dirty。
        if let Some(error) = self.frame_failure.take() {
            self.surface.needs_gpu_clear = true;
            return Err(error);
        }
        // 先把仅含 scroll/clear/native queue 的 retained boundary 写入持久纹理。
        if self.active_offscreen.is_none()
            && (!self.surface.pending_scroll_copies.is_empty()
                || !self.surface.pending_clear_rects.is_empty()
                || !self.surface.canvas.pending_native.is_empty()
                || self.surface.canvas.soft_has_content)
        {
            // 失败时保留记录，后续专用 RHI 或兼容路径继续作能力判断。
            let _ = self.try_flush_main_segment_rhi()?;
        }
        // FrameEncoder 主帧已经写入 retained texture 时，先完成唯一最终合成。
        let pending_frame_presented = self.try_present_pending_rhi_frame(damage)?;
        if pending_frame_presented {
            // pending RHI frame 路径已完成 swapchain present 和状态提交。
            return Ok(());
        }
        // 先尝试保序混合 RHI 队列；成功时该路径已经完成唯一 present。
        let mixed_presented = self.try_present_rhi_mixed(damage)?;
        if mixed_presented {
            return Ok(());
        }
        // 纯 solid 队列继续保留更轻量的专用 RHI lowering。
        let solid_presented = self.try_present_rhi_solid(damage)?;
        if solid_presented {
            return Ok(());
        }
        // 圆角/描边矩形也先尝试通用 RHI lowering。
        let shapes_presented = self.try_present_rhi_shapes(damage)?;
        if shapes_presented {
            return Ok(());
        }
        // 轴对齐 box shadow 也先尝试通用 RHI lowering。
        let shadows_presented = self.try_present_rhi_shadows(damage)?;
        if shadows_presented {
            return Ok(());
        }
        // 轴对齐 R8 glyph coverage 也先尝试通用 RHI lowering。
        let glyphs_presented = self.try_present_rhi_glyphs(damage)?;
        if glyphs_presented {
            return Ok(());
        }
        // 纯图片队列也先尝试通用 sampled-quad RHI lowering。
        let textured_presented = self.try_present_rhi_textured(damage)?;
        if textured_presented {
            return Ok(());
        }
        // 纯线性/径向渐变队列也先尝试通用 gradient RHI lowering。
        let gradients_presented = self.try_present_rhi_gradients(damage)?;
        if gradients_presented {
            return Ok(());
        }
        // 未覆盖的 scroll 绝不能被直接 swapchain 路径静默跳过。
        if !self.surface.pending_scroll_copies.is_empty() {
            return Err(Error::new(
                Errc::NotImplemented,
                "RHI scroll copy requires a retained surface lowering",
            ));
        }
        // 所有 RHI 纵切都未覆盖时，legacy 目标不能继续读取旧 retained 副本。
        self.abandon_rhi_surface_texture_for_legacy()?;
        self.gpu_ctx.make_current()?;

        // 先执行整面清理或已经记录的局部清理。
        if self.surface.needs_gpu_clear {
            self.gpu_ctx.clear_render_target(0.0, 0.0, 0.0, 0.0)?;
            self.surface.needs_gpu_clear = false;
            self.surface.pending_clear_rects.clear();
        } else if !self.surface.pending_clear_rects.is_empty() {
            // legacy clear_rects 失败时保留 clear 标记，等待下次重试。
            if let Err(err) = self.gpu_ctx.clear_rects(
                self.surface.width as f32,
                self.surface.height as f32,
                &self.surface.pending_clear_rects,
            ) {
                self.surface.needs_gpu_clear = true;
                return Err(err);
            }
            self.surface.pending_clear_rects.clear();
        }

        // 提交保序 native 队列，并在失败时保持帧未完成。
        if let Err(err) = self.surface.canvas.submit_native(self.gpu_ctx.as_mut()) {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }
        // 提交软回退内容，保证兼容路径仍覆盖 RHI 未支持的操作。
        if let Err(err) = self.surface.canvas.submit_soft(self.gpu_ctx.as_mut()) {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }

        // 生成与 RHI 路径相同的 damage 计划。
        let caps = self.gpu_ctx.caps();
        let present_surface = self.gpu_ctx.present_surface();
        let present_image = self.gpu_ctx.present_image();
        let damage_plan = self.present_damage_tracker.plan(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        // 将兼容内容送入 swapchain。
        let frame = PresentFrame::Swapchain {
            damage: damage_plan.present_damage,
        };
        let present_result = self.gpu_ctx.present(&frame);
        // present 失败时保持下一帧的全清保护。
        if let Err(err) = present_result {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }
        // 只有 swapchain 已提交才推进 damage 和 canvas 生命周期。
        self.present_damage_tracker.commit(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        let mut used_soft = self.surface.canvas.finish_presented_frame();
        // 主 surface 完成后推进各 Picture 软回退资源的 aging。
        for off in self.offscreens.iter_mut().flatten() {
            used_soft |= off.canvas.age_soft_fallback_after_present();
        }
        // 记录本帧是否使用了软回退资源。
        self.soft_used_in_last_present = used_soft;
        if !self.has_soft_fallback_allocation() {
            // 没有软资源时取消空闲回收截止时间。
            self.soft_fallback_idle_deadline = None;
        }
        // 告知 RenderBackend 兼容 present 已经完成。
        Ok(())
    }
}
