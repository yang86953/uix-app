//! 渲染后端 trait — 仅负责 surface 与像素提交，不含帧调度逻辑。

use std::any::Any;

use crate::core::{DamageRegion, Error, Point, Rect, Size};

use crate::draw::pipeline::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::traits::{Canvas2D, PresentationMode};
use crate::draw::ImageHandle;
use crate::native::traits::present::PresentTestResult;

/// Engine-level raster preference ([架构 · 图形](docs/架构.md#图形-api与帧提交硬约束)).
///
/// Aligned with orthogonal axes: `Cpu` / `Gpu` select raster path preference;
/// concrete API is [`crate::native::traits::GraphicsBackend`]. Not a bundled
/// pipeline enum — see `RasterMode` × `PresentMode` × `GraphicsBackend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Prefer CPU software raster (`RasterMode::Cpu`).
    Cpu,
    /// Prefer GPU path (`RasterMode::GpuNative` when context supports it).
    Gpu,
    /// Auto: without a platform GPU context, equivalent to Cpu; app bootstrap
    /// owns GPU-first fallback via `bootstrap_graphics_engine`.
    Auto,
    /// Null backend for tests.
    Null,
}

/// 后端能力声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub presentation_mode: PresentationMode,
    pub partial_redraw: bool,
    pub offscreen: bool,
    pub scroll_memmove: bool,
}

impl BackendCapabilities {
    pub fn cpu() -> Self {
        Self {
            presentation_mode: PresentationMode::ExternalPresenter,
            partial_redraw: true,
            offscreen: true,
            scroll_memmove: true,
        }
    }

    /// GPU 默认能力：局部 clear / partial present；离屏由具体后端声明。
    pub fn gpu() -> Self {
        Self {
            presentation_mode: PresentationMode::EngineManaged,
            partial_redraw: true,
            offscreen: false,
            scroll_memmove: false,
        }
    }

    /// GPU 降级：不支持 partial，pipeline 会扩为全帧重绘。
    pub fn gpu_full_redraw() -> Self {
        Self {
            presentation_mode: PresentationMode::EngineManaged,
            partial_redraw: false,
            offscreen: false,
            scroll_memmove: false,
        }
    }

    /// GPU + 真正的离屏 RT/FBO（非 CPU 像素池）。
    pub fn gpu_with_offscreen() -> Self {
        Self {
            presentation_mode: PresentationMode::EngineManaged,
            partial_redraw: true,
            offscreen: true,
            scroll_memmove: false,
        }
    }

    /// GPU 全帧重绘 + 离屏 RT/FBO。
    pub fn gpu_full_redraw_with_offscreen() -> Self {
        Self {
            presentation_mode: PresentationMode::EngineManaged,
            partial_redraw: false,
            offscreen: true,
            scroll_memmove: false,
        }
    }

    pub fn null() -> Self {
        Self {
            presentation_mode: PresentationMode::ExternalPresenter,
            partial_redraw: true,
            offscreen: false,
            scroll_memmove: false,
        }
    }
}

impl From<BackendCapabilities> for crate::draw::traits::GraphicsCapabilities {
    fn from(caps: BackendCapabilities) -> Self {
        Self {
            presentation_mode: caps.presentation_mode,
            partial_redraw: caps.partial_redraw,
            offscreen: caps.offscreen,
            scroll_memmove: caps.scroll_memmove,
        }
    }
}

/// 可绘制 surface — Backend 提供，Pipeline 通过此接口写入。
pub trait DrawSurface {
    fn size(&self) -> Size;
    fn width(&self) -> i32 {
        self.size().w as i32
    }
    fn height(&self) -> i32 {
        self.size().h as i32
    }

    fn push_clip(&mut self, rect: Rect);
    fn pop_clip(&mut self);

    fn clear_all(&mut self);
    fn clear_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32);

    fn copy_region(&mut self, src: Rect, dst: Point);

    fn canvas(&mut self) -> &mut dyn Canvas2D;

    /// Canvas2D keeps its immediate-mode compatibility surface, so operations
    /// without a `Result` return channel store an error here. The sole frame
    /// boundary consumes it before reporting `Present`.
    fn take_deferred_error(&mut self) -> Option<Error> {
        None
    }
}

/// 渲染后端 — 只负责 surface 与像素提交。
///
/// Live backends may own thread-affine graphics contexts and therefore cannot
/// cross threads through safe Rust.
///
/// ```compile_fail
/// use uix::draw::backend::RenderBackend;
///
/// fn needs_send<T: Send>(_value: T) {}
///
/// fn backend_cannot_cross_threads(backend: Box<dyn RenderBackend>) {
///     needs_send(backend);
/// }
/// ```
pub trait RenderBackend {
    fn kind(&self) -> BackendKind;
    fn capabilities(&self) -> BackendCapabilities;

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;

    /// Initializes draw-owned state for a context whose factory has already
    /// created and bound its native surface. The default preserves legacy
    /// backends; GPU backends override it to avoid treating startup as a
    /// second native resize/recreate.
    fn initialize_prepared(&mut self, width: i32, height: i32) -> Result<(i32, i32), Error> {
        let width = width.max(1);
        let height = height.max(1);
        self.resize(width, height)?;
        Ok((width, height))
    }

