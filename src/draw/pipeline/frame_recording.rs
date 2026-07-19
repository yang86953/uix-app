//! Private, API-neutral producer for one ordered main [`FrameEncoder`].
//!
//! The compositor paints into this engine instead of a real presentation
//! surface. Every visual operation is either lowered to a proven native op or
//! rasterized into one transparent CPU segment. Picture offscreens retain the
//! same API-neutral stream while it stays within the former BGRA memory budget;
//! unsafe mappings materialize that stream lazily as one ordered image blit.

use crate::core::{DamageRegion, Errc, Error, Rect};
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::engine::RenderOutcome;
use crate::draw::pipeline::{
    EncodedFrameExecution, EncodedPictureExecution, FrameEncoder, FrameEncoderError,
    FrameGlyphBlit, FrameImage, FrameOpacity, FrameRadius, FrameRasterOp, FrameRect,
    FrameStrokeRect, FrameStrokeWidth,
};
use crate::draw::primitives::path::{FillRule, Path};
use crate::draw::primitives::stroker::StrokeOptions;
use crate::draw::primitives::types::{
    BlendMode, GradientDirection, ImageHandle, Radius, Transform,
};
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::draw::Color;
use std::sync::Arc;

/// The only producer used by [`super::render_frame::FrameRenderer`]. It owns
/// no API object and cannot present; its output is consumed exactly once by
/// the caller's real graphics engine.
pub(crate) struct FrameRecordingEngine {
    canvas: FrameRecordingCanvas,
    offscreens: RecordedPicturePool,
    active_offscreen: Option<ActiveOffscreen>,
}

#[derive(Clone, Copy)]
struct ActiveOffscreen {
    handle: ImageHandle,
    failed: bool,
}

enum RecordedPicturePayload {
    Encoder(FrameEncoder),
    Image(FrameImage),
}

struct RecordedPicture {
    canvas: FrameRecordingCanvas,
    committed: Option<RecordedPicturePayload>,
}

impl RecordedPicture {
    fn new(width: i32, height: i32) -> Self {
        Self {
            canvas: FrameRecordingCanvas::new(width, height),
            committed: None,
        }
    }

    fn commit(&mut self, encoder: FrameEncoder) {
        let pixel_budget = (encoder.width() as usize)
            .saturating_mul(encoder.height() as usize)
            .saturating_mul(std::mem::size_of::<u32>());
        self.committed = Some(if encoder.retained_memory_usage() <= pixel_budget {
            RecordedPicturePayload::Encoder(encoder)
        } else {
            RecordedPicturePayload::Image(encoder.render_image())
        });
    }

    fn copy_pixels(&self) -> Option<(Vec<u32>, i32)> {
        match self.committed.as_ref()? {
            RecordedPicturePayload::Encoder(encoder) => {
                let image = encoder.render_image();
                Some((image.pixels().to_vec(), image.width()))
            }
            RecordedPicturePayload::Image(image) => Some((image.pixels().to_vec(), image.width())),
        }
    }

    fn materialized_image(&mut self) -> Option<FrameImage> {
        let payload = self.committed.take()?;
        let image = match payload {
            RecordedPicturePayload::Encoder(encoder) => encoder.render_image(),
            RecordedPicturePayload::Image(image) => image,
        };
        self.committed = Some(RecordedPicturePayload::Image(image.clone()));
        Some(image)
    }

    fn memory_usage(&self) -> usize {
        let working = self.canvas.retained_memory_usage();
        working.saturating_add(match &self.committed {
            Some(RecordedPicturePayload::Encoder(encoder)) => encoder.retained_memory_usage(),
            Some(RecordedPicturePayload::Image(image)) => image
                .pixels()
                .len()
                .saturating_mul(std::mem::size_of::<u32>()),
            None => 0,
        })
    }
}

#[derive(Default)]
struct RecordedPicturePool {
    slots: Vec<Option<RecordedPicture>>,
    free_ids: Vec<u32>,
    next_id: u32,
}

impl RecordedPicturePool {
    fn new() -> Self {
        Self::default()
    }

    fn clear(&mut self) {
        self.slots.clear();
        self.free_ids.clear();
        self.next_id = 0;
    }

    fn create(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        let (width, height) = FrameRecordingCanvas::prepare_resize(width, height).ok()?;
        let id = self.free_ids.pop().unwrap_or_else(|| {
            let id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            id
        });
        let index = id as usize;
        while self.slots.len() <= index {
            self.slots.push(None);
        }
        self.slots[index] = Some(RecordedPicture::new(width, height));
        Some(ImageHandle(id))
    }

    fn destroy(&mut self, handle: ImageHandle) {
        let index = handle.0 as usize;
        if index < self.slots.len() && self.slots[index].take().is_some() {
            self.free_ids.push(handle.0);
        }
    }

    fn get(&self, handle: &ImageHandle) -> Option<&RecordedPicture> {
        self.slots.get(handle.0 as usize)?.as_ref()
    }

    fn get_mut(&mut self, handle: &ImageHandle) -> Option<&mut RecordedPicture> {
        self.slots.get_mut(handle.0 as usize)?.as_mut()
    }

    fn compact(&mut self) {
        while self.slots.last().is_some_and(Option::is_none) {
            self.slots.pop();
        }
        self.free_ids.retain(|id| (*id as usize) < self.slots.len());
        self.next_id = self.slots.len() as u32;
    }

    fn memory_usage(&self) -> usize {
        self.slots
            .iter()
            .filter_map(Option::as_ref)
            .map(RecordedPicture::memory_usage)
            .fold(0usize, usize::saturating_add)
    }
}

impl Default for FrameRecordingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameRecordingEngine {
    pub(crate) fn new() -> Self {
        Self {
            canvas: FrameRecordingCanvas::new(1, 1),
            offscreens: RecordedPicturePool::new(),
            active_offscreen: None,
        }
    }

