//! [`GpuBackend`] 生命周期与资源管理 — backend 子模块。
//!
//! 后端构造、offscreen 槽管理与软回退分配。

use std::time::Instant;

use super::super::SOFT_FALLBACK_IDLE_TIME_GRACE;
use super::super::canvas::NativeGpuCanvas2D;
// 引入 Drawing GPU Module 私有的统一 FramePlan 启动探针。
use super::super::device_probe::probe_device;
use super::GpuBackend;
use super::logical_metadata_from_surface;
use super::surface::NativeGpuDrawSurface;
use crate::core::{Errc, Error, PresentDamageTracker};
use crate::draw::backend::contract::RenderBackend;
// 测试读回与 Agent 截屏只返回 Drawing 层的 API 无关快照。
#[cfg(any(feature = "test-harness", feature = "agent-control"))]
use crate::draw::backend::contract::SurfaceReadback;
use crate::draw::backend::slot_pool::SlotPool;
use crate::draw::geometry::types::ImageHandle;
// 使用薄 RHI 的设备维护入口承接每帧 owner-context 准备。
use crate::platform::presentation::rhi::GraphicsDevice;
// 读回能力启用时才引入窄 Surface 角色与统一区域描述。
#[cfg(any(feature = "test-harness", feature = "agent-control"))]
use crate::platform::presentation::rhi::{GraphicsSurface, RhiScissor};
// 引入 platform 构造期验证的 context recipe owner。
use crate::platform::presentation::rhi::GpuRecipeOwner;
// 引入所属 graphics backend Module 的 renderer 能力投影。
use super::super::NativeRasterCaps;

// GPU backend 构造失败时检查式关闭 native owner，并保留主失败因果。
fn shutdown_gpu_owner_with_error(
    // 借用尚未进入 GpuBackend 所有权的 recipe owner。
    gpu_ctx: &mut GpuRecipeOwner,
    // 接收导致当前候选被拒绝的主错误。
    primary_error: Error,
    // 返回主错误或以主错误为原因的 shutdown 失败。
) -> Error {
    // 所有构造拒绝都必须尝试释放当前候选的原生资源。
    match gpu_ctx.try_shutdown() {
        // shutdown 成功时保持原始拒绝语义。
        Ok(()) => primary_error,
        // shutdown 失败时让生命周期错误成为主错误并链接原始原因。
        Err(cleanup_error) => cleanup_error.with_source(primary_error),
    }
}

impl GpuBackend {
    // 在通用 renderer 不感知平台 current API 的前提下准备本帧 RHI device。
    pub(super) fn prepare_rhi_device(&mut self) -> Result<(), Error> {
        // 生产 GPU backend 的构造门禁已经要求组合 thin RHI 始终存在。
        // 已验证 owner 借用会先激活对应原生 context。
        let context = self.gpu_ctx.rhi_device()?;
        // 激活后再完成设备健康检查，保持两类失败语义正交。
        GraphicsDevice::maintain(context)
    }

    // 把可控 device-lost 注入送入当前 owner-thread 的薄 RHI。
    #[cfg(feature = "test-harness")]
    pub(crate) fn inject_graphics_device_lost_for_test(&mut self) -> Result<(), Error> {
        // 缺少薄 RHI 属于构造后状态破坏，测试注入必须返回 typed failure。
        // 测试注入沿用已验证 owner 的 typed 借用边界。
        let context = self.gpu_ctx.rhi_device()?;
        // 由 adapter 自己保存一次性注入状态，最终 present 才报告 typed failure。
        context.inject_device_lost_for_test()
    }

    // 把可控 surface-lost 注入送入当前 owner-thread 的薄 RHI surface。
    #[cfg(feature = "test-harness")]
    pub(crate) fn inject_graphics_surface_lost_for_test(&mut self) -> Result<(), Error> {
        // 缺少薄 RHI 属于构造后状态破坏，测试注入必须返回 typed failure。
        // 测试注入沿用已验证 owner 的 typed 借用边界。
        let context = self.gpu_ctx.rhi_surface()?;
        // 由 adapter 自己保存一次性注入状态，下一次 acquire 才报告 typed failure。
        context.inject_surface_lost_for_test()
    }