    /// Checked teardown boundary for backend-owned surfaces and native
    /// contexts. Recovery retains the previous owner when this returns `Err`.
    fn try_shutdown(&mut self) -> Result<(), Error>;

    fn surface(&mut self) -> &mut dyn DrawSurface;

    /// Bind or begin recording against the backend's current graphics target.
    fn make_current(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn device_pixel_ratio(&self) -> f32 {
        1.0
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        let _ = (width, height);
        None
    }
    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        let _ = handle;
    }

    /// Checked destruction boundary for an offscreen target. New compositor
    /// paths must use this method so native failures retain ownership for
    /// retry instead of being converted into a legacy void operation.
    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        self.destroy_offscreen(handle);
        Ok(())
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let _ = handle;
        None
    }
    fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        let _ = handle;
        None
    }

    /// Executes a lossless API-neutral encoded Picture into an existing
    /// offscreen target. A backend that cannot prove this operation preserves
    /// the encoded order returns `Unsupported`, so compositor code retains the
    /// complete DisplayList replay rather than approximating it.
    fn try_execute_encoded_picture(
        &mut self,
        _handle: &ImageHandle,
        _encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        Ok(EncodedPictureExecution::Unsupported)
    }

    /// Executes the complete ordered main frame. This operation never owns
    /// presentation; [`RenderBackend::present`] remains the only final
    /// submission boundary.
    fn try_execute_encoded_frame(
        &mut self,
        _encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        Ok(EncodedFrameExecution::Unsupported)
    }

    /// Bind + clear offscreen for Picture rasterize. CPU: no-op success if handle valid.
    fn begin_offscreen_paint(&mut self, handle: &ImageHandle) -> bool {
        let _ = handle;
        false
    }

    /// Checked counterpart of [`Self::begin_offscreen_paint`].  New compositor
    /// code must use this boundary so an invalid target or a native bind/clear
    /// failure can abort the frame before it is reported as presented.  The
    /// bool method remains only for compatibility with existing backend tests.
    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        if self.begin_offscreen_paint(handle) {
            Ok(())
        } else {
            Err(Error::new(
                crate::core::Errc::InvalidState,
                "backend could not begin Picture offscreen paint",
            ))
        }
    }

    /// Flush pending Canvas2D ops into the bound GPU RT (CPU: no-op).
    fn flush_offscreen_paint(&mut self, handle: &ImageHandle) {
        let _ = handle;
    }

    /// Checked counterpart of [`Self::flush_offscreen_paint`].  Implementors
    /// that call native APIs override this rather than logging and continuing.
    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.flush_offscreen_paint(handle);
        Ok(())
    }

    /// Unbind offscreen; restore swapchain / main target.
    fn end_offscreen_paint(&mut self) {}

    /// Checked counterpart of [`Self::end_offscreen_paint`].
    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.end_offscreen_paint();
        Ok(())
    }

    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        self.blit_offscreen_src(
            handle,
            Rect::new(0.0, 0.0, dst_rect.w, dst_rect.h),
            dst_rect,
        );
    }

    fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        let _ = (handle, src_rect, dst_rect);
    }

    /// Checked ordered Picture boundary.  A failed blit must stop the frame;
    /// it cannot be deferred to a later final present as a successful frame.
    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        self.blit_offscreen_src(handle, src_rect, dst_rect);
        Ok(())
    }

    /// 对 Picture 离屏目标做可分离高斯模糊。默认未实现。
    ///
    /// GPU 路径须走原生 RT 模糊，禁止 PixelUpload 冒充；半径语义同 CPU
    /// `gaussian_blur`（`sigma = radius / 3`）。
    fn try_blur_offscreen(
        &mut self,
        _handle: &ImageHandle,
        _region: Rect,
        _radius: f32,
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::Errc::NotImplemented,
            "render backend does not support offscreen separable blur",
        ))
    }

    /// 捕获保留主色缓冲为 overlay 干净背景（GPU 优先，无 CPU readback）。
    fn snapshot_overlay_backdrop(&mut self) -> bool {
        false
    }

    /// 恢复 overlay 背景到主表面，并取消本帧全幅 clear。
    fn restore_overlay_backdrop(&mut self) -> bool {
        false
    }

    /// 释放 overlay 背景快照。
    fn release_overlay_backdrop(&mut self) {}

    /// 是否持有有效的 overlay 背景快照。
    fn has_overlay_backdrop(&self) -> bool {
        false
    }

    fn present(&mut self, damage: &DamageRegion) -> Result<(), Error> {
        let _ = damage;
        Ok(())
    }

    /// Non-presenting availability test used only after a normal present has
    /// reported occlusion.
    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        Err(Error::new(
            crate::core::Errc::NotImplemented,
            "render backend does not support idle present tests",
        ))
    }

    /// 用于具体后端类型的向下转型（如 GpuEngine 读回像素）。
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
