//! 渲染后端 trait — 仅负责 surface 与像素提交，不含帧调度逻辑。

use std::any::Any;

use crate::core::{DamageRegion, Error, Point, Rect, Size};

use crate::draw::ImageHandle;
use crate::draw::painting::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::{Canvas2D, PresentationMode};
use crate::platform::presentation::rhi::PresentTestResult;

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
    /// 像素提交由后端还是外部 presenter 管理。
    pub presentation_mode: PresentationMode,
    /// 后端是否能只重绘并提交损坏区域。
    pub partial_redraw: bool,
    /// 后端是否拥有可作为 Picture 目标的离屏表面。
    pub offscreen: bool,
    /// 主表面是否支持安全的滚动区域像素搬移。
    pub scroll_memmove: bool,
}

/// `test-harness` 与 Agent 截屏可观察的规范 surface 像素快照。
///
/// 像素按左上原点、从上到下逐行紧密排列，每个值使用 `0xAARRGGBB`。
#[cfg(any(feature = "test-harness", feature = "agent-control"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceReadback {
    /// 快照的物理像素宽度。
    pub width: i32,
    /// 快照的物理像素高度。
    pub height: i32,
    /// 规范化后的 `0xAARRGGBB` 像素。
    pub pixels: Vec<u32>,
    /// 当前回读帧在共享 FramePlan 中实际执行的重叠安全纹理移动次数。
    pub executed_texture_moves: usize,
}

// 只允许 Drawing backend 从已经验证的规范载荷构造快照。
#[cfg(any(feature = "test-harness", feature = "agent-control"))]
impl SurfaceReadback {
    // 保存 owner-thread backend 已验证的完整 surface 结果。
    pub(crate) fn from_argb(width: i32, height: i32, pixels: Vec<u32>) -> Self {
        // RHI 已在进入本契约前验证正尺寸与精确载荷长度。
        debug_assert!(width > 0 && height > 0);
        // 载荷必须与紧密排列的完整 surface 尺寸一致。
        debug_assert_eq!(
            pixels.len(),
            (width as usize).saturating_mul(height as usize)
        );
        // 返回不携带任何原生 API 类型的 Drawing 快照。
        Self {
            // 保留物理像素宽度。
            width,
            // 保留物理像素高度。
            height,
            // 转移规范像素所有权。
            pixels,
            // 普通构造尚未附加当前帧执行证据。
            executed_texture_moves: 0,
        }
    }

    // 附加 API 无关的当前帧执行证据，不暴露任何原生对象或命令类型。
    pub(crate) fn with_executed_texture_moves(mut self, count: usize) -> Self {
        // 保存由通用 GPU backend 在成功 FramePlan 后统计的真实移动次数。
        self.executed_texture_moves = count;
        // 返回仍只包含 Drawing 规范值的完整快照。
        self
    }
}

impl BackendCapabilities {
    /// 返回软件栅格后端支持的默认能力集合。
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
    /// 返回当前可绘制表面的像素尺寸。
    fn size(&self) -> Size;
    /// 返回表面的整数像素宽度。
    fn width(&self) -> i32 {
        self.size().w as i32
    }
    /// 返回表面的整数像素高度。
    fn height(&self) -> i32 {
        self.size().h as i32
    }

    /// 将矩形裁剪压入表面裁剪栈。
    fn push_clip(&mut self, rect: Rect);
    /// 弹出最近压入的表面裁剪。
    fn pop_clip(&mut self);

    /// 清除整个表面的全部像素。
    fn clear_all(&mut self);
    /// 使用整数像素坐标清除指定矩形区域。
    fn clear_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32);

    /// 将源矩形像素复制到目标左上角，允许同一表面内重叠。
    fn copy_region(&mut self, src: Rect, dst: Point);

    /// 返回绑定当前表面的立即模式二维画布借用。
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
    /// 返回后端偏好的软件或 GPU 栅格路径。
    fn kind(&self) -> BackendKind;
    /// 返回后端当前实现保证的 surface 与提交能力。
    fn capabilities(&self) -> BackendCapabilities;

    /// 调整后端拥有的主表面尺寸。
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

    /// 返回后端拥有的主绘制表面可变借用。
    fn surface(&mut self) -> &mut dyn DrawSurface;

    /// 在一帧开始前准备后端自有状态。
    /// 原生 GPU 后端把该 hook 映射到薄 RHI 设备维护，
    /// 不向 renderer 暴露平台 current-context 操作。
    fn prepare_frame(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// 返回逻辑像素到设备像素的当前缩放倍率。
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

    /// 返回指定离屏资源绑定的立即模式画布；不支持时返回空值。
    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let _ = handle;
        None
    }
    /// 复制离屏资源像素并返回像素数组与每行像素跨度。
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

    /// 提交损坏区域覆盖的主表面内容。
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

    /// 安排在本次最终 surface composite 提交后、present 前执行一次回读。
    #[cfg(any(feature = "test-harness", feature = "agent-control"))]
    fn request_surface_readback_for_test(&mut self) -> Result<(), Error> {
        // 未接入可选能力的 backend 必须明确失败，不能保留悬挂请求。
        Err(Error::new(
            // 使用稳定的能力缺失分类。
            crate::core::Errc::NotImplemented,
            // 诊断不泄漏任何具体图形 API。
            "render backend does not support surface readback",
        ))
    }

    /// 在最终 present 返回后消费同一事务产生的规范回读结果。
    #[cfg(any(feature = "test-harness", feature = "agent-control"))]
    fn take_surface_readback_for_test(&mut self) -> Result<SurfaceReadback, Error> {
        // 未接入可选能力的 backend 不得伪造空快照。
        Err(Error::new(
            // 使用稳定的能力缺失分类。
            crate::core::Errc::NotImplemented,
            // 诊断不泄漏任何具体图形 API。
            "render backend does not expose a completed surface readback",
        ))
    }

    /// 用于唯一 Renderer 查询具体后端的内部能力。
    fn as_any(&self) -> &dyn Any;
    /// 返回用于唯一 Renderer 下行查询具体后端的可变类型擦除借用。
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