    // 构造已经由 native factory 完成 surface 准备的 GPU-only backend。
    pub(crate) fn new_gpu_only(mut gpu_ctx: GpuRecipeOwner) -> Result<Self, Error> {
        // 一次读取静态 recipe 事实，供构造门禁和 typed error 复用。
        let caps = gpu_ctx.caps();
        // 从组合 thin RHI 一次取得 native factory 已验证形状的 Device 能力快照。
        let rhi_capabilities = match gpu_ctx.rhi_device() {
            // 只在这个局部借用 owner，随后保存可复制的能力值。
            Ok(device) => device.device_capabilities(),
            // 借用失败也必须检查式关闭尚未交付给 backend 的 owner。
            Err(error) => return Err(shutdown_gpu_owner_with_error(&mut gpu_ctx, error)),
        };
        // 从同一快照派生通用 renderer 真正消费的窄能力投影。
        let native_caps = NativeRasterCaps::from_device_capabilities(rhi_capabilities);
        // 构造门禁同时要求完整 GPU 原语基线和 retained 主颜色目标。
        let raster_baseline = rhi_capabilities.has_gpu_baseline()
            // GPU-only canvas 还必须持有跨帧 retained 事实。
            && native_caps.has_gpu_only_baseline();
        if !raster_baseline {
            // 保存诊断所需的静态 backend 身份。
            let backend = caps.backend;
            // 保存诊断所需的 raster recipe。
            let raster = caps.raster;
            // 保存诊断所需的 present recipe。
            let present = caps.present;
            // 构造稳定参数错误，交由上层选择其它 recipe。
            let error = Error::new(
                // 能力不完整属于构造参数与 recipe 不匹配。
                Errc::InvalidArgument,
                // 同时记录唯一 RHI 快照与 renderer 投影，避免平行声明掩盖差异。
                format!(
                    "GpuBackend requires a complete GPU-only retained RHI baseline, got {backend} raster={raster} present={present} rhi={rhi_capabilities:?} renderer={native_caps:?}"
                ),
            );
            // 拒绝当前候选前检查式关闭 owner 并保留失败因果。
            return Err(shutdown_gpu_owner_with_error(&mut gpu_ctx, error));
        }
        // 在 Drawing System 内通过共享 FramePlan 验证固定 pipeline、Shape ABI 与 Device 命令。
        let probe_result = match gpu_ctx.rhi_device() {
            // 探针只取得 Device 角色，不能触碰 Surface 生命周期。
            Ok(device) => probe_device(device),
            // owner-thread 激活或借用失败保持原始 typed error。
            Err(error) => Err(error),
        };
        // probe 失败时由 probe 先清理临时资源，再关闭整个候选 owner。
        if let Err(error) = probe_result {
            // 当前候选不得带着部分初始化资源进入后续 recipe 回退。
            return Err(shutdown_gpu_owner_with_error(&mut gpu_ctx, error));
        }
        // 只有共享 FramePlan 真正提交通过后才建立 GPU backend。
        tracing::info!("Drawing GPU FramePlan startup probe passed");
        // 一次读取 live surface，避免 extent 与 DPR 来自不同生命周期时刻。
        let present_surface = gpu_ctx.present_surface();
        // 从同一快照派生逻辑 canvas 元数据。
        let ((logical_w, logical_h), device_pixel_ratio) =
            logical_metadata_from_surface(present_surface);
        // 只有已取得完整薄 RHI 能力快照的参考 adapter 创建 lowering cache。
        let rhi_renderer = Some(super::super::super::rhi_renderer::RhiRenderer::default());
        // 生产主 surface 固定使用禁止 legacy soft upload 的 GPU-only canvas。
        let mut canvas = NativeGpuCanvas2D::new_gpu_only(logical_w, logical_h, native_caps);
        canvas.set_device_pixel_ratio(device_pixel_ratio);
        Ok(Self {
            gpu_ctx,
            width: logical_w,
            height: logical_h,
            shutdown: false,
            offscreens: SlotPool::new(),
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
            // 默认不执行任何 surface 回读。
            #[cfg(any(feature = "test-harness", feature = "agent-control"))]
            surface_readback_requested: false,
            // 默认不存在上一帧测试结果。
            #[cfg(any(feature = "test-harness", feature = "agent-control"))]
            surface_readback_result: None,
            // 首帧尚未通过共享 FramePlan 执行主表面纹理移动。
            #[cfg(any(feature = "test-harness", feature = "agent-control"))]
            executed_texture_moves_in_frame: 0,
            // 启动时尚未捕获 overlay 干净背景。
            rhi_overlay_backdrop_texture: None,
            // 启动时不存在派生的 overlay effect 纹理。
            rhi_overlay_backdrop_effect_texture: None,
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

    // 在最终 FramePlan 已 submit、尚未 present 的窄 Surface 上读取真实像素。
    #[cfg(any(feature = "test-harness", feature = "agent-control"))]
    pub(super) fn try_readback(
        // 借用正在执行唯一最终呈现事务的 Surface 角色。
        surface: &mut dyn GraphicsSurface,
        // 返回不携带原生类型的 Drawing 快照。
    ) -> Result<SurfaceReadback, Error> {
        // 从同一个 Surface 读取回读能力，避免另一份 capability 快照漂移。
        let supports_readback = surface.surface_capabilities().readback;
        // 只调用 adapter 事实声明支持的可选能力。
        if !supports_readback {
            // 缺少可选能力必须返回 typed 错误，不能伪造空结果。
            return Err(Error::new(
                // 使用未实现错误区分 adapter 能力缺口。
                Errc::NotImplemented,
                // 提供稳定的诊断消息。
                "GPU backend thin RHI does not support surface readback",
            ));
        }
        // 冻结最终 composite 使用的 surface token 与物理 extent。
        let token = surface.token();
        // 公共 Drawing 尺寸使用有符号值，超大 surface 必须显式拒绝。
        let width = i32::try_from(token.extent.width).map_err(|_| {
            // 无法表示的物理宽度属于无效 surface 状态。
            Error::new(Errc::InvalidState, "surface readback width exceeds i32")
        })?;
        // 对物理高度执行相同的有界转换。
        let height = i32::try_from(token.extent.height).map_err(|_| {
            // 无法表示的物理高度属于无效 surface 状态。
            Error::new(Errc::InvalidState, "surface readback height exceeds i32")
        })?;
        // 通过同一个窄 Surface 角色执行 owner-thread 回读。
        let readback = surface.read_surface_pixels(RhiScissor {
            // 从 drawable 左边界开始。
            x: 0,
            // 从 drawable 顶部开始。
            y: 0,
            // 读取完整物理宽度。
            width,
            // 读取完整物理高度。
            height,
        })?;
        // RHI 已统一校验行序、通道与长度，此处只转移规范像素所有权。
        Ok(SurfaceReadback::from_argb(
            // 保存读取时观察到的完整物理宽度。
            width,
            // 保存读取时观察到的完整物理高度。
            height,
            // 隐藏 RHI 结果类型，只把规范像素提升到 Drawing 契约。
            readback.into_pixels(),
        ))
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
            .occupied_ids()
            .map(ImageHandle)
            .collect::<Vec<_>>();
        for handle in handles {
            self.try_destroy_offscreen(handle)?;
        }
        self.offscreens.clear();
        Ok(())
    }

    pub(super) fn compact_offscreen_slots(&mut self) {
        self.offscreens.compact();
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
                .iter_values()
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
        for offscreen in self.offscreens.iter_values_mut() {
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
