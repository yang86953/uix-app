//! GPU 后端最终 present 的有序状态机。

// 引入主 surface present 所需的错误和 damage 类型。
use crate::core::{DamageRegion, Errc, Error};
// 引入 RenderBackend trait，使 present 状态机可以结束 active offscreen。
use crate::draw::backend::contract::RenderBackend;
// 引入薄 RHI device 维护原语，确保最终提交前检查设备健康状态。
use crate::platform::presentation::rhi::GraphicsDevice;

// 引入当前 GPU backend owner。
use super::GpuBackend;

// 把生产主 surface 未完整进入 retained RHI 的结果收敛为稳定 typed failure。
pub(super) fn require_lossless_main_surface_submission(
    // 指示本次有序队列是否已经完整写入 retained texture。
    submitted: bool,
    // 保存具体失败边界，供恢复层和诊断日志定位。
    message: &'static str,
    // 成功时保持无副作用，失败时拒绝任何 direct swapchain 兼容执行。
) -> Result<(), Error> {
    // 只有完整提交才允许最终 present 状态机继续。
    if submitted {
        // 不引入额外资源或状态变更。
        return Ok(());
    }
    // 未覆盖 lowering 必须由上层按 NotImplemented 恢复或下一帧重试。
    Err(Error::new(
        // 明确区分 RHI 覆盖缺口与 surface/device 运行故障。
        Errc::NotImplemented,
        // 保留调用方提供的具体主 surface 阶段。
        message,
    ))
}

// 为 GpuBackend 提供 RenderBackend::present 的完整实现。
impl GpuBackend {
    // 按 RHI、滚动、局部清理、native、soft 的顺序完成唯一最终 present。
    pub(super) fn present_impl(&mut self, damage: &DamageRegion) -> Result<(), Error> {
        // 活跃 Picture 必须先结束，保证主 surface 不会与离屏 target 交错。
        if self.active_offscreen.is_some() {
            // checked end 失败时立即停止主 surface 提交并保留 typed error。
            self.try_end_offscreen_paint()?;
        }
        // 在任何 RHI lowering 或最终提交前执行 owner-thread device preflight。
        // 已验证 owner 必须在最终 present 前完成 owner-thread device preflight。
        let context = self.gpu_ctx.rhi_device()?;
        // device lost 必须在最终 present 前按 typed error 暴露给恢复 FSM。
        GraphicsDevice::maintain(context)?;
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
            && (self.surface.needs_gpu_clear
                || !self.surface.pending_scroll_copies.is_empty()
                || !self.surface.pending_clear_rects.is_empty()
                || !self.surface.canvas.pending_native.is_empty()
                || self.surface.canvas.soft_has_content)
        {
            // 失败时保留记录，后续专用 RHI 路径继续作能力判断。
            let _ = self.try_flush_main_segment_rhi()?;
        }
        // 没有新绘制但已有 retained 内容时，仍通过同一最终合成路径重新 present。
        if self.rhi_surface_texture.is_some()
            && !self.rhi_surface_frame_pending_present
            && !self.surface.needs_gpu_clear
            && self.surface.pending_scroll_copies.is_empty()
            && self.surface.pending_clear_rects.is_empty()
            && self.surface.canvas.pending_native.is_empty()
            && !self.surface.canvas.soft_has_content
        {
            // 将既有 retained 内容登记为本帧唯一待 present 来源。
            self.rhi_surface_frame_pending_present = true;
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
        // 所有 RHI 方案均拒绝当前非空语义时，保留下一帧完整清理边界。
        self.surface.needs_gpu_clear = true;
        // 禁止重新进入逐 UI adapter 调用或 direct swapchain present。
        require_lossless_main_surface_submission(
            // 到达此处说明没有任何 RHI 路径完整提交当前帧。
            false,
            // 给恢复层保留稳定、可检索的主 surface lowering 边界。
            "main surface commands cannot be lowered losslessly to retained RHI",
        )
    }
}
