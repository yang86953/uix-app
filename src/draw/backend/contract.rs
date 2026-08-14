//! 渲染后端 trait — 仅负责 surface 与像素提交，不含帧调度逻辑。

use std::any::Any;

use crate::core::{DamageRegion, Error, Point, Rect, Size};

use crate::draw::ImageHandle;
use crate::draw::painting::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::{Canvas2D, PresentationMode};
use crate::native::present::PresentTestResult;

/// Renderer backend preference ([架构 · 图形](docs/架构.md#图形-api与帧提交硬约束)).
///
/// Aligned with orthogonal axes: `Cpu` / `Gpu` select raster path preference;
/// concrete public API is [`crate::platform::graphics::GraphicsBackend`]. Not a bundled
/// pipeline enum — raster preference and concrete GPU API remain orthogonal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Prefer CPU software raster (`RasterMode::Cpu`).
    Cpu,
    /// Prefer GPU path (`RasterMode::GpuNative` when context supports it).
    Gpu,
    /// Auto: without a platform GPU context, equivalent to Cpu; app bootstrap
    /// owns GPU-first fallback via `bootstrap_renderer`.
    Auto,
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
            presentation_mode: PresentationMode::BackendManaged,
            partial_redraw: true,
            offscreen: false,
            scroll_memmove: false,
        }
    }

    /// GPU 降级：不支持 partial，pipeline 会扩为全帧重绘。
    pub fn gpu_full_redraw() -> Self {
        Self {
            presentation_mode: PresentationMode::BackendManaged,
            partial_redraw: false,
            offscreen: false,
            scroll_memmove: false,
        }
    }

    /// GPU + 真正的离屏 RT/FBO（非 CPU 像素池）。
    pub fn gpu_with_offscreen() -> Self {
        Self {
            presentation_mode: PresentationMode::BackendManaged,
            partial_redraw: true,
            offscreen: true,
            scroll_memmove: false,
        }
    }

    /// GPU 全帧重绘 + 离屏 RT/FBO。
    pub fn gpu_full_redraw_with_offscreen() -> Self {
        Self {
            presentation_mode: PresentationMode::BackendManaged,
            partial_redraw: false,
            offscreen: true,
            scroll_memmove: false,
        }
    }

    // 测试目标保留独立的最小能力配置，供后端契约测试按需构造。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn test() -> Self {
        Self {
            presentation_mode: PresentationMode::ExternalPresenter,
            partial_redraw: true,
            offscreen: false,
            scroll_memmove: false,
        }
    }
}

impl From<BackendCapabilities> for crate::draw::GraphicsCapabilities {
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

