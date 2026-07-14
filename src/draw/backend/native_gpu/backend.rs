//! Native GPU surface 与后端资源、提交生命周期。

use super::*;

/// DrawSurface for an API-neutral `GpuNative × Swapchain` path.
pub struct NativeGpuDrawSurface {
    pub(crate) canvas: NativeGpuCanvas2D,
    native_caps: NativeRasterCaps,
    pub(crate) width: i32,
    pub(crate) height: i32,
    /// Full clear pending (ClearRenderTargetView).
    pub(crate) needs_gpu_clear: bool,
    /// Partial clear rects (replace-blend quads).
    pub(crate) pending_clear_rects: Vec<GpuSolidRect>,
}

impl DrawSurface for NativeGpuDrawSurface {
    fn size(&self) -> crate::core::Size {
        crate::core::Size::new(self.width as f32, self.height as f32)
    }

    fn push_clip(&mut self, rect: Rect) {
        self.canvas.push_clip(rect);
    }

    fn pop_clip(&mut self) {
        self.canvas.pop_clip();
    }

    fn clear_all(&mut self) {
        self.canvas.clear_soft();
        self.pending_clear_rects.clear();
        self.needs_gpu_clear = true;
    }

    fn clear_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.canvas.clear_soft_rect(x, y, w, h);
        if self.needs_gpu_clear {
            return;
        }
        if !self.native_caps.clear_rects {
            self.pending_clear_rects.clear();
            self.needs_gpu_clear = true;
            return;
        }
        self.pending_clear_rects.push(GpuSolidRect {
            x: x as f32,
            y: y as f32,
            w: w as f32,
            h: h as f32,
            rgba: [0.0, 0.0, 0.0, 0.0],
            radius: [0.0; 4],
        });
    }

    fn copy_region(&mut self, _src: Rect, _dst: Point) {
        self.canvas.reject_unsupported("scroll-region copy");
    }

    fn canvas(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    fn take_deferred_error(&mut self) -> Option<Error> {
        self.canvas.take_deferred_error()
    }
}

/// Capability-driven non-GL `RenderBackend` with deterministic soft fallback.
pub struct NativeGpuBackend {
    pub(crate) gpu_ctx: Box<dyn IGraphicsContext>,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) surface: NativeGpuDrawSurface,
    pub(crate) offscreens: Vec<Option<NativeGpuOffscreen>>,
    pub(crate) free_offscreen_ids: Vec<u32>,
    next_offscreen_id: u32,
    /// Picture paint currently targeting this offscreen handle id.
    pub(crate) active_offscreen: Option<u32>,
    /// A draw-time operation without a `Result` return path (for example an
    /// immediate Picture blit) failed.  The failure is reported from the sole
    /// final present boundary, so callers keep the frame dirty instead of
    /// treating an incomplete command stream as committed.
    frame_failure: Option<Error>,
    present_damage_tracker: PresentDamageTracker,
    pub(crate) shutdown: bool,
}

pub(crate) struct NativeGpuOffscreen {
    target: OffscreenTargetId,
    pub(crate) canvas: NativeGpuCanvas2D,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

/// Canvas coordinates are logical pixels. Native contexts report their
/// drawable extent through `width`/`height`, so derive the matching logical
/// extent from their single DPR source before allocating draw-side state.
fn logical_extent_from_context(gpu_ctx: &dyn IGraphicsContext) -> (i32, i32) {
    let dpr = gpu_ctx.device_pixel_ratio();
    let dpr = if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    };
    let logical = |drawable: i32| ((drawable.max(1) as f32 / dpr).round() as i32).max(1);
    (logical(gpu_ctx.width()), logical(gpu_ctx.height()))
}