    /// Start a new main-frame recording.
    ///
    /// `clear_target`: full frames emit an initial Clear so
    /// `execute_into_pixels` replaces the whole CPU/GPU target. Dirty frames
    /// omit it — the real surface already cleared only the damage AABB, and
    /// undamaged pixels must survive.
    pub(crate) fn begin_recording(&mut self, clear_target: bool) -> Result<(), Error> {
        if self.active_offscreen.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "main FrameEncoder recording cannot begin while a Picture target is active",
            ));
        }
        self.canvas.begin_recording(clear_target)
    }

    pub(crate) fn finish_recording(&mut self) -> Result<FrameEncoder, Error> {
        if self.active_offscreen.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "FrameEncoder recording ended while a Picture target was still active",
            ));
        }
        self.canvas.finish_recording()
    }

    /// Restores one immutable retained main-surface snapshot at the start of
    /// an otherwise full frame. The image is recorded in painter order so
    /// subsequent root-level overlays compose over the clean backdrop.
    pub(crate) fn record_main_image(&mut self, image: FrameImage) -> Result<(), Error> {
        if self.active_offscreen.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "main-surface image cannot be recorded while a Picture target is active",
            ));
        }
        let rect = Rect::new(0.0, 0.0, image.width() as f32, image.height() as f32);
        self.canvas.record_picture_blit(image, rect, rect)
    }

    #[cfg(test)]
    pub(crate) fn scratch_surface_size(&self) -> (i32, i32) {
        (
            self.canvas.scratch.surface().width(),
            self.canvas.scratch.surface().height(),
        )
    }

    #[cfg(test)]
    pub(crate) fn offscreen_scratch_surface_size(
        &self,
        handle: &ImageHandle,
    ) -> Option<(i32, i32)> {
        let picture = self.offscreens.get(handle)?;
        Some((
            picture.canvas.scratch.surface().width(),
            picture.canvas.scratch.surface().height(),
        ))
    }

    fn record_main_picture_blit(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        if FrameOpacity::from_canvas(self.canvas.scratch.opacity()).is_transparent() {
            return Ok(());
        }
        if let Some(commands) =
            self.translated_picture_commands(handle, src_rect, dst_rect, &self.canvas)
        {
            return self.canvas.record_validated_commands(commands);
        }
        let image = self
            .offscreens
            .get_mut(handle)
            .and_then(RecordedPicture::materialized_image)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "Picture offscreen content disappeared before FrameEncoder recording",
                )
            })?;
        self.canvas.record_picture_blit(image, src_rect, dst_rect)
    }

    fn translated_picture_commands(
        &self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
        target: &FrameRecordingCanvas,
    ) -> Option<Vec<crate::draw::pipeline::FrameCommand>> {
        let picture = self.offscreens.get(handle)?;
        let RecordedPicturePayload::Encoder(encoder) = picture.committed.as_ref()? else {
            return None;
        };
        let (src, dst) = target.direct_picture_rects(src_rect, dst_rect)?;
        if !src.is_within(encoder.width(), encoder.height())
            || dst.width != src.width
            || dst.height != src.height
        {
            return None;
        }
        let dx = dst.x.checked_sub(src.x)?;
        let dy = dst.y.checked_sub(src.y)?;
        encoder.translated_source_over_commands_in(src, dx, dy, target.width, target.height)
    }

    fn mark_active_failed(&mut self) {
        if let Some(active) = self.active_offscreen.as_mut() {
            active.failed = true;
        }
    }
}