    /// 在一帧开始前准备后端自有状态。
    /// 原生 GPU 后端把该 hook 映射到薄 RHI 设备维护，
    /// 不向 renderer 暴露平台 current-context 操作。
    fn prepare_frame(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn device_pixel_ratio(&self) -> f32 {
        1.0
    }

    /// Checked creation boundary for a Picture resource.
    // 默认 backend 以 Ok(None) 明确表达不支持或不创建资源。
    fn try_create_offscreen(
        // 接收当前 backend 的唯一可变 owner。
        &mut self,
        // 接收调用方已验证或待验证的逻辑宽度。
        width: i32,
        // 接收调用方已验证或待验证的逻辑高度。
        height: i32,
        // 区分正常无资源、成功 handle 与 typed 资源失败。
    ) -> Result<Option<ImageHandle>, Error> {
        // 默认实现只消费尺寸，具体 backend 决定是否支持 Picture。
        let _ = (width, height);
        // 不支持不是资源失败，场景层可以保持无缓存路径。
        Ok(None)
    }

    /// Checked destruction boundary for a Picture resource.
    // 默认 backend 不允许把未知资源的释放伪装成成功。
    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        // 默认实现只消费 handle，具体 owner 必须显式覆盖释放语义。
        let _ = handle;
        // 缺失释放能力属于明确的契约缺口。
        Err(Error::new(
            // 使用稳定错误分类区分能力缺口与资源故障。
            crate::core::Errc::NotImplemented,
            // 保留可定位的 Picture destroy 诊断。
            "render backend does not support Picture offscreen destroy",
        ))
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

    /// Checked Picture begin boundary. Unsupported backends fail explicitly.
    // 默认 checked begin 不允许把缺失的 Picture target 伪装成成功。
    fn try_begin_offscreen_paint(
        // 接收当前 backend 的唯一可变 owner。
        &mut self,
        // 接收调用方已经创建的 Picture 目标身份。
        handle: &ImageHandle,
        // 返回 typed failure，供 renderer 恢复链处理。
    ) -> Result<(), Error> {
        // 默认实现只消费参数，具体 backend 必须显式覆盖生命周期。
        let _ = handle;
        // 缺失 Picture 生命周期属于明确的能力缺口。
        Err(Error::new(
            // 使用稳定错误分类区分能力缺口与资源故障。
            crate::core::Errc::NotImplemented,
            // 保留可定位的 Picture begin 诊断。
            "render backend does not support Picture offscreen begin",
        ))
    }

    /// Checked Picture flush boundary. Unsupported backends fail explicitly.
    // 默认 checked flush 不允许静默丢弃待提交的 Picture 绘制。
    fn try_flush_offscreen_paint(
        // 接收当前 backend 的唯一可变 owner。
        &mut self,
        // 接收必须提交的 Picture 目标身份。
        handle: &ImageHandle,
        // 返回 typed failure，阻止错误帧进入最终 present。
    ) -> Result<(), Error> {
        // 默认实现只消费参数，具体 backend 必须显式覆盖提交语义。
        let _ = handle;
        // 缺失 Picture flush 属于明确的能力缺口。
        Err(Error::new(
            // 使用稳定错误分类驱动既有恢复策略。
            crate::core::Errc::NotImplemented,
            // 保留可定位的 Picture flush 诊断。
            "render backend does not support Picture offscreen flush",
        ))
    }

    /// Checked Picture end boundary. Unsupported backends fail explicitly.
    // 默认 checked end 要求实现者显式闭合已经开始的 Picture 生命周期。
    fn try_end_offscreen_paint(
        // 接收当前 backend 的唯一可变 owner。
        &mut self,
        // 返回 typed failure，禁止绕过 target 恢复边界。
    ) -> Result<(), Error> {
        // 缺失 Picture end 属于明确的能力缺口。
        Err(Error::new(
            // 使用稳定错误分类驱动既有恢复策略。
            crate::core::Errc::NotImplemented,
            // 保留可定位的 Picture end 诊断。
            "render backend does not support Picture offscreen end",
        ))
    }

    /// Checked ordered Picture blit boundary. Unsupported backends fail explicitly.
    // 默认 checked blit 不允许把漏绘误报为成功帧。
    fn try_blit_offscreen_src(
        // 接收当前 backend 的唯一可变 owner。
        &mut self,
        // 接收稳定的 Picture 来源身份。
        handle: &ImageHandle,
        // 接收来源纹理内的裁剪区域。
        src_rect: Rect,
        // 接收当前目标内的绘制区域。
        dst_rect: Rect,
        // 返回 typed failure，阻止失败 blit 继续 present。
    ) -> Result<(), Error> {
        // 默认实现只消费参数，具体 backend 必须显式覆盖合成语义。
        let _ = (handle, src_rect, dst_rect);
        // 缺失 Picture blit 属于明确的能力缺口。
        Err(Error::new(
            // 使用稳定错误分类驱动既有恢复策略。
            crate::core::Errc::NotImplemented,
            // 保留可定位的 Picture blit 诊断。
            "render backend does not support Picture offscreen blit",
        ))
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

    /// 查询当前 backend 是否具备真实 overlay backdrop blur 事务。
    fn supports_backdrop_blur(&self) -> bool {
        // 普通 backend 默认显式降级为 mask-only。
        false
    }

    /// 捕获保留主色缓冲为 overlay 干净背景；资源失败保持 typed error。
    fn snapshot_overlay_backdrop(&mut self) -> Result<bool, Error> {
        Ok(false)
    }

    /// 对已捕获的 overlay 干净背景执行区域高斯模糊；不支持时返回 `Ok(false)`。
    fn blur_overlay_backdrop(&mut self, _region: Rect, _radius: f32) -> Result<bool, Error> {
        // 默认 backend 不拥有 GPU backdrop，保留可检测的不支持语义。
        Ok(false)
    }

    /// 恢复 overlay 背景到主表面，并取消本帧全幅 clear；提交失败保持 typed error。
    fn restore_overlay_backdrop(&mut self) -> Result<bool, Error> {
        Ok(false)
    }

    /// 检查式释放 overlay 背景快照；失败时保留 owner 供恢复或 shutdown 重试。
    fn release_overlay_backdrop(&mut self) -> Result<(), Error> {
        Ok(())
    }

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

    /// 用于唯一 Renderer 查询具体后端的内部能力。
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
