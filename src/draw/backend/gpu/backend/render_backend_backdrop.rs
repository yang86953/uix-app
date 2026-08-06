//! [`GpuBackend`] 的 overlay backdrop 实现 — backend 子模块。
//!
//! 叠加层 backdrop 快照、恢复与释放，仅在 retained framebuffer 能力可用时生效。

use crate::core::Error;

use super::GpuBackend;

// 叠加层 backdrop 快照依赖 retained framebuffer 与 gpu_ctx 的独立生命周期。
impl GpuBackend {
    pub(super) fn snapshot_overlay_backdrop_impl(&mut self) -> bool {
        // 没有保留缓冲的 adapter 不支持叠加层快照。
        if !self.surface.native_caps.retained_framebuffer {
            return false;
        }
        // 快照前先确保 owner-thread context 可用。
        if let Err(err) = self.gpu_ctx.make_current() {
            tracing::warn!(
                "GpuBackend: snapshot_overlay_backdrop make_current failed: {}",
                err.short_what()
            );
            return false;
        }
        // 快照成功与否都以布尔结果报告，错误只降级为日志。
        match self.gpu_ctx.snapshot_overlay_backdrop() {
            Ok(()) => true,
            Err(err) => {
                tracing::warn!(
                    "GpuBackend: snapshot_overlay_backdrop failed: {}",
                    err.short_what()
                );
                false
            }
        }
    }

    pub(super) fn restore_overlay_backdrop_impl(&mut self) -> bool {
        // 没有快照时无需恢复，直接成功返回。
        if !self.gpu_ctx.has_overlay_backdrop() {
            return false;
        }
        // 恢复前同样确保 owner-thread context 可用。
        if let Err(err) = self.gpu_ctx.make_current() {
            tracing::warn!(
                "GpuBackend: restore_overlay_backdrop make_current failed: {}",
                err.short_what()
            );
            return false;
        }
        // 恢复成功后取消 begin_frame 挂起的全幅 clear。
        match self.gpu_ctx.restore_overlay_backdrop() {
            Ok(()) => {
                // 快照已写入保留缓冲：取消 begin_frame 挂起的全幅 clear。
                self.surface.needs_gpu_clear = false;
                self.surface.pending_clear_rects.clear();
                true
            }
            Err(err) => {
                tracing::warn!(
                    "GpuBackend: restore_overlay_backdrop failed: {}",
                    err.short_what()
                );
                false
            }
        }
    }

    pub(super) fn release_overlay_backdrop_impl(&mut self) {
        // 释放由底层 context 负责，幂等。
        self.gpu_ctx.release_overlay_backdrop();
    }

    pub(super) fn has_overlay_backdrop_impl(&self) -> bool {
        // 查询当前是否持有可恢复的叠加层快照。
        self.gpu_ctx.has_overlay_backdrop()
    }
}
