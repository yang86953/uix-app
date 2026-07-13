//! Private, API-neutral producer for one ordered main [`FrameEncoder`].
//!
//! The compositor paints into this engine instead of a real presentation
//! surface. Every visual operation is either lowered to a proven native rect,
//! or immediately rasterized into one transparent CPU segment. Picture
//! offscreens remain private CPU targets and are recorded as ordered blits.

use crate::core::{DamageRegion, Errc, Error, Rect};
use crate::draw::backend::{CpuBackend, RenderBackend};
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::engine::RenderOutcome;
use crate::draw::pipeline::{
    EncodedFrameExecution, EncodedPictureExecution, FrameEncoder, FrameEncoderError, FrameImage,
    FrameRasterOp, FrameRect,
};
use crate::draw::primitives::path::{FillRule, Path};
use crate::draw::primitives::stroker::StrokeOptions;
use crate::draw::primitives::types::{BlendMode, GradientDirection, ImageHandle, Radius};
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::draw::Color;

/// The only producer used by [`super::render_frame::FrameRenderer`]. It owns
/// no API object and cannot present; its output is consumed exactly once by
/// the caller's real graphics engine.
pub(crate) struct FrameRecordingEngine {
    canvas: FrameRecordingCanvas,
    offscreens: CpuBackend,
    active_offscreen: Option<ImageHandle>,
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
            offscreens: CpuBackend::new(),
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
        self.active_offscreen = None;
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

    fn record_main_picture_blit(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        let (pixels, width) = self
            .offscreens
            .copy_offscreen_pixels(handle)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "Picture offscreen pixels disappeared before FrameEncoder recording",
                )
            })?;
        let width = width.max(1);
        let height = i32::try_from(pixels.len() / width as usize).map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "Picture offscreen extent overflows FrameEncoder image",
            )
        })?;
        if pixels.len() != width as usize * height as usize {
            return Err(Error::new(
                Errc::InvalidState,
                "Picture offscreen pixels do not match their reported row width",
            ));
        }
        let image = FrameImage::new(width, height, pixels).map_err(frame_encoder_error)?;
        self.canvas.record_picture_blit(image, src_rect, dst_rect)
    }
}