impl GraphicsEngine for FrameRecordingEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.resize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.active_offscreen = None;
        self.offscreens.clear();
        self.canvas.commit_resize(1, 1);
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let (width, height) = FrameRecordingCanvas::prepare_resize(width, height)?;
        self.canvas.commit_resize(width, height);
        Ok(())
    }

    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        RenderOutcome::FrameReady(DamageRegion::full())
    }

    fn end_frame(&mut self, _present_damage: &DamageRegion) -> RenderOutcome {
        RenderOutcome::Failed(crate::draw::engine::GraphicsFailure::from_error(
            Error::new(
                Errc::InvalidState,
                "FrameRecordingEngine cannot present a frame",
            ),
        ))
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        GraphicsCapabilities::engine_managed_with_offscreen()
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        self.offscreens.create(width, height)
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        if self
            .active_offscreen
            .is_some_and(|active| active.handle == handle)
        {
            self.active_offscreen = None;
        }
        self.offscreens.destroy(handle);
        self.offscreens.compact();
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.offscreens
            .get_mut(handle)
            .map(|picture| &mut picture.canvas as &mut dyn Canvas2D)
    }

    fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.offscreens.get(handle)?.copy_pixels()
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        let result = (|| {
            let active = self.active_offscreen.ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "Picture encoder execution requires an active Picture target",
                )
            })?;
            if active.handle != *handle {
                return Err(Error::new(
                    Errc::InvalidState,
                    "Picture encoder target does not match the active Picture",
                ));
            }
            let target = self.offscreens.get_mut(handle).ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "Picture offscreen target disappeared before FrameEncoder execution",
                )
            })?;
            if target.canvas.width != encoder.width() || target.canvas.height != encoder.height() {
                return Err(Error::new(
                    Errc::InvalidState,
                    format!(
                        "FrameEncoder {}x{} does not match Picture target {}x{}",
                        encoder.width(),
                        encoder.height(),
                        target.canvas.width,
                        target.canvas.height
                    ),
                ));
            }
            let commands = encoder
                .translated_source_over_commands(0, 0, target.canvas.width, target.canvas.height)
                .unwrap_or_else(|| {
                    let full = FrameRect::new(0, 0, encoder.width(), encoder.height());
                    vec![crate::draw::pipeline::FrameCommand::PictureBlit {
                        image: encoder.render_image(),
                        src: full,
                        dst: full,
                        opacity: crate::draw::pipeline::FrameOpacity::opaque(),
                    }]
                });
            target.canvas.record_validated_commands(commands)?;
            Ok(EncodedPictureExecution::Executed)
        })();
        if result.is_err() {
            self.mark_active_failed();
        }
        result
    }

    fn try_execute_encoded_frame(
        &mut self,
        _encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        Err(Error::new(
            Errc::InvalidState,
            "FrameRecordingEngine cannot execute a final frame",
        ))
    }

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        if self.active_offscreen.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "a Picture target is already active",
            ));
        }
        let picture = self.offscreens.get_mut(handle).ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "Picture offscreen target does not exist",
            )
        })?;
        picture.canvas.begin_recording(true)?;
        self.active_offscreen = Some(ActiveOffscreen {
            handle: *handle,
            failed: false,
        });
        Ok(())
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        let result = (|| {
            let active = self.active_offscreen.ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "no Picture target is active before flush",
                )
            })?;
            if active.handle != *handle {
                return Err(Error::new(
                    Errc::InvalidState,
                    "Picture flush target does not match the active Picture",
                ));
            }
            let picture = self.offscreens.get_mut(handle).ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "Picture offscreen target disappeared before flush",
                )
            })?;
            picture.canvas.flush_recording()
        })();
        if result.is_err() {
            self.mark_active_failed();
        }
        result
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        let active = self.active_offscreen.take();
        let Some(active) = active else {
            return Ok(());
        };
        let picture = self.offscreens.get_mut(&active.handle).ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "active Picture target disappeared before end",
            )
        })?;
        if active.failed {
            picture.canvas.abandon_recording();
            return Ok(());
        }
        match picture.canvas.finish_recording() {
            Ok(encoder) => {
                picture.canvas.release_scratch_allocation();
                picture.commit(encoder);
                Ok(())
            }
            Err(error) => {
                picture.canvas.abandon_recording();
                Err(error)
            }
        }
    }

    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        if self.offscreens.get(handle).is_none() {
            self.mark_active_failed();
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen target does not exist before blit",
            ));
        }
        if let Some(active) = self.active_offscreen {
            let dst_handle = active.handle;
            if dst_handle == *handle {
                self.mark_active_failed();
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "Picture offscreen target cannot blit into itself",
                ));
            }
            if self.offscreens.get(&dst_handle).is_some_and(|target| {
                FrameOpacity::from_canvas(target.canvas.scratch.opacity()).is_transparent()
            }) {
                return Ok(());
            }
            let commands = {
                let target = self.offscreens.get(&dst_handle).ok_or_else(|| {
                    Error::new(
                        Errc::InvalidState,
                        "active Picture offscreen target disappeared before blit",
                    )
                })?;
                self.translated_picture_commands(handle, src_rect, dst_rect, &target.canvas)
            };
            let result = if let Some(commands) = commands {
                self.offscreens
                    .get_mut(&dst_handle)
                    .ok_or_else(|| {
                        Error::new(
                            Errc::InvalidState,
                            "active Picture offscreen target disappeared before command splice",
                        )
                    })?
                    .canvas
                    .record_validated_commands(commands)
            } else {
                let image = self
                    .offscreens
                    .get_mut(handle)
                    .and_then(RecordedPicture::materialized_image)
                    .ok_or_else(|| {
                        Error::new(
                            Errc::InvalidState,
                            "Picture offscreen content disappeared before nested blit",
                        )
                    })?;
                self.offscreens
                    .get_mut(&dst_handle)
                    .ok_or_else(|| {
                        Error::new(
                            Errc::InvalidState,
                            "active Picture offscreen target disappeared before image blit",
                        )
                    })?
                    .canvas
                    .record_picture_blit(image, src_rect, dst_rect)
            };
            if result.is_err() {
                self.mark_active_failed();
            }
            return result;
        }
        self.record_main_picture_blit(handle, src_rect, dst_rect)
    }

    fn memory_usage(&self) -> usize {
        self.offscreens
            .memory_usage()
            .saturating_add(self.canvas.scratch.memory_usage())
    }
}

/// State-preserving CPU scratch rasterizer. Consecutive CPU draws accumulate
/// in scratch and flush at painter-order barriers (native / Picture / finish),
/// so glyphs and rounded fills share one packed CpuSegment instead of
/// re-scanning the window after every op.
struct FrameRecordingCanvas {
    scratch: SharedRasterizer,
    encoder: Option<FrameEncoder>,
    blend_mode: BlendMode,
    blend_stack: Vec<BlendMode>,
    scratch_dirty: bool,
    /// Surface-space AABB covering pixels written since the last flush.
    /// Pack scans only this region (plus AA pad) instead of the full window.
    scratch_pack_bounds: Option<FrameRect>,
    deferred_error: Option<Error>,
    width: i32,
    height: i32,
}

impl FrameRecordingCanvas {
    fn new(width: i32, height: i32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let mut scratch = SharedRasterizer::new(PixelSurface::one_pixel());
        scratch.reset_state_for_extent(width, height);
        Self {
            scratch,
            encoder: None,
            blend_mode: BlendMode::default(),
            blend_stack: Vec::new(),
            scratch_dirty: false,
            scratch_pack_bounds: None,
            deferred_error: None,
            width,
            height,
        }
    }

    fn prepare_resize(width: i32, height: i32) -> Result<(i32, i32), Error> {
        let width = width.max(1);
        let height = height.max(1);
        PixelSurface::validate_extent(width, height)?;
        Ok((width, height))
    }

