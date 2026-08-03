//! [`GpuBackend`] 的 `RenderBackend` 实现 — backend 子模块。
//!
//! offscreen 生命周期、编码 Picture/Frame 执行、叠加层 backdrop 快照。

use std::any::Any;
use std::time::Instant;

use crate::core::{DamageRegion, Errc, Error, Rect};
use crate::draw::backend::contract::{
    BackendCapabilities, BackendKind, DrawSurface, RenderBackend,
};
use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::{BlendMode, ImageHandle};
use crate::draw::painting::{
    EncodedFrameExecution, EncodedPictureExecution, FrameEncoder, FrameEncoderError,
};
use crate::draw::Canvas2D;
use crate::native::present::{
    IGraphicsContext, OffscreenTargetId, PresentFrame, PresentTestResult,
};

use super::super::canvas::NativeGpuCanvas2D;
use super::super::pending::PendingNativeOp;
use super::{GpuBackend, NativeGpuOffscreen};

impl RenderBackend for GpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        // 保留主色缓冲证明绘制侧可局部更新；present 仍 FullOnly（全幅 blit）。
        // 未声明 retained_framebuffer 的后端继续全帧绘制，避免 swapchain 未定义像素。
        let mut caps = if self.surface.native_caps.retained_framebuffer {
            BackendCapabilities::gpu_with_offscreen()
        } else {
            BackendCapabilities::gpu_full_redraw()
        };
        caps.offscreen = self.surface.native_caps.offscreen_targets;
        caps
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let logical_w = width.max(1);
        let logical_h = height.max(1);
        self.gpu_ctx.resize(logical_w, logical_h)?;
        // D3D11/D3D12 等会按 HWND GetClientRect 校正缓冲尺寸；canvas/布局必须跟
        // 实际 RT 一致，否则清出更大黑底而 UI 仍画旧几何 → 窗口黑边。
        self.factory_prepared = true;
        self.adopt_factory_drawable_extent();
        Ok(())
    }

    fn initialize_prepared(&mut self, width: i32, height: i32) -> Result<(i32, i32), Error> {
        // `IGraphicsContext::initialize` already ran in the factory against
        // the real surface. Startup only synchronizes draw-owned state to the
        // factory-reported drawable; it must not recreate the swapchain.
        if !self.factory_prepared {
            self.gpu_ctx.resize(width.max(1), height.max(1))?;
            self.factory_prepared = true;
        }
        Ok(self.adopt_factory_drawable_extent())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        if self.shutdown {
            return Ok(());
        }
        self.destroy_all_offscreens()?;
        self.gpu_ctx.try_shutdown()?;
        self.shutdown = true;
        Ok(())
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.surface
    }

    fn make_current(&mut self) -> Result<(), Error> {
        self.gpu_ctx.make_current()
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.gpu_ctx.device_pixel_ratio()
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        if !self.surface.native_caps.offscreen_targets || width <= 0 || height <= 0 {
            return None;
        }
        let target = self
            .gpu_ctx
            .create_offscreen_target(width, height)
            .inspect_err(|err| {
                tracing::warn!(
                    "GpuBackend: create_offscreen_target failed: {}",
                    err.short_what()
                );
            })
            .ok()?;
        let id = if let Some(id) = self.free_offscreen_ids.pop() {
            id
        } else {
            let id = self.next_offscreen_id;
            self.next_offscreen_id = self.next_offscreen_id.saturating_add(1);
            id
        };
        let idx = id as usize;
        while self.offscreens.len() <= idx {
            self.offscreens.push(None);
        }
        self.offscreens[idx] = Some(NativeGpuOffscreen {
            target,
            canvas: if self.gpu_only {
                NativeGpuCanvas2D::new_gpu_only(width, height, self.surface.native_caps)
            } else {
                NativeGpuCanvas2D::new(width, height, self.surface.native_caps)
            },
            width,
            height,
        });
        Some(ImageHandle(id))
    }

    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        let idx = handle.0 as usize;
        let Some(off) = self.offscreens.get(idx).and_then(Option::as_ref) else {
            return Ok(());
        };
        if self.active_offscreen == Some(handle.0) {
            self.gpu_ctx.bind_swapchain_target()?;
        }
        self.gpu_ctx.try_destroy_offscreen_target(off.target)?;
        self.offscreens[idx] = None;
        if self.active_offscreen == Some(handle.0) {
            self.active_offscreen = None;
            self.offscreen_flush_committed = false;
        }
        self.free_offscreen_ids.push(handle.0);
        self.compact_offscreen_slots();
        Ok(())
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        if let Err(error) = self.try_destroy_offscreen(handle) {
            self.remember_frame_failure(error);
        }
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let idx = handle.0 as usize;
        self.offscreens
            .get_mut(idx)?
            .as_mut()
            .map(|o| &mut o.canvas as &mut dyn Canvas2D)
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        if self.active_offscreen != Some(handle.0) {
            return Err(Error::new(
                Errc::InvalidState,
                "FrameEncoder Picture execution requires its bound offscreen target",
            ));
        }
        let target = self
            .offscreens
            .get(handle.0 as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "Picture offscreen target disappeared before FrameEncoder execution",
                )
            })?;
        if (target.width, target.height) != (encoder.width(), encoder.height()) {
            return Err(Error::new(
                Errc::InvalidState,
                format!(
                    "FrameEncoder {}x{} does not match Picture target {}x{}",
                    encoder.width(),
                    encoder.height(),
                    target.width,
                    target.height
                ),
            ));
        }
        let target = target.target;

        self.gpu_ctx.bind_offscreen_target(target)?;
        self.execute_frame_encoder(encoder, false)?;
        Ok(EncodedPictureExecution::Executed)
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        if self.active_offscreen.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "main FrameEncoder execution cannot run while a Picture target is bound",
            ));
        }
        if (self.width, self.height) != (encoder.width(), encoder.height()) {
            return Err(Error::new(
                Errc::InvalidState,
                format!(
                    "FrameEncoder {}x{} does not match main native target {}x{}",
                    encoder.width(),
                    encoder.height(),
                    self.width,
                    self.height
                ),
            ));
        }

        let execute = (|| {
            self.gpu_ctx.make_current()?;
            self.gpu_ctx.bind_swapchain_target()?;
            let target_initialized = self.prepare_main_frame_encoder_target(encoder)?;
            self.execute_frame_encoder(encoder, target_initialized)
        })();
        if let Err(error) = execute {
            self.surface.needs_gpu_clear = true;
            return Err(error);
        }

        // `begin_frame` may have prepared a clear or retained Canvas2D state.
        // The FrameEncoder has replaced the target, so final `present` must
        // not submit a second clear/draw sequence over it.
        self.surface.needs_gpu_clear = false;
        self.surface.pending_clear_rects.clear();
        self.surface.canvas.commit_presented_frame();
        Ok(EncodedFrameExecution::Executed)
    }

    fn begin_offscreen_paint(&mut self, handle: &ImageHandle) -> bool {
        match self.try_begin_offscreen_paint(handle) {
            Ok(()) => true,
            Err(error) => {
                self.remember_frame_failure(error);
                false
            }
        }
    }

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.offscreen_flush_committed = false;
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen target does not exist",
            ));
        };
        let target = off.target;
        self.gpu_ctx.bind_offscreen_target(target)?;
        if let Err(error) = self.gpu_ctx.clear_render_target(0.0, 0.0, 0.0, 0.0) {
            return match self.gpu_ctx.bind_swapchain_target() {
                Ok(()) => Err(error),
                Err(restore_error) => Err(restore_error.with_source(error)),
            };
        }
        if let Some(Some(off)) = self.offscreens.get_mut(idx) {
            off.canvas.reset_for_repaint();
        }
        self.active_offscreen = Some(handle.0);
        Ok(())
    }

    fn flush_offscreen_paint(&mut self, handle: &ImageHandle) {
        if let Err(error) = self.try_flush_offscreen_paint(handle) {
            // The legacy void entry remains for old callers.  The production
            // compositor uses `try_*` and therefore returns this failure
            // before a final present can be reported as success.
            self.remember_frame_failure(error);
        }
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        if self.active_offscreen == Some(handle.0) {
            self.offscreen_flush_committed = false;
        }
        let idx = handle.0 as usize;
        let off = self
            .offscreens
            .get_mut(idx)
            .and_then(Option::as_mut)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "offscreen target disappeared before flush",
                )
            })?;
        if let Some(error) = off.canvas.take_deferred_error() {
            return Err(error);
        }
        let target = off.target;
        self.gpu_ctx.bind_offscreen_target(target)?;
        off.canvas.submit_native(self.gpu_ctx.as_mut())?;
        off.canvas.submit_soft(self.gpu_ctx.as_mut())?;
        off.canvas.commit_presented_frame();
        if self.active_offscreen == Some(handle.0) {
            self.offscreen_flush_committed = true;
        }
        Ok(())
    }

    fn end_offscreen_paint(&mut self) {
        if let Err(error) = self.try_end_offscreen_paint() {
            self.remember_frame_failure(error);
        }
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        let active = self.active_offscreen.take();
        let restore = self.gpu_ctx.bind_swapchain_target();
        if restore.is_ok() && self.offscreen_flush_committed {
            if let Some(offscreen) = active
                .and_then(|id| self.offscreens.get_mut(id as usize))
                .and_then(Option::as_mut)
            {
                offscreen.canvas.release_committed_picture_staging();
            }
        }
        self.offscreen_flush_committed = false;
        restore
    }

    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        let Some(Some(off)) = self.offscreens.get(handle.0 as usize) else {
            return;
        };
        let src = Rect::new(0.0, 0.0, off.width as f32, off.height as f32);
        self.blit_offscreen_src(handle, src, dst_rect);
    }

    fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        if let Err(error) = self.try_blit_offscreen_src(handle, src_rect, dst_rect) {
            self.remember_frame_failure(error);
        }
    }

    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen target does not exist before blit",
            ));
        };
        let target = off.target;
        let opacity = if let Some(active) = self.active_offscreen {
            self.offscreens
                .get(active as usize)
                .and_then(|slot| slot.as_ref())
                .map(|slot| slot.canvas.opacity())
                .unwrap_or(1.0)
        } else {
            self.surface.canvas.opacity()
        };
        let additive = if let Some(active) = self.active_offscreen {
            self.offscreens
                .get(active as usize)
                .and_then(|slot| slot.as_ref())
                .map(|slot| matches!(slot.canvas.current_blend_mode(), BlendMode::Additive))
                .unwrap_or(false)
        } else {
            matches!(
                self.surface.canvas.current_blend_mode(),
                BlendMode::Additive
            )
        };
        if !opacity.is_finite() || opacity <= 0.0 {
            return Ok(());
        }
        if let Some(active) = self.active_offscreen {
            if active == handle.0 {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "Picture offscreen target cannot blit into itself",
                ));
            }
            // The destination is the currently bound Picture target. Submit
            // its queued native/soft commands before the immediate source
            // blit so painter order remains destination commands → blit.
            // Flushing the main swapchain here would clear/submit the wrong
            // target and invert that order.
            self.try_flush_offscreen_paint(&ImageHandle(active))?;
        } else {
            // `blit_offscreen_target` is immediate on native APIs. Flush
            // clear, native work and any bounded CPU segment before it so
            // Picture does not leapfrog preceding painter-order commands.
            // The subsequent commands remain queued and are committed by the
            // same final present.
            self.flush_main_segment_before_ordered_boundary()?;
        }
        self.gpu_ctx.blit_offscreen_target(
            target,
            src_rect,
            dst_rect,
            opacity.clamp(0.0, 1.0),
            additive,
        )
    }

    fn try_blur_offscreen(
        &mut self,
        handle: &ImageHandle,
        region: Rect,
        radius: f32,
    ) -> Result<(), Error> {
        if !self.surface.native_caps.offscreen_targets {
            return Err(Error::new(
                Errc::NotImplemented,
                "native GPU backend lacks offscreen targets required for separable blur",
            ));
        }
        if !radius.is_finite() || radius < 0.5 {
            return Ok(());
        }
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen target does not exist before blur",
            ));
        };
        let target = off.target;
        // 模糊前必须把挂起的绘制落到纹理，且不能在绑定为目标时采样。
        if self.active_offscreen == Some(handle.0) {
            self.try_flush_offscreen_paint(handle)?;
            self.try_end_offscreen_paint()?;
        } else if self.active_offscreen.is_some() {
            // 另一离屏正绑定：先 flush 当前绑定，避免命令落错目标。
            if let Some(active) = self.active_offscreen {
                self.try_flush_offscreen_paint(&ImageHandle(active))?;
            }
        } else {
            self.flush_main_segment_before_ordered_boundary()?;
        }
        self.gpu_ctx.blur_offscreen_target(target, region, radius)
    }

    fn snapshot_overlay_backdrop(&mut self) -> bool {
        if !self.surface.native_caps.retained_framebuffer {
            return false;
        }
        if let Err(err) = self.gpu_ctx.make_current() {
            tracing::warn!(
                "GpuBackend: snapshot_overlay_backdrop make_current failed: {}",
                err.short_what()
            );
            return false;
        }
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

    fn restore_overlay_backdrop(&mut self) -> bool {
        if !self.gpu_ctx.has_overlay_backdrop() {
            return false;
        }
        if let Err(err) = self.gpu_ctx.make_current() {
            tracing::warn!(
                "GpuBackend: restore_overlay_backdrop make_current failed: {}",
                err.short_what()
            );
            return false;
        }
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

    fn release_overlay_backdrop(&mut self) {
        self.gpu_ctx.release_overlay_backdrop();
    }

    fn has_overlay_backdrop(&self) -> bool {
        self.gpu_ctx.has_overlay_backdrop()
    }

    fn present(&mut self, damage: &DamageRegion) -> Result<(), Error> {
        if self.active_offscreen.is_some() {
            self.end_offscreen_paint();
        }
        if let Some(error) = self.surface.canvas.take_deferred_error() {
            self.surface.needs_gpu_clear = true;
            return Err(error);
        }
        if let Some(error) = self.frame_failure.take() {
            self.surface.needs_gpu_clear = true;
            return Err(error);
        }
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

        let caps = self.gpu_ctx.caps();
        let present_surface = self.gpu_ctx.present_surface();
        let present_image = self.gpu_ctx.present_image();
        let damage_plan = self.present_damage_tracker.plan(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        let frame = PresentFrame::Swapchain {
            damage: damage_plan.present_damage,
        };
        let present_t0 = std::time::Instant::now();
        let present_result = self.gpu_ctx.present(&frame);
        let mut present_sample = crate::core::perf_probe::take_present();
        present_sample.present_us = present_t0.elapsed().as_micros();
        crate::core::perf_probe::record_present(present_sample);
        if let Err(err) = present_result {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }
        self.present_damage_tracker.commit(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        let mut used_soft = self.surface.canvas.finish_presented_frame();
        for off in self.offscreens.iter_mut().flatten() {
            used_soft |= off.canvas.age_soft_fallback_after_present();
        }
        self.soft_used_in_last_present = used_soft;
        if !self.has_soft_fallback_allocation() {
            self.soft_fallback_idle_deadline = None;
        }
        Ok(())
    }

    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        self.gpu_ctx.test_present()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
