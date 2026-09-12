//! 私有、API 中立的单个主 [`FrameEncoder`] 有序生产者。
//!
//! 合成器把绘制命令录制到这个命令目标而不是实时表现 surface：每个视觉
//! 操作要么降级为已证明的原生操作，要么光栅化进一个透明 CPU 分段。Picture
//! 离屏在前 BGRA 内存预算内保留同样的 API 中立流；超预算时由 unsafe 映射
//! 把该流惰性物化为一次有序图片 blit。
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
// SrcOver 矩形与字形的原生准入集中在独立 shape 契约组件。
pub(crate) mod canvas_shape;
pub(crate) mod geometry;
pub(crate) mod offscreen;

use self::offscreen::RecordedPicturePayload;
pub(crate) use offscreen::{ActiveOffscreen, RecordedPicture, RecordedPicturePool};

use crate::core::{DamageRegion, Errc, Error, Rect};
use crate::draw::geometry::types::ImageHandle;
use crate::draw::outcome::RenderOutcome;
use crate::draw::painting::{
    EncodedFrameExecution, EncodedPictureExecution, FrameEncoder, FrameImage, FrameOpacity,
    FrameRect, FrameSampledRect,
};
use crate::draw::{Canvas2D, GraphicsCapabilities, RenderTarget, UpdateStrategy};

use self::canvas::FrameRecordingCanvas;

/// 录制一个有序主帧的命令录制器：持有主画布、离屏池与当前活动离屏。
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
    /// 创建空录制器（1×1 画布，尺寸在首次 resize 时确定）。
    pub(crate) fn new() -> Self {
        Self {
            canvas: FrameRecordingCanvas::new(1, 1),
            offscreens: RecordedPicturePool::new(),
            active_offscreen: None,
        }
    }

    /// 开始一帧新的主帧录制。
    ///
    /// `clear_target`：全帧录制发射初始 Clear，让 `execute_into_pixels`
    /// 整体替换 CPU/GPU 目标；脏帧省略 Clear——真实 surface 已只清除过
    /// damage AABB，未损坏像素必须保留。
    pub(crate) fn begin_recording(&mut self, clear_target: bool) -> Result<(), Error> {
        if self.active_offscreen.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "main FrameEncoder recording cannot begin while a Picture target is active",
            ));
        }
        self.canvas.begin_recording(clear_target)
    }

    /// 结束录制：flush 剩余 scratch 并交出完整的 `FrameEncoder`。
    pub(crate) fn finish_recording(&mut self) -> Result<FrameEncoder, Error> {
        if self.active_offscreen.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "FrameEncoder recording ended while a Picture target was still active",
            ));
        }
        self.canvas.finish_recording()
    }

    /// 收回目标已同步消费的主帧命令缓冲，供下一帧复用其 Vec 容量。
    pub(crate) fn recycle_frame_encoder(&mut self, encoder: FrameEncoder) {
        if self.active_offscreen.is_none() {
            self.canvas.recycle_encoder(encoder);
        }
    }

    /// 在整帧录制开始时恢复一帧不可变的主 surface 快照。图片按 painter
    /// 顺序录制，后续根级覆盖层会合成在干净背景之上。
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

    /// 录制主画布上的 Picture blit：可 splice 时复用命令流，否则物化为图片。
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

    /// 尝试把 Picture 的已提交命令流平移后直接拼入目标画布；不可 splice
    /// 时返回 None（调用方改走物化图片）。
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

    /// 标记当前活动离屏为失败，便于结束时放弃其录制。
    fn mark_active_failed(&mut self) {
        if let Some(active) = self.active_offscreen.as_mut() {
            active.failed = true;
        }
    }
}

impl RenderTarget for CommandRecorder {
    /// 初始化目标尺寸（等价于 resize）。
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.resize(width, height)
    }

    /// 关闭录制器：清空离屏池并缩回最小画布。
    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.active_offscreen = None;
        self.offscreens.clear();
        self.canvas.commit_resize(1, 1);
        Ok(())
    }

    /// 调整主画布尺寸（含 1×1 下限校验）。
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let (width, height) = FrameRecordingCanvas::prepare_resize(width, height)?;
        self.canvas.commit_resize(width, height);
        Ok(())
    }

    /// 录制器不参与帧级呈现调度：每帧开始时都视为需要全量重绘。
    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        RenderOutcome::FrameReady(DamageRegion::full())
    }

    /// 录制器不能呈现最终帧（由合成器 / 后端呈现）。
    fn end_frame(&mut self, _present_damage: &DamageRegion) -> RenderOutcome {
        RenderOutcome::Failed(crate::draw::outcome::GraphicsFailure::from_error(
            Error::new(Errc::InvalidState, "CommandRecorder cannot present a frame"),
        ))
    }

    /// 暴露录制画布为 `Canvas2D`。
    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    /// 录制器自带离屏池，报告后端托管能力。
    fn capabilities(&self) -> GraphicsCapabilities {
        GraphicsCapabilities::backend_managed_with_offscreen()
    }

    // 让 recorder Picture 像素分配错误进入同一 typed 失败链。
    fn try_create_offscreen(
        // 借用 recorder 的唯一可变 owner。
        &mut self,
        // 接收 Picture 的逻辑宽度。
        width: i32,
        // 接收 Picture 的逻辑高度。
        height: i32,
        // 返回正常无资源、成功 handle 或 typed 分配失败。
    ) -> Result<Option<ImageHandle>, Error> {
        // 复用 CPU 离屏池的检查式创建边界。
        self.offscreens.try_create(width, height)
    }

    // recorder 释放不触碰原生 API，但仍显式闭合资源事务。
    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        if self
            .active_offscreen
            .is_some_and(|active| active.handle == handle)
        {
            self.active_offscreen = None;
        }
        self.offscreens.destroy(handle);
        self.offscreens.compact();
        // 槽位回收完成后再报告释放成功。
        Ok(())
    }

    /// 取离屏 Picture 的画布（仅录制语义）。
    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.offscreens
            .get_mut(handle)
            .map(|picture| &mut picture.canvas as &mut dyn Canvas2D)
    }

    /// 读取离屏 Picture 的参考像素（测试 / 诊断）。
    fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.offscreens.get(handle)?.copy_pixels()
    }

    /// 把编码好的命令流执行进活动 Picture 目标（校验 handle、尺寸与 splice）。
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

    /// 录制器不执行最终帧。
    fn try_execute_encoded_frame(
        &mut self,
        _encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        Err(Error::new(
            Errc::InvalidState,
            "CommandRecorder cannot execute a final frame",
        ))
    }

    /// 开始离屏 Picture 绘制：进入活动目标并开始其录制。
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

    /// 冲刷离屏录制（不结束录制，供中途取像素等场景）。
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

    /// 结束离屏绘制：成功则提交编码器，失败则放弃录制。
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

    /// 离屏 src 到目标（主画布或另一离屏）的 blit。
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

    /// 录制器内存占用：离屏池与 scratch 之和。
    fn memory_usage(&self) -> usize {
        self.offscreens
            .memory_usage()
            .saturating_add(self.canvas.scratch.memory_usage())
    }
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../../tests-src/draw/painting/recorder/mod_tests.rs"]
mod mod_tests;