impl GraphicsEngine for FrameRecordingEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.resize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.active_offscreen = None;
        self.offscreens.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let width = width.max(1);
        let height = height.max(1);
        self.canvas.resize(width, height);
        self.offscreens.resize(width, height)
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
        self.offscreens.create_offscreen(width, height)
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        if self.active_offscreen == Some(handle) {
            self.active_offscreen = None;
        }
        self.offscreens.destroy_offscreen(handle);
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.offscreens.offscreen_canvas(handle)
    }

    fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.offscreens.copy_offscreen_pixels(handle)
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        self.offscreens.try_execute_encoded_picture(handle, encoder)
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
        self.offscreens.try_begin_offscreen_paint(handle)?;
        self.active_offscreen = Some(*handle);
        Ok(())
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.offscreens.try_flush_offscreen_paint(handle)
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.offscreens.try_end_offscreen_paint()?;
        self.active_offscreen = None;
        Ok(())
    }

    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        if self.active_offscreen.is_some() {
            return self
                .offscreens
                .try_blit_offscreen_src(handle, src_rect, dst_rect);
        }
        self.record_main_picture_blit(handle, src_rect, dst_rect)
    }

    fn memory_usage(&self) -> usize {
        self.offscreens.memory_usage()
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
        Self {
            scratch: SharedRasterizer::new(PixelSurface::new(width, height)),
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

    fn resize(&mut self, width: i32, height: i32) {
        let width = width.max(1);
        let height = height.max(1);
        self.width = width;
        self.height = height;
        self.scratch = SharedRasterizer::new(PixelSurface::new(width, height));
        self.encoder = None;
        self.blend_mode = BlendMode::default();
        self.blend_stack.clear();
        self.scratch_dirty = false;
        self.scratch_pack_bounds = None;
        self.deferred_error = None;
    }

    fn begin_recording(&mut self, clear_target: bool) -> Result<(), Error> {
        self.scratch = SharedRasterizer::new(PixelSurface::new(self.width, self.height));
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

    fn record_picture_blit(
        &mut self,
        image: FrameImage,
        src: Rect,
        dst: Rect,
    ) -> Result<(), Error> {
        self.flush_scratch()?;
        if self.can_emit_direct_picture() {
            self.encoder_mut()?
                .blit_picture(image, rect_to_frame(src)?, rect_to_frame(dst)?);
            return Ok(());
        }
        self.scratch
            .blit_image(image.pixels(), image.width(), src, dst);
        self.note_scratch_bounds(dst, 1.0);
        self.scratch_dirty = true;
        self.flush_scratch()
    }

    fn note_scratch_bounds(&mut self, local: Rect, pad: f32) {
        let Some(bounds) = surface_pack_bounds(
            local,
            pad,
            self.scratch.offset(),
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
        // Full clear: deferred batching may have touched many tiles; clearing
        // only the last pack AABB would leave stale pixels for the next batch.
        self.scratch.surface_mut().clear_all();
        self.scratch_dirty = false;
        crate::draw::perf_probe::add_cpu_flush(flush_t0.elapsed().as_micros());
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

    fn can_emit_native_rect(&self, rect: Rect, color: Color, radius: Option<Radius>) -> bool {
        radius.is_none()
            && color.a == u8::MAX
            && self.blend_mode != BlendMode::Additive
            && self.scratch.offset() == (0.0, 0.0)
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

    /// Additive fills are destination-dependent, so they must land as Native
    /// ops. Geometry constraints match the SrcOver native path, but alpha may
    /// be any value (Additive is meaningful with translucent sources).
    fn can_emit_native_additive_rect(&self, rect: Rect, radius: Option<Radius>) -> bool {
        self.blend_mode == BlendMode::Additive
            && radius.is_none()
            && self.scratch.offset() == (0.0, 0.0)
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

    fn can_emit_direct_picture(&self) -> bool {
        self.blend_mode != BlendMode::Additive
            && self.scratch.offset() == (0.0, 0.0)
            && self.scratch.opacity() == 1.0
            && self.scratch.current_clip() == self.full_rect()
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
        if self.can_emit_native_additive_rect(rect, radius) {
            if let Err(error) = self.flush_scratch().and_then(|()| {
                self.encoder_mut()?.native(FrameRasterOp::FillRectAdditive {
                    rect: FrameRect::new(
                        rect.x as i32,
                        rect.y as i32,
                        rect.w as i32,
                        rect.h as i32,
                    ),
                    color,
                });
                Ok(())
            }) {
                self.remember_error(error);
            }
            return;
        }
        if self.can_emit_native_rect(rect, color, radius) {
            if let Err(error) = self.flush_scratch().and_then(|()| {
                self.encoder_mut()?.native(FrameRasterOp::FillRect {
                    rect: FrameRect::new(
                        rect.x as i32,
                        rect.y as i32,
                        rect.w as i32,
                        rect.h as i32,
                    ),
                    color,
                });
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
        self.draw_cpu(bounds, 1.0, |scratch| scratch.fill_circle(cx, cy, r, color));
    }

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        self.draw_cpu(rect, 1.0, |scratch| scratch.fill_ellipse(rect, color));
    }

    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, sa: f32, ea: f32, color: Color) {
        let bounds = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
        self.draw_cpu(bounds, 1.0, |scratch| scratch.fill_sector(cx, cy, r, sa, ea, color));
    }

    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        let bounds = path
            .bounds()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, self.width as f32, self.height as f32));
        self.draw_cpu(bounds, 1.0, |scratch| scratch.fill_path(path, color, fill_rule));
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32, radius: Option<Radius>) {
        self.draw_cpu(rect, width.max(1.0), |scratch| {
            scratch.stroke_rect(rect, color, width, radius)
        });
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, width: f32) {
        let bounds = Rect::new(cx - r, cy - r, r * 2.0, r * 2.0);
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
        let bounds = Rect::new(x as f32, y as f32, width as f32, height as f32);
        self.draw_cpu(bounds, 1.0, |scratch| {
            scratch.blit_glyph(x, y, coverage, width, height, color)
        });
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
        self.scratch_dirty = true;
        self.scratch.pixels_mut()
    }

    fn surface_size(&self) -> crate::core::Size {
        self.scratch.surface_size()
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

