//! [`GpuBackend`] 生命周期与资源管理 — backend 子模块。
//!
//! 后端构造、offscreen 槽管理与软回退分配。

use std::time::Instant;

use super::super::canvas::NativeGpuCanvas2D;
use super::super::SOFT_FALLBACK_IDLE_TIME_GRACE;
use super::logical_metadata_from_surface;
use super::surface::NativeGpuDrawSurface;
use super::GpuBackend;
use crate::core::{Errc, Error, PresentDamageTracker};
use crate::draw::backend::contract::RenderBackend;
use crate::draw::geometry::types::ImageHandle;
// 使用薄 RHI 的设备维护入口承接每帧 owner-context 准备。
use crate::native::present::rhi::GraphicsDevice;
// 引入 platform 构造期验证的 context recipe owner。
use crate::native::present::GpuRecipeOwner;
// 引入所属 graphics backend Module 的 renderer 能力投影。
use super::super::NativeRasterCaps;

impl GpuBackend {
    // 在通用 renderer 不感知平台 current API 的前提下准备本帧 RHI device。
    pub(super) fn prepare_rhi_device(&mut self) -> Result<(), Error> {
        // 生产 GPU backend 的构造门禁已经要求组合 thin RHI 始终存在。
        // 已验证 owner 将运行期状态破坏直接映射为 typed error。
        let context = self.gpu_ctx.rhi_context()?;
        // adapter 在这里完成设备健康检查；OpenGL 同时恢复 owner context current。
        GraphicsDevice::maintain(context)
    }

    // 把可控 device-lost 注入送入当前 owner-thread 的薄 RHI。
    #[cfg(feature = "test-harness")]
    pub(crate) fn inject_graphics_device_lost_for_test(&mut self) -> Result<(), Error> {
        // 缺少薄 RHI 属于构造后状态破坏，测试注入必须返回 typed failure。
        // 测试注入沿用已验证 owner 的 typed 借用边界。
        let context = self.gpu_ctx.rhi_context()?;
        // 由 adapter 自己保存一次性注入状态，最终 present 才报告 typed failure。
        context.inject_device_lost_for_test()
    }

    // 把可控 surface-lost 注入送入当前 owner-thread 的薄 RHI surface。
    #[cfg(feature = "test-harness")]
    pub(crate) fn inject_graphics_surface_lost_for_test(&mut self) -> Result<(), Error> {
        // 缺少薄 RHI 属于构造后状态破坏，测试注入必须返回 typed failure。
        // 测试注入沿用已验证 owner 的 typed 借用边界。
        let context = self.gpu_ctx.rhi_context()?;
        // 由 adapter 自己保存一次性注入状态，下一次 acquire 才报告 typed failure。
        context.inject_surface_lost_for_test()
    }

    // 构造已经由 native factory 完成 surface 准备的 GPU-only backend。
    pub(crate) fn new_gpu_only(mut gpu_ctx: GpuRecipeOwner) -> Result<Self, Error> {
        // 一次读取静态 recipe 事实，供构造门禁和 typed error 复用。
        let caps = gpu_ctx.caps();
        // 从组合 thin RHI 一次取得已由 factory probe 验证的事实快照。
        // 只在这个局部借用已验证 owner，随后保存可复制的能力值。
        let rhi_capabilities = Some(gpu_ctx.rhi_context()?.capabilities());
        // 从同一快照派生通用 renderer 真正消费的窄能力投影。
        let native_caps = rhi_capabilities
            // 保留 retained 与 Additive 两项绘制事实。
            .map(NativeRasterCaps::from_rhi_capabilities)
            // 缺少薄 RHI 时使用空投影进入统一 typed failure。
            .unwrap_or_default();
        // 构造门禁同时要求完整 GPU 原语基线和 retained 主颜色目标。
        let raster_baseline = rhi_capabilities
            // 缺少薄 RHI 或任一 GPU 基线原语都不能构造生产 backend。
            .is_some_and(|capabilities| capabilities.has_gpu_baseline())
            // GPU-only canvas 还必须持有跨帧 retained 事实。
            && native_caps.has_gpu_only_baseline();
        if !raster_baseline {
            // 保存诊断所需的静态 backend 身份。
            let backend = caps.backend;
            // 保存诊断所需的 raster recipe。
            let raster = caps.raster;
            // 保存诊断所需的 present recipe。
            let present = caps.present;
            // 构造失败前检查式关闭已经创建的 native owner。
            gpu_ctx.try_shutdown()?;
            // 返回稳定参数错误，交由上层选择其它 recipe。
            return Err(Error::new(
                // 能力不完整属于构造参数与 recipe 不匹配。
                Errc::InvalidArgument,
                // 同时记录唯一 RHI 快照与 renderer 投影，避免平行声明掩盖差异。
                format!(
                    "GpuBackend requires a complete GPU-only retained RHI baseline, got {backend} raster={raster} present={present} rhi={rhi_capabilities:?} renderer={native_caps:?}"
                ),
            ));
        }
        // 一次读取 live surface，避免 extent 与 DPR 来自不同生命周期时刻。
        let present_surface = gpu_ctx.present_surface();
        // 从同一快照派生逻辑 canvas 元数据。
        let ((logical_w, logical_h), device_pixel_ratio) =
            logical_metadata_from_surface(present_surface);
        // 只有已取得完整薄 RHI 能力快照的参考 adapter 创建 lowering cache。
        let rhi_renderer = rhi_capabilities
            // 能力值只作为已存在组合 RHI 的构造证明。
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
        // 在借用组合 RHI 前一次读取完整 drawable 元数据。
        let present_surface = self.gpu_ctx.present_surface();
        // 从同一快照保存当前 drawable 宽度。
        let width = present_surface.drawable_width.max(1);
        // 从同一快照保存当前 drawable 高度。
        let height = present_surface.drawable_height.max(1);
        // 构造后丢失组合 RHI 属于生命周期状态破坏。
        // 已验证 owner 将运行期 context 状态破坏收敛为 typed error。
        let context = self.gpu_ctx.rhi_context()?;
        // 只调用 adapter 事实声明支持的可选能力。
        if !context.capabilities().surface_readback {
            // 缺少可选能力必须返回 typed 错误，不能伪造空结果。
            return Err(Error::new(
                // 使用未实现错误区分 adapter 能力缺口。
                Errc::NotImplemented,
                // 提供稳定的诊断消息。
                "GPU backend thin RHI does not support surface readback",
            ));
        }
        // 通过组合 RHI surface 执行 owner-thread 回读。
        context.read_surface_pixels(0, 0, width, height)
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
        // factory/surface 变更后一次读取新的完整元数据快照。
        let present_surface = self.gpu_ctx.present_surface();
        // 从同一快照更新逻辑 canvas extent 与 DPR。
        let ((logical_w, logical_h), device_pixel_ratio) =
            logical_metadata_from_surface(present_surface);
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
