//! Private, API-neutral producer for one ordered main [`FrameEncoder`].
//!
//! The compositor paints into this command target instead of a live presentation
//! surface. Every visual operation is either lowered to a proven native op or
//! rasterized into one transparent CPU segment. Picture offscreens retain the
//! same API-neutral stream while it stays within the former BGRA memory budget;
//! unsafe mappings materialize that stream lazily as one ordered image blit.
//!
//! 子模块划分（P2 行数治理）：[`offscreen`] 离屏池、[`canvas`] 画布录制器、
//! [`canvas2d`] `Canvas2D` 实现、[`geometry`] 几何辅助；本文件保留
//! [`CommandRecorder`] 主体并重导出，`recorder::CommandRecorder` 路径不变。

pub(crate) mod canvas;
pub(crate) mod canvas2d;
// recorder 只在可保真 source scratch 状态下接受路径 coverage clip。
pub(crate) mod canvas_clip;
// raw image 的 retained crop 与 Additive sampled 直达逻辑独立于画布主体。
pub(crate) mod canvas_image;
pub(crate) mod geometry;
pub(crate) mod offscreen;

pub(crate) use offscreen::{ActiveOffscreen, RecordedPicture, RecordedPicturePool};
use self::offscreen::RecordedPicturePayload;

use crate::core::{DamageRegion, Errc, Error, Rect};
use crate::draw::outcome::RenderOutcome;
use crate::draw::painting::{
    EncodedFrameExecution, EncodedPictureExecution, FrameEncoder, FrameImage, FrameOpacity, FrameRect, FrameSampledRect,
};
use crate::draw::{Canvas2D, GraphicsCapabilities, RenderTarget, UpdateStrategy};
use crate::draw::geometry::types::ImageHandle;

use self::canvas::FrameRecordingCanvas;

pub(crate) struct CommandRecorder {
    canvas: FrameRecordingCanvas,
    offscreens: RecordedPicturePool,
    active_offscreen: Option<ActiveOffscreen>,
}

impl Default for CommandRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRecorder {
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

    // 测试目标保留 scratch surface 尺寸观测入口，供 recorder 生命周期测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn scratch_surface_size(&self) -> (i32, i32) {
        (
            self.canvas.scratch.surface().width(),
            self.canvas.scratch.surface().height(),
        )
    }

    // 测试目标保留 offscreen scratch 尺寸观测入口，供 recorder 生命周期测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
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
    ) -> Option<Vec<crate::draw::painting::FrameCommand>> {
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

impl RenderTarget for CommandRecorder {
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
        RenderOutcome::Failed(crate::draw::outcome::GraphicsFailure::from_error(
            Error::new(Errc::InvalidState, "CommandRecorder cannot present a frame"),
        ))
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        GraphicsCapabilities::backend_managed_with_offscreen()
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
                    vec![crate::draw::painting::FrameCommand::PictureBlit {
                        image: encoder.render_image(),
                        src: full,
                        dst: FrameSampledRect::from_integer(full),
                        opacity: crate::draw::painting::FrameOpacity::opaque(),
                        additive: false,
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
            "CommandRecorder cannot execute a final frame",
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