    fn commit_resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        self.scratch
            .replace_surface_preserving_state(PixelSurface::one_pixel());
        self.scratch.reset_state_for_extent(width, height);
        self.encoder = None;
        self.blend_mode = BlendMode::default();
        self.blend_stack.clear();
        self.scratch_dirty = false;
        self.scratch_pack_bounds = None;
        self.deferred_error = None;
    }

    fn begin_recording(&mut self, clear_target: bool) -> Result<(), Error> {
        // A successful flush clears every touched scratch pixel. Reuse that
        // allocation instead of reallocating and zeroing the full window each
        // frame. An abandoned recording may have unflushed pixels, so only
        // that recovery boundary pays for a conservative full clear.
        if self.encoder.is_some() || self.scratch_dirty || self.deferred_error.is_some() {
            self.scratch.surface_mut().clear_all();
        }
        self.scratch.reset_state_for_extent(self.width, self.height);
        self.blend_mode = BlendMode::default();
        self.blend_stack.clear();
        self.scratch_dirty = false;
        self.scratch_pack_bounds = None;
        self.deferred_error = None;
        let mut encoder =
            FrameEncoder::new(self.width, self.height).map_err(frame_encoder_error)?;
        if clear_target {
            encoder.clear(Color::transparent());
        }
        self.encoder = Some(encoder);
        Ok(())
    }

    fn finish_recording(&mut self) -> Result<FrameEncoder, Error> {
        self.flush_scratch()?;
        if let Some(error) = self.deferred_error.take() {
            return Err(error);
        }
        self.encoder.take().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "FrameEncoder recording was not started before finish",
            )
        })
    }

    fn flush_recording(&mut self) -> Result<(), Error> {
        self.flush_scratch()?;
        if let Some(error) = self.deferred_error.take() {
            return Err(error);
        }
        Ok(())
    }

    fn abandon_recording(&mut self) {
        if (
            self.scratch.surface().width(),
            self.scratch.surface().height(),
        ) == (1, 1)
            && (self.scratch_dirty || self.encoder.is_some() || self.deferred_error.is_some())
        {
            self.scratch.surface_mut().clear_all();
        }
        self.scratch.reset_state_for_extent(self.width, self.height);
        self.encoder = None;
        self.blend_mode = BlendMode::default();
        self.blend_stack.clear();
        self.scratch_dirty = false;
        self.scratch_pack_bounds = None;
        self.deferred_error = None;
        self.release_scratch_allocation();
    }

    fn release_scratch_allocation(&mut self) {
        if (
            self.scratch.surface().width(),
            self.scratch.surface().height(),
        ) != (1, 1)
        {
            self.scratch
                .replace_surface_preserving_state(PixelSurface::one_pixel());
        }
        self.scratch.reset_state_for_extent(self.width, self.height);
    }

    fn retained_memory_usage(&self) -> usize {
        self.scratch.memory_usage().saturating_add(
            self.encoder
                .as_ref()
                .map(FrameEncoder::retained_memory_usage)
                .unwrap_or(0),
        )
    }

    fn record_validated_commands(
        &mut self,
        commands: Vec<crate::draw::pipeline::FrameCommand>,
    ) -> Result<(), Error> {
        self.flush_scratch()?;
        self.encoder_mut()?
            .append_validated_commands(commands)
            .map_err(frame_encoder_error)?;
        Ok(())
    }

    fn record_picture_blit(
        &mut self,
        image: FrameImage,
        src: Rect,
        dst: Rect,
    ) -> Result<(), Error> {
        self.flush_scratch()?;
        if let Some((src, dst)) = self.direct_picture_geometry(src, dst) {
            let opacity = FrameOpacity::from_canvas(self.scratch.opacity());
            self.encoder_mut()?
                .blit_picture_with_opacity(image, src, dst, opacity);
            return Ok(());
        }
        self.ensure_scratch()?;
        self.scratch
            .blit_image(image.pixels(), image.width(), src, dst);
        self.note_scratch_bounds(dst, 1.0);
        self.scratch_dirty = true;
        self.flush_scratch()
    }

    fn record_direct_image_blit(
        &mut self,
        pixels: &[u32],
        source_width: i32,
        source_rect: Rect,
        destination_rect: Rect,
    ) -> Result<bool, Error> {
        let canvas_opacity = self.scratch.opacity();
        if !canvas_opacity.is_finite() {
            return Ok(false);
        }
        let opacity = FrameOpacity::from_canvas(canvas_opacity);
        if opacity.is_transparent() {
            return Ok(true);
        }
        let Ok(source_stride) = usize::try_from(source_width) else {
            return Ok(false);
        };
        if source_stride == 0 {
            return Ok(false);
        }
        let Ok(source_height) = i32::try_from(pixels.len() / source_stride) else {
            return Ok(false);
        };
        let Some((source, destination)) =
            self.direct_picture_geometry(source_rect, destination_rect)
        else {
            return Ok(false);
        };
        if source.width != destination.width
            || source.height != destination.height
            || !source.is_within(source_width, source_height)
        {
            return Ok(false);
        }

        let pixel_count = usize::try_from(i64::from(source.width) * i64::from(source.height))
            .map_err(|_| {
                Error::new(
                    Errc::GraphicsOutOfMemory,
                    "direct image blit crop exceeds addressable memory",
                )
            })?;
        let mut retained = Vec::new();
        retained.try_reserve_exact(pixel_count).map_err(|error| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!(
                    "direct image blit crop {}x{} allocation failed: {error}",
                    source.width, source.height
                ),
            )
        })?;
        let copy_width = source.width as usize;
        for y in source.y..source.y + source.height {
            let row = y as usize * source_stride + source.x as usize;
            retained.extend_from_slice(&pixels[row..row + copy_width]);
        }
        let image =
            FrameImage::new(source.width, source.height, retained).map_err(frame_encoder_error)?;
        let retained_source = FrameRect::new(0, 0, source.width, source.height);
        self.flush_scratch()?;
        self.encoder_mut()?
            .blit_picture_with_opacity(image, retained_source, destination, opacity);
        Ok(true)
    }

    fn note_scratch_bounds(&mut self, local: Rect, pad: f32) {
        let mapped = self.scratch.map_rect(local);
        let Some(bounds) = surface_pack_bounds(
            mapped,
            pad,
            (0.0, 0.0),
            self.scratch.current_clip(),
            self.width,
            self.height,
        ) else {
            return;
        };
        self.scratch_pack_bounds = Some(match self.scratch_pack_bounds {
            Some(prev) => union_frame_rect(prev, bounds),
            None => bounds,
        });
    }

    fn draw_cpu(&mut self, local_bounds: Rect, pad: f32, draw: impl FnOnce(&mut SharedRasterizer)) {
        if self.deferred_error.is_some() {
            return;
        }
        // Transparent scratch + source-over upload cannot preserve Additive
        // against prior commands; only Native FillRectAdditive is equivalent.
        if self.blend_mode == BlendMode::Additive {
            self.unsupported_state("destination-dependent Additive blend via CPU segment");
            return;
        }
        if let Err(error) = self.ensure_scratch() {
            self.remember_error(error);
            return;
        }
        draw(&mut self.scratch);
        self.note_scratch_bounds(local_bounds, pad);
        self.scratch_dirty = true;
        // Defer flush until a painter-order barrier (native op / Picture blit /
        // finish). Per-op flush re-scanned and re-uploaded after every glyph
        // and rounded fill, dominating record time on dense pages.
    }

    fn flush_scratch(&mut self) -> Result<(), Error> {
        if !self.scratch_dirty {
            return Ok(());
        }
        let flush_t0 = std::time::Instant::now();
        let pack_bounds = self.scratch_pack_bounds.take();
        let packed = pack_visible_scratch_tile(
            self.scratch.surface().pixels(),
            self.width,
            self.height,
            pack_bounds,
        );
        if let Some((pixels, dst)) = packed {
            let image =
                FrameImage::new(dst.width, dst.height, pixels).map_err(frame_encoder_error)?;
            let src = FrameRect::new(0, 0, dst.width, dst.height);
            self.encoder_mut()?.cpu_image_segment(image, src, dst);
        }
        // pack_bounds 是本批所有 draw bounds 的并集，不是最后一笔；清理该并集即可
        // 隔离下一批，同时避免每个 painter barrier 都扫完整窗口。
        if let Some(bounds) = pack_bounds.filter(|bounds| {
            let bounded_area = i64::from(bounds.width).saturating_mul(i64::from(bounds.height));
            let surface_area = i64::from(self.width).saturating_mul(i64::from(self.height));
            bounded_area.saturating_mul(2) < surface_area
        }) {
            self.scratch.surface_mut().clear_rect_raw(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
            );
        } else {
            // 大批次连续 clear_all 更快；pixels_mut 等无边界写入也必须保守全清。
            self.scratch.surface_mut().clear_all();
        }
        self.scratch_dirty = false;
        crate::core::perf_probe::add_cpu_flush(flush_t0.elapsed().as_micros());
        Ok(())
    }

    fn ensure_scratch(&mut self) -> Result<(), Error> {
        if (
            self.scratch.surface().width(),
            self.scratch.surface().height(),
        ) == (self.width, self.height)
        {
            return Ok(());
        }
        let surface = PixelSurface::try_new(self.width, self.height)?;
        self.scratch.replace_surface_preserving_state(surface);
        Ok(())
    }

    fn encoder_mut(&mut self) -> Result<&mut FrameEncoder, Error> {
        self.encoder.as_mut().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "FrameEncoder command was recorded outside its frame lifetime",
            )
        })
    }

    fn remember_error(&mut self, error: Error) {
        if self.deferred_error.is_none() {
            self.deferred_error = Some(error);
        }
    }

    fn native_src_over_rects(&self, rect: Rect) -> Option<(FrameRect, FrameRect)> {
        if !self.scratch.current_transform().is_identity() {
            return None;
        }

        // Match RasterRenderer's identity-transform fill path exactly: offset
        // changes x/y only, while width/height retain their original f32 values.
        let (offset_x, offset_y) = self.scratch.offset();
        let mapped = Rect::new(rect.x + offset_x, rect.y + offset_y, rect.w, rect.h);
        let right = f64::from(mapped.x) + f64::from(mapped.w);
        let bottom = f64::from(mapped.y) + f64::from(mapped.h);
        if !mapped.x.is_finite()
            || !mapped.y.is_finite()
            || !mapped.w.is_finite()
            || !mapped.h.is_finite()
            || mapped.x.fract() != 0.0
            || mapped.y.fract() != 0.0
            || mapped.w.fract() != 0.0
            || mapped.h.fract() != 0.0
            || mapped.x < 0.0
            || mapped.y < 0.0
            || mapped.w <= 0.0
            || mapped.h <= 0.0
            || right > f64::from(self.width)
            || bottom > f64::from(self.height)
        {
            return None;
        }
        Some((
            rect_to_frame(mapped).ok()?,
            self.native_src_over_fill_clip()?,
        ))
    }

    fn native_src_over_glyph(&self, x: i32, y: i32) -> Option<(FrameRect, i32, i32)> {
        if !self.scratch.current_transform().is_identity() {
            return None;
        }
        let (offset_x, offset_y) = self.scratch.offset();
        if !offset_x.is_finite()
            || !offset_y.is_finite()
            || offset_x.fract() != 0.0
            || offset_y.fract() != 0.0
        {
            return None;
        }
        let x = (x as f32 + offset_x) as i32;
        let y = (y as f32 + offset_y) as i32;
        Some((self.native_src_over_fill_clip()?, x, y))
    }

    fn native_src_over_fill_clip(&self) -> Option<FrameRect> {
        if !matches!(self.blend_mode, BlendMode::Alpha | BlendMode::SrcOver) {
            return None;
        }
        let clip = rect_to_frame(self.scratch.current_clip()).ok()?;
        Some(
            clip.intersection(FrameRect::new(0, 0, self.width, self.height))
                .unwrap_or(FrameRect::new(0, 0, 0, 0)),
        )
    }

    fn record_glyph_shared(
        &mut self,
        x: i32,
        y: i32,
        coverage: Arc<[u8]>,
        width: usize,
        height: usize,
        color: Color,
    ) {
        if let Some((clip, native_x, native_y)) = self.native_src_over_glyph(x, y) {
            let native_color =
                crate::draw::rasterizer::color_with_glyph_opacity(color, self.scratch.opacity());
            if native_color.a == 0 {
                return;
            }
            let Ok(glyph) = FrameGlyphBlit::new(
                native_x,
                native_y,
                Arc::clone(&coverage),
                width,
                height,
                native_color,
            ) else {
                // The historical void Canvas API treats empty or malformed
                // glyph coverage as a no-op; promotion must preserve it.
                return;
            };
            if clip.width <= 0 || clip.height <= 0 {
                return;
            }
            if let Err(error) = self.flush_scratch().and_then(|()| {
                self.encoder_mut()?.native(FrameRasterOp::BlitGlyphs {
                    glyphs: vec![glyph],
                    clip,
                });
                Ok(())
            }) {
                self.remember_error(error);
            }
            return;
        }
        let Ok(glyph) = FrameGlyphBlit::new(x, y, coverage, width, height, color) else {
            return;
        };
        let bounds = Rect::new(x as f32, y as f32, width as f32, height as f32);
        self.draw_cpu(bounds, 1.0, move |scratch| {
            scratch.blit_glyph(x, y, glyph.coverage().as_ref(), width, height, color)
        });
    }

    /// Additive 填充依赖目标像素，因此只能进入 Native 命令。几何约束与
    /// SrcOver 直达路径一致，但允许半透明源色。
    fn can_emit_native_additive_fill(&self, rect: Rect) -> bool {
        self.blend_mode == BlendMode::Additive
            && self.scratch.offset() == (0.0, 0.0)
            && self.scratch.current_transform().is_identity()
            && self.scratch.opacity() == 1.0
            && self.scratch.current_clip() == self.full_rect()
            && rect.x.fract() == 0.0
            && rect.y.fract() == 0.0
            && rect.w.fract() == 0.0
            && rect.h.fract() == 0.0
            && rect.x >= 0.0
            && rect.y >= 0.0
            && rect.w > 0.0
            && rect.h > 0.0
            && rect.x + rect.w <= self.width as f32
            && rect.y + rect.h <= self.height as f32
    }

    fn direct_picture_rects(&self, src: Rect, dst: Rect) -> Option<(FrameRect, FrameRect)> {
        if self.scratch.opacity() != 1.0 {
            return None;
        }
        self.direct_picture_geometry(src, dst)
    }

    fn direct_picture_geometry(&self, src: Rect, dst: Rect) -> Option<(FrameRect, FrameRect)> {
        let (offset_x, offset_y) = self.scratch.offset();
        if self.blend_mode == BlendMode::Additive
            || !offset_x.is_finite()
            || !offset_y.is_finite()
            || offset_x.fract() != 0.0
            || offset_y.fract() != 0.0
            || !self.scratch.current_transform().is_identity()
        {
            return None;
        }
        let src = rect_to_frame(src).ok()?;
        let dst =
            rect_to_frame(Rect::new(dst.x + offset_x, dst.y + offset_y, dst.w, dst.h)).ok()?;
        let clip = self.scratch.current_clip();
        if clip == self.full_rect() && dst.is_within(self.width, self.height) {
            return Some((src, dst));
        }
        if src.width != dst.width || src.height != dst.height {
            return None;
        }
        let clip = rect_to_frame(clip).ok()?;
        let left = dst.x.max(clip.x);
        let top = dst.y.max(clip.y);
        let right = dst
            .x
            .saturating_add(dst.width)
            .min(clip.x.saturating_add(clip.width));
        let bottom = dst
            .y
            .saturating_add(dst.height)
            .min(clip.y.saturating_add(clip.height));
        if left >= right || top >= bottom {
            return None;
        }
        let clipped_dst = FrameRect::new(left, top, right - left, bottom - top);
        let clipped_src = FrameRect::new(
            src.x.saturating_add(left - dst.x),
            src.y.saturating_add(top - dst.y),
            clipped_dst.width,
            clipped_dst.height,
        );
        Some((clipped_src, clipped_dst))
    }

    fn full_rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width as f32, self.height as f32)
    }

    fn unsupported_state(&mut self, detail: &'static str) {
        self.remember_error(Error::new(
            Errc::NotImplemented,
            format!("FrameEncoder recording cannot faithfully lower {detail}"),
        ));
    }
}