impl NativeGpuBackend {
    pub(crate) fn new(mut gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let caps = gpu_ctx.caps();
        let native_caps = gpu_ctx.native_raster_caps();
        if caps.raster != RasterMode::GpuNative
            || caps.present != PresentMode::Swapchain
            || !native_caps.has_hybrid_baseline()
        {
            let backend = caps.backend;
            let raster = caps.raster;
            let present = caps.present;
            gpu_ctx.try_shutdown()?;
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "NativeGpuBackend requires GpuNative × Swapchain plus clear/soft-blit baseline, got {backend} raster={raster} present={present} native={native_caps:?}"
                ),
            ));
        }
        let (logical_w, logical_h) = logical_extent_from_context(gpu_ctx.as_ref());
        Ok(Self {
            gpu_ctx,
            width: logical_w,
            height: logical_h,
            shutdown: false,
            offscreens: Vec::new(),
            free_offscreen_ids: Vec::new(),
            next_offscreen_id: 0,
            active_offscreen: None,
            frame_failure: None,
            present_damage_tracker: PresentDamageTracker::new(),
            surface: NativeGpuDrawSurface {
                canvas: NativeGpuCanvas2D::new(logical_w, logical_h, native_caps),
                native_caps,
                width: logical_w,
                height: logical_h,
                needs_gpu_clear: true,
                pending_clear_rects: Vec::new(),
            },
        })
    }

    /// Flushes the current ordered segment without presenting, then reads the
    /// native drawable. This is a crate-local diagnostic/test boundary; it
    /// deliberately uses the same command ordering as a final present.
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

    #[cfg(all(test, any(feature = "opengles", feature = "d3d11", feature = "d3d12")))]
    pub(crate) fn last_soft_upload_bytes(&self) -> usize {
        self.surface.canvas.last_soft_upload_bytes
    }

    fn destroy_all_offscreens(&mut self) -> Result<(), Error> {
        self.active_offscreen = None;
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

    fn remember_frame_failure(&mut self, error: Error) {
        if self.frame_failure.is_none() {
            self.frame_failure = Some(error);
        }
        // Commands before an immediate boundary may already have reached the
        // target.  The next retained-dirty retry must start from a known full
        // clear rather than alpha-blending on that partial target.
        self.surface.needs_gpu_clear = true;
    }

    fn adopt_factory_drawable_extent(&mut self) -> (i32, i32) {
        let (logical_w, logical_h) = logical_extent_from_context(self.gpu_ctx.as_ref());
        self.width = logical_w;
        self.height = logical_h;
        self.surface.width = logical_w;
        self.surface.height = logical_h;
        self.surface.canvas.resize(logical_w, logical_h);
        self.surface.needs_gpu_clear = true;
        self.surface.pending_clear_rects.clear();
        (logical_w, logical_h)
    }

    /// Submit all commands that precede an immediate ordered operation such
    /// as a Picture/offscreen blit.  This is deliberately *not* a present:
    /// it only establishes the exact painter-order boundary inside the one
    /// frame and leaves final swap/present to [`RenderBackend::present`].
    fn flush_main_segment_before_ordered_boundary(&mut self) -> Result<(), Error> {
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
    fn execute_frame_encoder(&mut self, encoder: &FrameEncoder) -> Result<(), Error> {
        let mut target_initialized = false;
        for command in encoder.commands() {
            match command {
                FrameCommand::Clear { color } => {
                    self.clear_frame_encoder_target(*color)?;
                    target_initialized = true;
                }
                FrameCommand::Native { operation } => {
                    self.ensure_frame_encoder_target(&mut target_initialized)?;
                    self.execute_native_frame_operation(
                        encoder.width(),
                        encoder.height(),
                        operation,
                    )?;
                }
                FrameCommand::CpuSegment { image, src, dst } => {
                    self.ensure_frame_encoder_target(&mut target_initialized)?;
                    let source = encoder.cpu_segment_reference(image, *src, *dst);
                    self.alpha_blit_frame_encoder_source(&source)?;
                }
                FrameCommand::PictureBlit { image, src, dst } => {
                    self.ensure_frame_encoder_target(&mut target_initialized)?;
                    let source = encoder.picture_blit_reference(image, *src, *dst);
                    self.alpha_blit_frame_encoder_source(&source)?;
                }
            }
        }
        if !target_initialized {
            self.clear_frame_encoder_target(Color::transparent())?;
        }
        Ok(())
    }

    fn ensure_frame_encoder_target(&mut self, target_initialized: &mut bool) -> Result<(), Error> {
        if !*target_initialized {
            self.clear_frame_encoder_target(Color::transparent())?;
            *target_initialized = true;
        }
        Ok(())
    }

    fn clear_frame_encoder_target(&mut self, color: Color) -> Result<(), Error> {
        self.gpu_ctx.clear_render_target(
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            color.a as f32 / 255.0,
        )
    }

    fn execute_native_frame_operation(
        &mut self,
        target_width: i32,
        target_height: i32,
        operation: &FrameRasterOp,
    ) -> Result<(), Error> {
        match operation {
            FrameRasterOp::FillRect { rect, color } => {
                if rect.width <= 0 || rect.height <= 0 {
                    return Ok(());
                }
                self.gpu_ctx.draw_solid_rects(
                    target_width as f32,
                    target_height as f32,
                    None,
                    &[GpuSolidRect {
                        x: rect.x as f32,
                        y: rect.y as f32,
                        w: rect.width as f32,
                        h: rect.height as f32,
                        rgba: [
                            color.r as f32 / 255.0,
                            color.g as f32 / 255.0,
                            color.b as f32 / 255.0,
                            color.a as f32 / 255.0,
                        ],
                        radius: [0.0; 4],
                    }],
                )
            }
            FrameRasterOp::FillRectAdditive { .. } | FrameRasterOp::ScrollCopy { .. } => {
                self.execute_destination_dependent_frame_op(target_width, target_height, operation)
            }
        }
    }

    /// Destination-dependent IR ops cannot be drawn with SrcOver GPU quads.
    /// When the context supports readback + full upload, apply the reference
    /// semantics on CPU pixels and replace the RT — pixel-correct, not an
    /// alpha-over approximation. Otherwise return typed NotImplemented.
    fn execute_destination_dependent_frame_op(
        &mut self,
        target_width: i32,
        target_height: i32,
        operation: &FrameRasterOp,
    ) -> Result<(), Error> {
        let expected = (target_width as usize).saturating_mul(target_height as usize);
        let mut pixels = self
            .gpu_ctx
            .read_pixels(0, 0, target_width, target_height)
            .map_err(|error| {
                if error.code() == Errc::NotImplemented {
                    Error::new(
                        Errc::NotImplemented,
                        "NativeGpuBackend: destination-dependent FrameRasterOp requires readback",
                    )
                    .with_source(error)
                } else {
                    error
                }
            })?;
        if pixels.len() != expected {
            return Err(Error::new(
                Errc::InvalidState,
                format!(
                    "NativeGpuBackend: destination-dependent FrameRasterOp readback extent mismatch (got {}, expected {expected})",
                    pixels.len()
                ),
            ));
        }
        crate::draw::pipeline::frame_encoder::apply_frame_raster_op(
            target_width,
            target_height,
            &mut pixels,
            operation,
        );
        self.gpu_ctx
            .upload_surface_pixels(&pixels, target_width, target_height)
            .map_err(|error| {
                if error.code() == Errc::NotImplemented {
                    Error::new(
                        Errc::NotImplemented,
                        "NativeGpuBackend: destination-dependent FrameRasterOp requires replace upload",
                    )
                    .with_source(error)
                } else {
                    error
                }
            })
    }

    fn alpha_blit_frame_encoder_source(&mut self, source: &ReferenceFrame) -> Result<(), Error> {
        if let Some((pixels, tile)) =
            pack_visible_soft_fallback_tile(source.pixels(), source.width(), source.height())
        {
            self.gpu_ctx.blit_soft_fallback_tile(&pixels, tile)?;
        }
        Ok(())
    }
}

impl RenderBackend for NativeGpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        // Swapchain image repair is planned at the final present boundary.
        // Until acquisition moves before draw, render the complete GPU frame;
        // narrow compositor damage remains available without stale pixels.
        let mut caps = BackendCapabilities::gpu_full_redraw();
        caps.offscreen = self.surface.native_caps.offscreen_targets;
        caps
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let logical_w = width.max(1);
        let logical_h = height.max(1);
        self.gpu_ctx.resize(logical_w, logical_h)?;
        // D3D11/D3D12 等会按 HWND GetClientRect 校正缓冲尺寸；canvas/布局必须跟
        // 实际 RT 一致，否则清出更大黑底而 UI 仍画旧几何 → 窗口黑边。
        self.adopt_factory_drawable_extent();
        Ok(())
    }

    fn initialize_prepared(&mut self, _width: i32, _height: i32) -> Result<(i32, i32), Error> {
        // `IGraphicsContext::initialize` already ran in the factory against
        // the real surface. Startup only synchronizes draw-owned state to the
        // factory-reported drawable; it must not recreate the swapchain.
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
                crate::core::log::warn_fn(format!(
                    "NativeGpuBackend: create_offscreen_target failed: {}",
                    err.short_what()
                ));
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
            canvas: NativeGpuCanvas2D::new(width, height, self.surface.native_caps),
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
        }
        self.free_offscreen_ids.push(handle.0);
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
        self.execute_frame_encoder(encoder)?;
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
            self.execute_frame_encoder(encoder)
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
        Ok(())
    }

    fn end_offscreen_paint(&mut self) {
        self.active_offscreen = None;
        if let Err(error) = self.try_end_offscreen_paint() {
            self.remember_frame_failure(error);
        }
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.active_offscreen = None;
        self.gpu_ctx.bind_swapchain_target()
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
        self.gpu_ctx
            .blit_offscreen_target(target, src_rect, dst_rect)
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
        if let Err(err) = self.gpu_ctx.present(&frame) {
            self.surface.needs_gpu_clear = true;
            return Err(err);
        }
        self.present_damage_tracker.commit(
            caps.present_coherency,
            present_surface,
            present_image,
            damage,
        );
        self.surface.canvas.commit_presented_frame();
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

impl Drop for NativeGpuBackend {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            crate::core::log::error_fn(format!(
                "NativeGpuBackend: checked shutdown failed: {}",
                error.short_what()
            ));
        }
    }
}
