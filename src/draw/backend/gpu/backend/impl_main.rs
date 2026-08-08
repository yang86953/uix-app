//! [`GpuBackend`] 生命周期与帧编码执行 — backend 子模块。
//!
//! 后端构造、offscreen 槽管理、软回退分配与主帧 encoder 的 native 执行
//! （含 destination-dependent 回退判定）。

use std::time::Instant;

use crate::core::{Errc, Error, PresentDamageTracker};
use crate::draw::backend::contract::RenderBackend;
use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::painting::{
    FrameCommand, FrameEncoder, FrameEncoderError, FrameRasterOp, FrameRect,
};
use crate::native::present::{
    IGraphicsContext, PresentMode, RasterMode,
};
use super::super::canvas::NativeGpuCanvas2D;
use super::super::SOFT_FALLBACK_IDLE_TIME_GRACE;
use super::surface::NativeGpuDrawSurface;
use super::GpuBackend;
use super::{device_pixel_ratio_from_context, logical_extent_from_context};

impl GpuBackend {
    // 保留 hybrid GPU backend 的兼容构造器，当前 bootstrap 使用 new_gpu_only 或带模式入口。
    #[allow(dead_code)]
    pub(crate) fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        Self::new_with_mode(gpu_ctx, false, false)
    }

    pub(crate) fn new_gpu_only(gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        Self::new_with_mode(gpu_ctx, true, true)
    }

    // 把可控 device-lost 注入送入当前 owner-thread 的薄 RHI。
    #[cfg(feature = "test-harness")]
    pub(crate) fn inject_graphics_device_lost_for_test(&mut self) -> Result<(), Error> {
        // 没有薄 RHI 的 GPU 后端继续使用恢复包装器兼容回退。
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
        // 没有薄 RHI 的 GPU 后端继续使用恢复包装器兼容回退。
        let Some(context) = self.gpu_ctx.rhi_context() else {
            return Err(Error::new(
                Errc::NotImplemented,
                "GPU backend does not expose a thin RHI surface",
            ));
        };
        // 由 adapter 自己保存一次性注入状态，下一次 acquire 才报告 typed failure。
        context.inject_surface_lost_for_test()
    }

    pub(super) fn new_with_mode(
        mut gpu_ctx: Box<dyn IGraphicsContext>,
        gpu_only: bool,
        factory_prepared: bool,
    ) -> Result<Self, Error> {
        let caps = gpu_ctx.caps();
        let native_caps = gpu_ctx.native_raster_caps();
        let raster_baseline = if gpu_only {
            native_caps.has_gpu_only_baseline()
        } else {
            native_caps.has_hybrid_baseline()
        };
        if caps.raster != RasterMode::GpuNative
            || caps.present != PresentMode::Swapchain
            || !raster_baseline
        {
            let backend = caps.backend;
            let raster = caps.raster;
            let present = caps.present;
            gpu_ctx.try_shutdown()?;
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "GpuBackend requires a complete {:?} raster baseline, got {backend} raster={raster} present={present} native={native_caps:?}",
                    if gpu_only { "GPU-only" } else { "hybrid" }
                ),
            ));
        }
        let (logical_w, logical_h) = logical_extent_from_context(gpu_ctx.as_ref());
        let device_pixel_ratio = device_pixel_ratio_from_context(gpu_ctx.as_ref());
        // 只有已暴露薄 RHI 组合视图的参考 adapter 创建 lowering cache。
        let rhi_renderer = gpu_ctx
            .rhi_context()
            .map(|_| super::super::super::rhi_renderer::RhiRenderer::default());
        let mut canvas = if gpu_only {
            NativeGpuCanvas2D::new_gpu_only(logical_w, logical_h, native_caps)
        } else {
            NativeGpuCanvas2D::new(logical_w, logical_h, native_caps)
        };
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
            gpu_only,
            factory_prepared,
            rhi_renderer,
            // 启动时延迟创建 retained texture，避免在 context 尚未完成 probe 前占用资源。
            rhi_surface_texture: None,
            // 没有 texture 时不存在可复用的 surface 代际。
            rhi_surface_token: None,
            // 首帧还没有 FrameEncoder 写入 retained target。
            rhi_surface_frame_pending_present: false,
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
        self.gpu_ctx.bind_swapchain_target()?;
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
        // 已有 retained target 且没有待提交内容时，有序 boundary 不需要触碰 swapchain。
        if self.rhi_surface_texture.is_some()
            && !self.surface.canvas.soft_has_content
            && self.surface.canvas.pending_native.is_empty()
            && self.surface.pending_clear_rects.is_empty()
        {
            // 后续 RHI Picture blit 可以继续写入同一 retained target。
            return Ok(());
        }
        // 优先把可验证的 native queue 写入 retained target，保持 painter order。
        if self.try_flush_main_segment_rhi()? {
            // 最终 swapchain present 仍由统一帧边界完成。
            return Ok(());
        }
        // legacy boundary 不能与旧 retained 副本并存，先安全丢弃 retained target。
        self.abandon_rhi_surface_texture_for_legacy()?;
        self.gpu_ctx.make_current()?;

        if self.surface.needs_gpu_clear {
            self.gpu_ctx.clear_render_target(0.0, 0.0, 0.0, 0.0)?;
            self.surface.needs_gpu_clear = false;
            self.surface.pending_clear_rects.clear();
        } else if !self.surface.pending_clear_rects.is_empty() {
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

        if let Err(err) = self.surface.canvas.submit_native(self.gpu_ctx.as_mut()) {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }
        if let Err(err) = self.surface.canvas.submit_soft(self.gpu_ctx.as_mut()) {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }
        self.surface.canvas.commit_presented_frame();
        Ok(())
    }

    /// Executes the API-neutral stream at each recorded command boundary.
    /// `Native` maps to the GPU-native DTO, while CPU segments and Picture
    /// blits produce isolated transparent sources and are alpha-uploaded at
    /// their original painter-order position. This deliberately does not
    /// upload a completed `render_reference()` frame.
    pub(super) fn execute_frame_encoder(
        &mut self,
        encoder: &FrameEncoder,
        mut target_initialized: bool,
    ) -> Result<(), Error> {
        for command in encoder.commands() {
            match command {
                FrameCommand::Clear { color } => {
                    self.clear_frame_encoder_target(*color)?;
                    target_initialized = true;
                }
                FrameCommand::Native { operation } => {
                    self.ensure_frame_encoder_target(&mut target_initialized)?;
                    self.execute_native_frame_operation(encoder, operation)?;
                }
                FrameCommand::CpuSegment { image, src, dst } => {
                    self.ensure_frame_encoder_target(&mut target_initialized)?;
                    self.execute_frame_image_blit(
                        encoder,
                        image,
                        *src,
                        crate::draw::painting::FrameSampledRect::from_integer(*dst),
                        1.0,
                        false,
                    )?;
                }
                FrameCommand::PictureBlit {
                    image,
                    src,
                    dst,
                    opacity,
                    additive,
                } => {
                    self.ensure_frame_encoder_target(&mut target_initialized)?;
                    if opacity.is_transparent() {
                        continue;
                    }
                    self.execute_frame_image_blit(
                        encoder,
                        image,
                        *src,
                        *dst,
                        opacity.value(),
                        *additive,
                    )?;
                }
            }
        }
        if !target_initialized {
            self.clear_frame_encoder_target(Color::transparent())?;
        }
        Ok(())
    }

    /// Prepares the retained main target for an encoded partial frame.
    ///
    /// A recording without a leading `Clear` intentionally represents a
    /// partial update. Initializing it through `clear_render_target` would
    /// discard every clean pixel while the encoder only repaints its damage.
    /// Apply the damage clears queued by `begin_frame` and let the native
    /// command stream load the retained target instead.
    pub(super) fn prepare_main_frame_encoder_target(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<bool, Error> {
        if matches!(encoder.commands().first(), Some(FrameCommand::Clear { .. })) {
            return Ok(false);
        }
        if self.surface.needs_gpu_clear {
            self.clear_frame_encoder_target(Color::transparent())?;
            self.surface.needs_gpu_clear = false;
            self.surface.pending_clear_rects.clear();
            return Ok(true);
        }
        if !self.surface.pending_clear_rects.is_empty() {
            self.gpu_ctx.clear_rects(
                self.surface.width as f32,
                self.surface.height as f32,
                &self.surface.pending_clear_rects,
            )?;
            self.surface.pending_clear_rects.clear();
        }
        Ok(true)
    }

    pub(super) fn ensure_frame_encoder_target(
        &mut self,
        target_initialized: &mut bool,
    ) -> Result<(), Error> {
        if !*target_initialized {
            self.clear_frame_encoder_target(Color::transparent())?;
            *target_initialized = true;
        }
        Ok(())
    }

    pub(super) fn clear_frame_encoder_target(&mut self, color: Color) -> Result<(), Error> {
        self.gpu_ctx.clear_render_target(
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        )
    }

    pub(super) fn execute_native_frame_operation(
        &mut self,
        encoder: &FrameEncoder,
        operation: &FrameRasterOp,
    ) -> Result<(), Error> {
        let target_width = encoder.width();
        let target_height = encoder.height();
        match operation {
            FrameRasterOp::FillRect { rect, color } => self.draw_frame_solid_rect(
                target_width,
                target_height,
                *rect,
                *color,
                [0.0; 4],
                None,
            ),
            FrameRasterOp::FillRoundedRect {
                rect,
                color,
                radius,
            } => {
                let radius = radius.to_radius();
                self.draw_frame_solid_rect(
                    target_width,
                    target_height,
                    *rect,
                    *color,
                    [radius.tl, radius.tr, radius.br, radius.bl],
                    None,
                )
            }
            FrameRasterOp::FillRoundedRectClipped {
                rect,
                color,
                radius,
                clip,
            } => {
                let Some(clip) =
                    clip.intersection(FrameRect::new(0, 0, target_width, target_height))
                else {
                    return Ok(());
                };
                let radius = radius.to_radius();
                self.draw_frame_solid_rect(
                    target_width,
                    target_height,
                    *rect,
                    *color,
                    [radius.tl, radius.tr, radius.br, radius.bl],
                    Some(clip),
                )
            }
            FrameRasterOp::BlitGlyphs { glyphs, clip } => {
                self.draw_frame_glyphs(target_width, target_height, glyphs, *clip)
            }
            FrameRasterOp::StrokeRoundedRects { strokes, clip } => {
                let Some(clip) =
                    clip.intersection(FrameRect::new(0, 0, target_width, target_height))
                else {
                    return Ok(());
                };
                if self.surface.native_caps.stroke_rects {
                    return self.draw_frame_stroke_rects(
                        target_width,
                        target_height,
                        strokes,
                        clip,
                    );
                }
                let tile = encoder
                    .stroke_rects_reference_tile(strokes, clip)
                    .map_err(|error| {
                        let code = if matches!(error, FrameEncoderError::CommandAllocationFailed) {
                            Errc::GraphicsOutOfMemory
                        } else {
                            Errc::InvalidState
                        };
                        Error::new(
                            code,
                            format!("could not rasterize compact frame stroke tile: {error}"),
                        )
                    })?;
                if let Some((source, destination)) = tile {
                    self.alpha_blit_frame_encoder_source(&source, destination)?;
                }
                Ok(())
            }
            FrameRasterOp::FillRectAdditive { .. }
            | FrameRasterOp::FillRoundedRectAdditive { .. }
            | FrameRasterOp::ScrollCopy { .. } => {
                self.execute_destination_dependent_frame_op(target_width, target_height, operation)
            }
        }
    }
}
