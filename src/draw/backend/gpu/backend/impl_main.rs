//! [`GpuBackend`] 生命周期与资源管理 — backend 子模块。
//!
//! 后端构造、offscreen 槽管理与软回退分配。

use std::time::Instant;

use super::super::canvas::NativeGpuCanvas2D;
use super::super::SOFT_FALLBACK_IDLE_TIME_GRACE;
use super::surface::NativeGpuDrawSurface;
use super::GpuBackend;
use super::{device_pixel_ratio_from_context, logical_extent_from_context};
use crate::core::{Errc, Error, PresentDamageTracker};
use crate::draw::backend::contract::RenderBackend;
use crate::draw::geometry::types::ImageHandle;
// 使用薄 RHI 的设备维护入口承接每帧 owner-context 准备。
use crate::native::present::rhi::GraphicsDevice;
use crate::native::present::{IGraphicsContext, PresentMode, RasterMode};

impl GpuBackend {
    // 在通用 renderer 不感知平台 current API 的前提下准备本帧 RHI device。
    pub(super) fn prepare_rhi_device(&mut self) -> Result<(), Error> {
        // 生产 GPU backend 的构造门禁已经要求组合 thin RHI 始终存在。
        let Some(context) = self.gpu_ctx.rhi_context() else {
            // 构造后的能力消失属于状态破坏，而不是可降级的功能缺口。
            return Err(Error::new(
                // 使用稳定状态错误触发既有恢复流程。
                Errc::InvalidState,
                // 明确指出 owner context 已丢失。
                "GPU backend lost its thin RHI context before frame preparation",
            ));
        };
        // adapter 在这里完成设备健康检查；OpenGL 同时恢复 owner context current。
        GraphicsDevice::maintain(context)
    }

    // 把可控 device-lost 注入送入当前 owner-thread 的薄 RHI。
    #[cfg(feature = "test-harness")]
    pub(crate) fn inject_graphics_device_lost_for_test(&mut self) -> Result<(), Error> {
        // 缺少薄 RHI 属于构造后状态破坏，测试注入必须返回 typed failure。
        let Some(context) = self.gpu_ctx.rhi_context() else {
            return Err(Error::new(
                Errc::NotImplemented,
                "GPU backend does not expose a thin RHI device",
            ));
        };
        // 由 adapter 自己保存一次性注入状态，最终 present 才报告 typed failure。
        context.inject_device_lost_for_test()
    }

    // 把可控 surface-lost 注入送入当前 owner-thread 的薄 RHI surface。
    #[cfg(feature = "test-harness")]
    pub(crate) fn inject_graphics_surface_lost_for_test(&mut self) -> Result<(), Error> {
        // 缺少薄 RHI 属于构造后状态破坏，测试注入必须返回 typed failure。
        let Some(context) = self.gpu_ctx.rhi_context() else {
            return Err(Error::new(
                Errc::NotImplemented,
                "GPU backend does not expose a thin RHI surface",
            ));
        };
        // 由 adapter 自己保存一次性注入状态，下一次 acquire 才报告 typed failure。
        context.inject_surface_lost_for_test()
    }