impl Canvas2D for FrameRecordingCanvas {
    fn current_transform(&self) -> Transform {
        self.scratch.current_transform()
    }

    fn set_transform(&mut self, transform: Transform) {
        self.scratch.set_transform(transform);
    }

    fn offset(&self) -> (f32, f32) {
        self.scratch.offset()
    }

    fn set_offset(&mut self, dx: f32, dy: f32) {
        self.scratch.set_offset(dx, dy);
    }

    fn translate(&mut self, dx: f32, dy: f32) {
        self.scratch.translate(dx, dy);
    }

    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        let additive_radius = if self.blend_mode == BlendMode::Additive {
            match radius.map(FrameRadius::new).transpose() {
                Ok(radius) => radius,
                Err(error) => {
                    self.remember_error(frame_encoder_error(error));
                    return;
                }
            }
        } else {
            None
        };
        if self.can_emit_native_additive_fill(rect) {
            if let Err(error) = self.flush_scratch().and_then(|()| {
                let rect =
                    FrameRect::new(rect.x as i32, rect.y as i32, rect.w as i32, rect.h as i32);
                let operation = match additive_radius {
                    Some(radius) => FrameRasterOp::FillRoundedRectAdditive {
                        rect,
                        color,
                        radius,
                    },
                    None => FrameRasterOp::FillRectAdditive { rect, color },
                };
                self.encoder_mut()?.native(operation);
                Ok(())
            }) {
                self.remember_error(error);
            }
            return;
        }
        if let Some((native_rect, clip)) = self.native_src_over_rects(rect) {
            let native_color = crate::draw::rasterizer::color_with_premultiplied_opacity(
                color,
                self.scratch.opacity(),
            );
            let native_radius = match radius.map(FrameRadius::new).transpose() {
                Ok(radius) => radius,
                Err(_) => {
                    // 普通 blend 的非法半径历史上由 CPU rasterizer 处理；
                    // 这里只拒绝提升，不把既有 void API 改成 deferred typed failure。
                    self.draw_cpu(rect, 1.0, |scratch| scratch.fill_rect(rect, color, radius));
                    return;
                }
            };
            if clip.width <= 0 || clip.height <= 0 {
                return;
            }
            if let Err(error) = self.flush_scratch().and_then(|()| {
                let full_clip = FrameRect::new(0, 0, self.width, self.height);
                let operation = if clip == full_clip {
                    match native_radius {
                        Some(radius) => {
                            let value = radius.to_radius();
                            if value.tl != 0.0
                                || value.tr != 0.0
                                || value.br != 0.0
                                || value.bl != 0.0
                            {
                                FrameRasterOp::FillRoundedRect {
                                    rect: native_rect,
                                    color: native_color,
                                    radius,
                                }
                            } else {
                                FrameRasterOp::FillRect {
                                    rect: native_rect,
                                    color: native_color,
                                }
                            }
                        }
                        None => FrameRasterOp::FillRect {
                            rect: native_rect,
                            color: native_color,
                        },
                    }
                } else {
                    FrameRasterOp::FillRoundedRectClipped {
                        rect: native_rect,
                        color: native_color,
                        radius: native_radius.unwrap_or_else(FrameRadius::zero),
                        clip,
                    }
                };
                self.encoder_mut()?.native(operation);
                Ok(())
            }) {
                self.remember_error(error);
            }
            return;
        }
        self.draw_cpu(rect, 1.0, |scratch| scratch.fill_rect(rect, color, radius));
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let bounds = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        if r.is_finite() && r > 0.0 && self.native_src_over_rects(bounds).is_some() {
            // A circle is exactly the shared rounded-rect SDF with a square
            // extent and every corner radius equal to half that extent.
            self.fill_rect(bounds, color, Some(Radius::uniform(r)));
            return;
        }
        self.draw_cpu(bounds, 1.0, |scratch| scratch.fill_circle(cx, cy, r, color));
    }

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        self.draw_cpu(rect, 1.0, |scratch| scratch.fill_ellipse(rect, color));
    }

    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, sa: f32, ea: f32, color: Color) {
        let bounds = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        self.draw_cpu(bounds, 1.0, |scratch| {
            scratch.fill_sector(cx, cy, r, sa, ea, color)
        });
    }

    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        let bounds = path
            .bounds()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, self.width as f32, self.height as f32));
        self.draw_cpu(bounds, 1.0, |scratch| {
            scratch.fill_path(path, color, fill_rule)
        });
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32, radius: Option<Radius>) {
        if let Some((native_rect, clip)) = self.native_src_over_rects(rect) {
            let native_radius = radius.unwrap_or_default();
            let (Ok(native_radius), Ok(line_width)) = (
                FrameRadius::new(native_radius),
                FrameStrokeWidth::new(width),
            ) else {
                self.draw_cpu(rect, width.max(1.0), |scratch| {
                    scratch.stroke_rect(rect, color, width, radius)
                });
                return;
            };
            if clip.width <= 0 || clip.height <= 0 {
                return;
            }
            let native_color = crate::draw::rasterizer::color_with_premultiplied_opacity(
                color,
                self.scratch.opacity(),
            );
            if native_color.a == 0 {
                return;
            }
            if let Err(error) = self.flush_scratch().and_then(|()| {
                self.encoder_mut()?
                    .native(FrameRasterOp::StrokeRoundedRects {
                        strokes: vec![FrameStrokeRect::new(
                            native_rect,
                            native_color,
                            native_radius,
                            line_width,
                        )],
                        clip,
                    });
                Ok(())
            }) {
                self.remember_error(error);
            }
            return;
        }
        self.draw_cpu(rect, width.max(1.0), |scratch| {
            scratch.stroke_rect(rect, color, width, radius)
        });
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, width: f32) {
        let bounds = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        if r.is_finite() && r > 0.0 && self.native_src_over_rects(bounds).is_some() {
            // A circle stroke is the shared rounded-rect stroke SDF over a
            // square whose four radii equal half the extent.
            self.stroke_rect(bounds, color, width, Some(Radius::uniform(r)));
            return;
        }
        self.draw_cpu(bounds, width.max(1.0), |scratch| {
            scratch.stroke_circle(cx, cy, r, color, width)
        });
    }

    fn stroke_path(&mut self, path: &Path, color: Color, options: &StrokeOptions) {
        let bounds = path
            .bounds()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, self.width as f32, self.height as f32));
        self.draw_cpu(bounds, options.width.max(1.0), |scratch| {
            scratch.stroke_path(path, color, options)
        });
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        let bounds = Rect::new(
            x1.min(x2),
            y1.min(y2),
            (x1 - x2).abs().max(1.0),
            (y1 - y2).abs().max(1.0),
        );
        self.draw_cpu(bounds, width.max(1.0), |scratch| {
            scratch.draw_line(x1, y1, x2, y2, color, width)
        });
    }

    fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
    ) {
        self.draw_cpu(rect, 1.0, |scratch| {
            scratch.fill_linear_gradient(rect, color_a, color_b, dir)
        });
    }

    fn fill_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        inner_r: f32,
        outer_r: f32,
        inner_color: Color,
        outer_color: Color,
    ) {
        let bounds = Rect::new(cx - outer_r, cy - outer_r, outer_r * 2.0, outer_r * 2.0);
        self.draw_cpu(bounds, 1.0, |scratch| {
            scratch.fill_radial_gradient(cx, cy, inner_r, outer_r, inner_color, outer_color)
        });
    }

    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        radius: Option<Radius>,
    ) {
        let pad = blur.max(0.0) + offset_x.abs().max(offset_y.abs()) + 1.0;
        self.draw_cpu(rect, pad, |scratch| {
            scratch.draw_box_shadow(rect, blur, offset_x, offset_y, color, radius)
        });
    }

    fn draw_box_shadow_ambient(
        &mut self,
        rect: Rect,
        blur: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        radius: Option<Radius>,
    ) {
        let pad = blur.max(0.0) + offset_x.abs().max(offset_y.abs()) + 1.0;
        self.draw_cpu(rect, pad, |scratch| {
            scratch.draw_box_shadow_ambient(rect, blur, offset_x, offset_y, color, radius)
        });
    }

    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        if self.deferred_error.is_some() {
            return;
        }
        match self.record_direct_image_blit(src, src_w, src_rect, dst_rect) {
            Ok(true) => return,
            Ok(false) => {}
            Err(error) => {
                self.remember_error(error);
                return;
            }
        }
        self.draw_cpu(dst_rect, 1.0, |scratch| {
            scratch.blit_image(src, src_w, src_rect, dst_rect)
        });
    }

    fn blit_glyph(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        width: usize,
        height: usize,
        color: Color,
    ) {
        self.record_glyph_shared(x, y, Arc::from(coverage), width, height, color);
    }

    fn blit_glyph_shared(
        &mut self,
        x: i32,
        y: i32,
        coverage: Arc<[u8]>,
        width: usize,
        height: usize,
        color: Color,
    ) {
        self.record_glyph_shared(x, y, coverage, width, height, color);
    }

    fn save(&mut self) {
        self.scratch.save();
        self.blend_stack.push(self.blend_mode);
    }

    fn restore(&mut self) {
        self.scratch.restore();
        if let Some(mode) = self.blend_stack.pop() {
            self.blend_mode = mode;
        }
    }

    fn push_clip(&mut self, rect: Rect) {
        self.scratch.push_clip(rect);
    }

    fn pop_clip(&mut self) {
        self.scratch.pop_clip();
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.scratch.set_opacity(opacity);
    }

    fn opacity(&self) -> f32 {
        self.scratch.opacity()
    }

    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.scratch.set_blend_mode(mode);
        self.blend_mode = mode;
    }

    fn push_clip_path(&mut self, _path: &Path) {
        self.unsupported_state("path clip");
    }

    fn pixels_mut(&mut self) -> &mut [u32] {
        if let Err(error) = self.ensure_scratch() {
            self.remember_error(error);
            return self.scratch.pixels_mut();
        }
        self.scratch_dirty = true;
        self.scratch.pixels_mut()
    }

    fn surface_size(&self) -> crate::core::Size {
        crate::core::Size::new(self.width as f32, self.height as f32)
    }

    fn current_clip(&self) -> Rect {
        self.scratch.current_clip()
    }

    fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32) {
        if self.deferred_error.is_some() {
            return;
        }
        let int_dx = dx.round() as i32;
        let int_dy = dy.round() as i32;
        if int_dx == 0 && int_dy == 0 {
            return;
        }
        let Ok(frame_viewport) = rect_to_frame(viewport) else {
            self.unsupported_state("scroll-region with non-integral viewport");
            return;
        };
        if let Err(error) = self.flush_scratch().and_then(|()| {
            self.encoder_mut()?.native(FrameRasterOp::ScrollCopy {
                viewport: frame_viewport,
                dx: int_dx,
                dy: int_dy,
            });
            Ok(())
        }) {
            self.remember_error(error);
        }
    }
}

