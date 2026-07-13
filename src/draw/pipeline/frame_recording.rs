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

    pub(crate) fn begin_recording(&mut self) -> Result<(), Error> {
        self.active_offscreen = None;
        self.canvas.begin_recording()
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

    fn shutdown(&mut self) {
        self.active_offscreen = None;
        self.offscreens.shutdown();
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

/// State-preserving CPU scratch rasterizer. Its pixels are cleared after each
/// visual operation, not accumulated across operations: this keeps the exact
/// source-over rounding and painter order when the segments are replayed.
struct FrameRecordingCanvas {
    scratch: SharedRasterizer,
    encoder: Option<FrameEncoder>,
    blend_mode: BlendMode,
    blend_stack: Vec<BlendMode>,
    scratch_dirty: bool,
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
        self.deferred_error = None;
    }

    fn begin_recording(&mut self) -> Result<(), Error> {
        self.scratch = SharedRasterizer::new(PixelSurface::new(self.width, self.height));
        self.blend_mode = BlendMode::default();
        self.blend_stack.clear();
        self.scratch_dirty = false;
        self.deferred_error = None;
        let mut encoder =
            FrameEncoder::new(self.width, self.height).map_err(frame_encoder_error)?;
        encoder.clear(Color::transparent());
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
        self.scratch_dirty = true;
        self.flush_scratch()
    }

    fn draw_cpu(&mut self, draw: impl FnOnce(&mut SharedRasterizer)) {
        if self.deferred_error.is_some() {
            return;
        }
        draw(&mut self.scratch);
        self.scratch_dirty = true;
        if let Err(error) = self.flush_scratch() {
            self.remember_error(error);
        }
    }

    fn flush_scratch(&mut self) -> Result<(), Error> {
        if !self.scratch_dirty {
            return Ok(());
        }
        let packed =
            pack_visible_scratch_tile(self.scratch.surface().pixels(), self.width, self.height);
        if let Some((pixels, dst)) = packed {
            let image =
                FrameImage::new(dst.width, dst.height, pixels).map_err(frame_encoder_error)?;
            let src = FrameRect::new(0, 0, dst.width, dst.height);
            self.encoder_mut()?.cpu_image_segment(image, src, dst);
        }
        self.scratch.surface_mut().clear_all();
        self.scratch_dirty = false;
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
        self.draw_cpu(|scratch| scratch.fill_rect(rect, color, radius));
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.draw_cpu(|scratch| scratch.fill_circle(cx, cy, r, color));
    }

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        self.draw_cpu(|scratch| scratch.fill_ellipse(rect, color));
    }

    fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, sa: f32, ea: f32, color: Color) {
        self.draw_cpu(|scratch| scratch.fill_sector(cx, cy, r, sa, ea, color));
    }

    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        self.draw_cpu(|scratch| scratch.fill_path(path, color, fill_rule));
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32, radius: Option<Radius>) {
        self.draw_cpu(|scratch| scratch.stroke_rect(rect, color, width, radius));
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, width: f32) {
        self.draw_cpu(|scratch| scratch.stroke_circle(cx, cy, r, color, width));
    }

    fn stroke_path(&mut self, path: &Path, color: Color, options: &StrokeOptions) {
        self.draw_cpu(|scratch| scratch.stroke_path(path, color, options));
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        self.draw_cpu(|scratch| scratch.draw_line(x1, y1, x2, y2, color, width));
    }

    fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
    ) {
        self.draw_cpu(|scratch| scratch.fill_linear_gradient(rect, color_a, color_b, dir));
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
        self.draw_cpu(|scratch| {
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
        self.draw_cpu(|scratch| {
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
        self.draw_cpu(|scratch| {
            scratch.draw_box_shadow_ambient(rect, blur, offset_x, offset_y, color, radius)
        });
    }

    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        self.draw_cpu(|scratch| scratch.blit_image(src, src_w, src_rect, dst_rect));
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
        self.draw_cpu(|scratch| scratch.blit_glyph(x, y, coverage, width, height, color));
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
        if mode == BlendMode::Additive {
            self.unsupported_state("destination-dependent Additive blend");
        }
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

    fn scroll_region(&mut self, _viewport: Rect, _dx: f32, _dy: f32) {
        self.unsupported_state("scroll-region copy");
    }
}

/// Packs one transparent full-surface scratch operation into its smallest
/// alpha-visible tile. CPU segments stay immutable and ordered, but no longer
/// retain a full window-sized pixel buffer for every glyph or rounded shape.
fn pack_visible_scratch_tile(
    pixels: &[u32],
    width: i32,
    height: i32,
) -> Option<(Vec<u32>, FrameRect)> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let expected = (width as usize).checked_mul(height as usize)?;
    if pixels.len() < expected {
        return None;
    }

    let mut left = width;
    let mut top = height;
    let mut right = 0;
    let mut bottom = 0;
    for y in 0..height {
        let row = y as usize * width as usize;
        for x in 0..width {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_canvas_emits_native_cpu_and_picture_commands_in_painter_order() {
        let mut engine = FrameRecordingEngine::new();
        engine.initialize(12, 8).expect("initialize recorder");
        engine.begin_recording().expect("begin recording");
        engine
            .canvas_2d()
            .fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::red(), None);
        engine.canvas_2d().fill_rect(
            Rect::new(4.0, 1.0, 2.0, 2.0),
            Color::from_rgba(0, 120, 255, 128),
            Some(Radius::uniform(1.0)),
        );
        let picture = engine.create_offscreen(2, 2).expect("Picture");
        engine
            .try_begin_offscreen_paint(&picture)
            .expect("begin Picture");
        engine
            .offscreen_canvas(&picture)
            .expect("Picture canvas")
            .fill_rect(Rect::new(0.0, 0.0, 2.0, 2.0), Color::green(), None);
        engine
            .try_end_offscreen_paint()
            .expect("end Picture target");
        engine
            .try_blit_offscreen_src(
                &picture,
                Rect::new(0.0, 0.0, 2.0, 2.0),
                Rect::new(8.0, 4.0, 2.0, 2.0),
            )
            .expect("record Picture blit");
        engine
            .canvas_2d()
            .fill_rect(Rect::new(9.0, 1.0, 2.0, 2.0), Color::blue(), None);

        let encoder = engine.finish_recording().expect("finish recorder");
        assert!(matches!(
            encoder.commands()[0],
            crate::draw::pipeline::FrameCommand::Clear { .. }
        ));
        assert!(matches!(
            encoder.commands()[1],
            crate::draw::pipeline::FrameCommand::Native { .. }
        ));
        assert!(matches!(
            encoder.commands()[2],
            crate::draw::pipeline::FrameCommand::CpuSegment { .. }
        ));
        assert!(matches!(
            encoder.commands()[3],
            crate::draw::pipeline::FrameCommand::PictureBlit { .. }
        ));
        assert!(matches!(
            encoder.commands()[4],
            crate::draw::pipeline::FrameCommand::Native { .. }
        ));
    }

    #[test]
    fn recording_canvas_retains_compact_cpu_segment_tiles() {
        let mut engine = FrameRecordingEngine::new();
        engine.initialize(1200, 800).expect("initialize recorder");
        engine.begin_recording().expect("begin recording");
        let color = Color::from_rgba(20, 40, 60, 128);
        engine
            .canvas_2d()
            .fill_rect(Rect::new(701.0, 503.0, 3.0, 2.0), color, None);

        let encoder = engine.finish_recording().expect("finish recorder");
        let cpu_segment = encoder
            .commands()
            .iter()
            .find_map(|command| match command {
                crate::draw::pipeline::FrameCommand::CpuSegment { image, src, dst } => {
                    Some((image, src, dst))
                }
                _ => None,
            })
            .expect("CPU segment");

        assert_eq!((cpu_segment.0.width(), cpu_segment.0.height()), (3, 2));
        assert_eq!(cpu_segment.0.pixels().len(), 6);
        assert_eq!(*cpu_segment.1, FrameRect::new(0, 0, 3, 2));
        assert_eq!(*cpu_segment.2, FrameRect::new(701, 503, 3, 2));
        let reference = encoder.render_reference();
        assert_eq!(
            reference.pixel(700, 503),
            Some(Color::transparent().premultiplied())
        );
        assert_eq!(reference.pixel(701, 503), Some(color.premultiplied()));
        assert_eq!(reference.pixel(703, 504), Some(color.premultiplied()));
        assert_eq!(
            reference.pixel(704, 504),
            Some(Color::transparent().premultiplied())
        );
    }

    #[test]
    fn additive_recording_fails_instead_of_approximating_destination_blend() {
        let mut engine = FrameRecordingEngine::new();
        engine.initialize(2, 2).expect("initialize recorder");
        engine.begin_recording().expect("begin recording");
        engine.canvas_2d().set_blend_mode(BlendMode::Additive);
        engine
            .canvas_2d()
            .fill_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::red(), None);

        let error = engine
            .finish_recording()
            .expect_err("Additive must not become a source-over CPU segment");
        assert_eq!(error.code(), Errc::NotImplemented);
    }
}