    // 构造已经由 native factory 完成 surface 准备的 GPU-only backend。
    pub(crate) fn new_gpu_only(mut gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let caps = gpu_ctx.caps();
        let native_caps = gpu_ctx.native_raster_caps();
        // 生产 GPU backend 不再接受依赖兼容 soft upload 的 hybrid 基线。
        let raster_baseline = native_caps.has_gpu_only_baseline();
        // 生产 GPU backend 必须持有已经通过 factory probe 的组合 thin RHI。
        let has_rhi_context = gpu_ctx.rhi_context().is_some();
        if caps.raster != RasterMode::GpuNative
            || caps.present != PresentMode::Swapchain
            || !raster_baseline
            || !has_rhi_context
        {
            let backend = caps.backend;
            let raster = caps.raster;
            let present = caps.present;
            gpu_ctx.try_shutdown()?;
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "GpuBackend requires a complete GPU-only retained RHI baseline, got {backend} raster={raster} present={present} thin_rhi={has_rhi_context} native={native_caps:?}"
                ),
            ));
        }
        let (logical_w, logical_h) = logical_extent_from_context(gpu_ctx.as_ref());
        let device_pixel_ratio = device_pixel_ratio_from_context(gpu_ctx.as_ref());
        // 只有已暴露薄 RHI 组合视图的参考 adapter 创建 lowering cache。
        let rhi_renderer = gpu_ctx
            .rhi_context()
            .map(|_| super::super::super::rhi_renderer::RhiRenderer::default());
        // 生产主 surface 固定使用禁止 legacy soft upload 的 GPU-only canvas。
        let mut canvas = NativeGpuCanvas2D::new_gpu_only(logical_w, logical_h, native_caps);
        canvas.set_device_pixel_ratio(device_pixel_ratio);
        Ok(Self {
            gpu_ctx,
            width: logical_w,
            height: logical_h,
            shutdown: false,
            offscreens: Vec::new(),
            free_offscreen_ids: Vec::new(),
            next_offscreen_id: 0,
            active_offscreen: None,
            offscreen_rhi_initialized: false,
            offscreen_flush_committed: false,
            frame_failure: None,
            present_damage_tracker: PresentDamageTracker::new(),
            soft_fallback_idle_deadline: None,
            soft_used_in_last_present: false,
            rhi_renderer,
            // 启动时延迟创建 retained texture，避免在 context 尚未完成 probe 前占用资源。
            rhi_surface_texture: None,
            // 没有 texture 时不存在可复用的 surface 代际。
            rhi_surface_token: None,
            // 首帧还没有 FrameEncoder 写入 retained target。
            rhi_surface_frame_pending_present: false,
            // 启动时尚未捕获 overlay 干净背景。
            rhi_overlay_backdrop_texture: None,
            // 没有 backdrop texture 时不存在对应 surface 代际。
            rhi_overlay_backdrop_token: None,
            surface: NativeGpuDrawSurface {
                canvas,
                native_caps,
                width: logical_w,
                height: logical_h,
                needs_gpu_clear: true,
                pending_clear_rects: Vec::new(),
                pending_scroll_copies: Vec::new(),
            },
        })
    }

    /// Flushes the current ordered segment without presenting, then reads the
    /// native drawable. This is a crate-local diagnostic/test boundary; it
    /// deliberately uses the same command ordering as a final present.
    // 测试目标保留原生回读诊断入口，供启用具体 GPU feature 的契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(all(test, any(feature = "opengles", feature = "d3d11", feature = "d3d12")))]
    pub(crate) fn try_readback(&mut self) -> Result<Vec<u32>, Error> {
        if self.active_offscreen.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "cannot read the swapchain while an offscreen target is active",
            ));
        }
        self.flush_main_segment_before_ordered_boundary()?;
        let width = self.gpu_ctx.width().max(1);
        let height = self.gpu_ctx.height().max(1);
        self.gpu_ctx.read_pixels(0, 0, width, height)
    }

    // 测试目标保留 soft upload 字节数观测入口，供 GPU 诊断测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn last_soft_upload_bytes(&self) -> usize {
        self.surface.canvas.last_soft_upload_bytes
    }

    pub(super) fn destroy_all_offscreens(&mut self) -> Result<(), Error> {
        self.active_offscreen = None;
        self.offscreen_flush_committed = false;
        let handles = self
            .offscreens
            .iter()
            .enumerate()
            .filter_map(|(id, target)| target.as_ref().map(|_| ImageHandle(id as u32)))
            .collect::<Vec<_>>();
        for handle in handles {
            self.try_destroy_offscreen(handle)?;
        }
        self.offscreens.clear();
        self.free_offscreen_ids.clear();
        self.next_offscreen_id = 0;
        Ok(())
    }

    pub(super) fn compact_offscreen_slots(&mut self) {
        while self.offscreens.last().is_some_and(Option::is_none) {
            self.offscreens.pop();
        }
        self.free_offscreen_ids
            .retain(|id| (*id as usize) < self.offscreens.len());
        self.next_offscreen_id = self.offscreens.len() as u32;
    }

    pub(super) fn remember_frame_failure(&mut self, error: Error) {
        if self.frame_failure.is_none() {
            self.frame_failure = Some(error);
        }
        // Commands before an immediate boundary may already have reached the
        // target.  The next retained-dirty retry must start from a known full
        // clear rather than alpha-blending on that partial target.
        self.surface.needs_gpu_clear = true;
    }

    pub(super) fn adopt_factory_drawable_extent(&mut self) -> (i32, i32) {
        let (logical_w, logical_h) = logical_extent_from_context(self.gpu_ctx.as_ref());
        let device_pixel_ratio = device_pixel_ratio_from_context(self.gpu_ctx.as_ref());
        self.width = logical_w;
        self.height = logical_h;
        self.surface.width = logical_w;
        self.surface.height = logical_h;
        self.surface.canvas.resize(logical_w, logical_h);
        self.surface
            .canvas
            .set_device_pixel_ratio(device_pixel_ratio);
        self.soft_fallback_idle_deadline = None;
        self.soft_used_in_last_present = false;
        self.offscreen_flush_committed = false;
        self.surface.needs_gpu_clear = true;
        self.surface.pending_clear_rects.clear();
        self.surface.pending_scroll_copies.clear();
        // drawable extent 变化后，下一次 RHI submit 必须重新确认 retained token。
        self.rhi_surface_frame_pending_present = false;
        (logical_w, logical_h)
    }

    pub(super) fn has_soft_fallback_allocation(&self) -> bool {
        self.surface.canvas.soft_fallback.is_some()
            || self
                .offscreens
                .iter()
                .flatten()
                .any(|off| off.canvas.soft_fallback.is_some())
    }

    pub(crate) fn note_presented_at(&mut self, now: Instant) {
        if self.soft_used_in_last_present {
            self.soft_fallback_idle_deadline = now.checked_add(SOFT_FALLBACK_IDLE_TIME_GRACE);
        } else if !self.has_soft_fallback_allocation() {
            self.soft_fallback_idle_deadline = None;
        }
        self.soft_used_in_last_present = false;
    }

    pub(crate) fn idle_resource_deadline(&self) -> Option<Instant> {
        self.soft_fallback_idle_deadline
    }

    pub(crate) fn release_idle_resources(&mut self, now: Instant) {
        if self
            .soft_fallback_idle_deadline
            .is_none_or(|deadline| deadline > now)
        {
            return;
        }
        self.surface.canvas.release_idle_soft_fallback();
        for offscreen in self.offscreens.iter_mut().flatten() {
            offscreen.canvas.release_idle_soft_fallback();
        }
        self.soft_fallback_idle_deadline = None;
    }

    /// Submit all commands that precede an immediate ordered operation such
    /// as a Picture/offscreen blit.  This is deliberately *not* a present:
    /// it only establishes the exact painter-order boundary inside the one
    /// frame and leaves final swap/present to [`RenderBackend::present`].
    pub(super) fn flush_main_segment_before_ordered_boundary(&mut self) -> Result<(), Error> {
        // 没有任何前置内容且 retained target 已初始化时，有序 boundary 可以安全 no-op。
        if self.rhi_surface_texture.is_some()
            && !self.surface.needs_gpu_clear
            && !self.surface.canvas.soft_has_content
            && self.surface.canvas.pending_native.is_empty()
            && self.surface.pending_clear_rects.is_empty()
            && self.surface.pending_scroll_copies.is_empty()
        {
            // 后续 RHI Picture blit 可以继续写入同一 retained target。
            return Ok(());
        }
        // 优先把可验证的 native queue 写入 retained target，保持 painter order。
        if self.try_flush_main_segment_rhi()? {
            // 最终 swapchain present 仍由统一帧边界完成。
            return Ok(());
        }
        // 未完整提交的有序前缀必须让下一帧从确定的全清状态重试。
        self.surface.needs_gpu_clear = true;
        // 使用与最终 present 相同的 typed 门禁，禁止切换到逐 UI adapter 执行。
        super::render_present::require_lossless_main_surface_submission(
            // 到达此处说明 retained RHI 未完整消费当前前缀。
            false,
            // 明确失败发生在 Picture/effect 之前的主 surface 顺序边界。
            "ordered main surface prefix cannot be lowered losslessly to retained RHI",
        )
    }
}