/// Packs one transparent full-surface scratch operation into its smallest
/// alpha-visible tile. When `scan_bounds` is set, only that AABB is scanned —
/// per-op flushes otherwise re-scanned the entire window every glyph/round-rect.
fn pack_visible_scratch_tile(
    pixels: &[u32],
    width: i32,
    height: i32,
    scan_bounds: Option<FrameRect>,
) -> Option<(Vec<u32>, FrameRect)> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let expected = (width as usize).checked_mul(height as usize)?;
    if pixels.len() < expected {
        return None;
    }

    let (scan_left, scan_top, scan_right, scan_bottom) = match scan_bounds {
        Some(b) if b.width > 0 && b.height > 0 => {
            let left = b.x.max(0).min(width);
            let top = b.y.max(0).min(height);
            let right = (b.x + b.width).max(0).min(width);
            let bottom = (b.y + b.height).max(0).min(height);
            if left >= right || top >= bottom {
                return None;
            }
            (left, top, right, bottom)
        }
        _ => (0, 0, width, height),
    };

    let mut left = scan_right;
    let mut top = scan_bottom;
    let mut right = scan_left;
    let mut bottom = scan_top;
    for y in scan_top..scan_bottom {
        let row = y as usize * width as usize;
        for x in scan_left..scan_right {
            if pixels[row + x as usize] & 0xff00_0000 == 0 {
                continue;
            }
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if left >= right || top >= bottom {
        return None;
    }

    let tile_width = right - left;
    let tile_height = bottom - top;
    let mut packed = Vec::with_capacity((tile_width as usize).checked_mul(tile_height as usize)?);
    for y in top..bottom {
        let row = y as usize * width as usize;
        packed.extend_from_slice(&pixels[row + left as usize..row + right as usize]);
    }
    Some((packed, FrameRect::new(left, top, tile_width, tile_height)))
}

fn surface_pack_bounds(
    local: Rect,
    pad: f32,
    offset: (f32, f32),
    clip: Rect,
    width: i32,
    height: i32,
) -> Option<FrameRect> {
    if !local.x.is_finite()
        || !local.y.is_finite()
        || !local.w.is_finite()
        || !local.h.is_finite()
        || local.w <= 0.0
        || local.h <= 0.0
    {
        return None;
    }
    let pad = pad.max(0.0);
    let x0 = (local.x + offset.0 - pad).floor();
    let y0 = (local.y + offset.1 - pad).floor();
    let x1 = (local.x + offset.0 + local.w + pad).ceil();
    let y1 = (local.y + offset.1 + local.h + pad).ceil();
    let cx0 = clip.x.floor();
    let cy0 = clip.y.floor();
    let cx1 = (clip.x + clip.w).ceil();
    let cy1 = (clip.y + clip.h).ceil();
    let left = x0.max(cx0).max(0.0) as i32;
    let top = y0.max(cy0).max(0.0) as i32;
    let right = x1.min(cx1).min(width as f32) as i32;
    let bottom = y1.min(cy1).min(height as f32) as i32;
    if left >= right || top >= bottom {
        return None;
    }
    Some(FrameRect::new(left, top, right - left, bottom - top))
}

fn union_frame_rect(a: FrameRect, b: FrameRect) -> FrameRect {
    let left = a.x.min(b.x);
    let top = a.y.min(b.y);
    let right = (a.x + a.width).max(b.x + b.width);
    let bottom = (a.y + a.height).max(b.y + b.height);
    FrameRect::new(left, top, right - left, bottom - top)
}

fn rect_to_frame(rect: Rect) -> Result<FrameRect, Error> {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.w.is_finite()
        || !rect.h.is_finite()
        || rect.x.fract() != 0.0
        || rect.y.fract() != 0.0
        || rect.w.fract() != 0.0
        || rect.h.fract() != 0.0
    {
        return Err(Error::new(
            Errc::InvalidArgument,
            "Picture source/destination must use finite integral FrameEncoder coordinates",
        ));
    }
    Ok(FrameRect::new(
        rect.x as i32,
        rect.y as i32,
        rect.w as i32,
        rect.h as i32,
    ))
}

fn frame_encoder_error(error: FrameEncoderError) -> Error {
    Error::new(
        Errc::InvalidState,
        format!("could not record FrameEncoder command: {error}"),
    )
}
