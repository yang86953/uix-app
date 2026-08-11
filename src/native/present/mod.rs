//! Presentation contracts for CPU presenters and GPU graphics contexts.

use crate::core::error::{Error, Result};
pub use crate::core::{PresentCoherency, PresentDamage, PresentImage, PresentSurface};
use std::fmt;
use std::str::FromStr;

/// Result of a non-presenting availability test while a swapchain is idle.
///
/// This probe is not an entry detector: callers invoke it only after a normal
/// present reported that the window was occluded. No frame data is submitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentTestResult {
    Presentable,
    Occluded,
}

/// 呈现侧可提供的逐窗遮挡进入与退出能力。
///
/// 该能力不包含隐藏、最小化或 zero extent；这些状态始终由窗口生命周期管理。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresentOcclusionSupport {
    /// 当前 recipe 无可靠逐窗遮挡 API，只能依赖窗口生命周期休眠。
    #[default]
    Unsupported,
    /// 正常 present 报告进入遮挡，并支持无帧数据的 `test_present` 退出探测。
    // 只有 Windows D3D11 adapter 当前实现了 status 与无数据退出探测。
    #[cfg(all(windows, feature = "d3d11"))]
    PresentStatusAndTest,
}

impl fmt::Display for PresentOcclusionSupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unsupported => "unsupported",
            // 与变体使用同一构建边界，避免无 D3D11 时保留不可达分支。
            #[cfg(all(windows, feature = "d3d11"))]
            Self::PresentStatusAndTest => "present_status_and_test",
        })
    }
}

/// Validates a CPU pixel payload before it crosses a native presentation
/// boundary.  A short slice must be a typed error: native image constructors
/// cannot infer the intended row layout safely from missing pixels.
// 该校验只被 Unix 像素上传 presenter 使用，Windows GPU 路径不编译此入口。
#[cfg(unix)]
pub fn validate_pixel_buffer(pixels: &[u32], width: i32, height: i32) -> Result<(), Error> {
    if width <= 0 || height <= 0 {
        return Err(Error::new(
            crate::core::error::Errc::InvalidArgument,
            format!("pixel buffer extent must be positive, got {width}x{height}"),
        ));
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| {
            Error::new(
                crate::core::error::Errc::InvalidArgument,
                format!("pixel buffer extent overflows usize: {width}x{height}"),
            )
        })?;
    if pixels.len() < expected {
        return Err(Error::new(
            crate::core::error::Errc::InvalidArgument,
            format!(
                "pixel buffer too small, got {} pixels for {width}x{height}, need {expected}",
                pixels.len()
            ),
        ));
    }
    Ok(())
}

/// CPU pixel presenter.
pub trait IPresenter {
    /// Preservation proof used to gate narrow compositor damage.
    fn present_coherency(&self) -> PresentCoherency {
        PresentCoherency::FullOnly
    }

    /// Current target metadata. A presenter that rebuilds at the same extent
    /// must override this method with a monotonically changing generation.
    fn present_surface(
        &self,
        drawable_width: i32,
        drawable_height: i32,
        device_pixel_ratio: f32,
    ) -> PresentSurface {
        PresentSurface::identity(drawable_width, drawable_height, device_pixel_ratio, 0)
    }

    /// Acquired image identity for [`PresentCoherency::TrackedSwapchain`].
    fn present_image(&self) -> Option<PresentImage> {
        None
    }

    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<(), Error>;

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
}

/// Raster axis — how Canvas2D content is produced ([架构 · 图形](docs/架构.md#图形-api与帧提交硬约束)).
///
/// Orthogonal to [`PresentMode`] and [`GraphicsApi`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RasterMode {
    /// CPU Canvas2D (`CpuBackend`).
    Cpu,
    /// GPU-native raster (`RenderBackend` via `RenderBackendRegistry`).
    GpuNative,
}

impl fmt::Display for RasterMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cpu => "cpu",
            Self::GpuNative => "gpu_native",
        })
    }
}

/// Present axis — how pixels reach the screen ([架构 · 图形](docs/架构.md#图形-api与帧提交硬约束)).
///
/// Orthogonal to [`RasterMode`] and [`GraphicsApi`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresentMode {
    /// GPU swapchain / equivalent via thin RHI or a dedicated external presenter view.
    Swapchain,
    /// CPU pixels uploaded via the dedicated [`PixelUploadSurface`] contract.
    PixelUpload,
}

impl fmt::Display for PresentMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Swapchain => "swapchain",
            Self::PixelUpload => "pixel_upload",
        })
    }
}

/// Native-side capability snapshot for a live [`IGraphicsContext`].
///
/// Does not replace draw's `GraphicsCapabilities`. Engine dispatch uses
/// `raster` × `present` (× `backend` for GPU raster pairing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphicsContextCaps {
    pub backend: GraphicsApi,
    pub raster: RasterMode,
    pub present: PresentMode,
    /// 当前 live recipe 的逐窗呈现遮挡能力。
    pub present_occlusion: PresentOcclusionSupport,
    /// Typed proof controlling partial redraw and present damage.
    pub present_coherency: PresentCoherency,
}

impl GraphicsContextCaps {
    /// Legal combo: [`RasterMode::GpuNative`] × [`PresentMode::Swapchain`].
    // 仅在 GPU-native backend、测试或显式测试门面需要时编译该构造器。
    #[cfg(any(
        test,
        feature = "test-harness",
        all(windows, any(feature = "d3d11", feature = "d3d12")),
        feature = "opengles"
    ))]
    pub fn gpu_native_swapchain(backend: GraphicsApi, present_coherency: PresentCoherency) -> Self {
        Self {
            backend,
            raster: RasterMode::GpuNative,
            present: PresentMode::Swapchain,
            present_occlusion: PresentOcclusionSupport::Unsupported,
            present_coherency,
        }
    }

    /// Legal combo: [`RasterMode::Cpu`] × [`PresentMode::PixelUpload`].
    // 仅 Vulkan、Metal 与内部测试需要构造 CPU PixelUpload recipe 能力。
    #[cfg(any(test, feature = "vulkan", feature = "metal"))]
    pub fn cpu_pixel_upload(backend: GraphicsApi) -> Self {
        Self {
            backend,
            raster: RasterMode::Cpu,
            present: PresentMode::PixelUpload,
            present_occlusion: PresentOcclusionSupport::Unsupported,
            present_coherency: PresentCoherency::FullOnly,
        }
    }

    /// 为具备可靠 present-status 入口与无数据退出探测的 context 提升能力。
    // 只有 Windows D3D11 可把默认遮挡能力提升为 status-and-test。
    #[cfg(all(windows, feature = "d3d11"))]
    pub const fn with_present_occlusion(mut self, support: PresentOcclusionSupport) -> Self {
        self.present_occlusion = support;
        self
    }
}

/// 原生工厂内部使用的具体 GPU API 身份。
///
/// 该类型只承载 registry 与运行时诊断事实，不属于公开选择面，也不包含自动或回退策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicsApi {
    /// Direct3D 12 的内部 registry 身份。
    D3d12,
    /// Direct3D 11 的内部 registry 身份。
    D3d11,
    /// Vulkan 的内部 registry 身份。
    Vulkan,
    /// Metal 的内部 registry 身份。
    Metal,
    /// OpenGL ES 的内部 registry 身份。
    OpenGlEs,
}

impl GraphicsApi {
    /// 返回稳定的内部诊断名称。
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::D3d12 => "d3d12",
            Self::D3d11 => "d3d11",
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::OpenGlEs => "opengles",
        }
    }
}

impl fmt::Display for GraphicsApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 原生启动阶段的私有图形选择策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphicsSelection {
    /// 未指定具体 API 时，按 registry 的 Active recipe 顺序自动探测。
    Automatic,
    /// 显式请求一个具体 API，失败时不得偷换为其他 GPU API。
    Explicit(GraphicsApi),
}

impl fmt::Display for GraphicsSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // 自动策略使用稳定名称写入诊断。
            Self::Automatic => f.write_str("auto"),
            // 显式策略沿用具体 API 的稳定名称。
            Self::Explicit(api) => fmt::Display::fmt(api, f),
        }
    }
}

impl FromStr for GraphicsSelection {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // 配置输入统一忽略大小写与常见分隔符。
        let normalized = value
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', ' '], "");
        match normalized.as_str() {
            // 空值与 auto 只构造私有自动策略。
            "" | "auto" => Ok(Self::Automatic),
            // D3D12 别名统一映射到显式具体 API。
            "d3d12" | "direct3d12" | "directx12" => Ok(Self::Explicit(GraphicsApi::D3d12)),
            // D3D11 别名统一映射到显式具体 API。
            "d3d11" | "direct3d11" | "directx11" => Ok(Self::Explicit(GraphicsApi::D3d11)),
            // Vulkan 别名统一映射到显式具体 API。
            "vulkan" | "vk" => Ok(Self::Explicit(GraphicsApi::Vulkan)),
            // Metal 配置统一映射到显式具体 API。
            "metal" => Ok(Self::Explicit(GraphicsApi::Metal)),
            // OpenGL ES 别名统一映射到显式具体 API。
            "opengles" | "gles" | "gl" => Ok(Self::Explicit(GraphicsApi::OpenGlEs)),
            // 未知名称保持 typed 配置错误。
            _ => Err(Error::new(
                crate::core::error::Errc::InvalidArgument,
                format!("unknown graphics backend: {value}"),
            )),
        }
    }
}

#[cfg(test)]
mod graphics_selection_tests {
    // 复用被测私有选择策略与具体 API 身份。
    use super::{GraphicsApi, GraphicsSelection};

    #[test]
    // 验证自动配置不会重新进入具体 API 枚举。
    fn automatic_config_parses_as_private_selection_strategy() {
        // 空配置保持历史上的自动选择语义。
        assert_eq!("".parse(), Ok(GraphicsSelection::Automatic));
        // 显式 auto 文本同样只构造私有策略。
        assert_eq!("auto".parse(), Ok(GraphicsSelection::Automatic));
        // 自动策略诊断名称保持稳定。
        assert_eq!(GraphicsSelection::Automatic.to_string(), "auto");
    }

    #[test]
    // 验证具体配置只产生显式 API 请求。
    fn concrete_config_parses_as_explicit_api_selection() {
        // 常见 D3D11 别名归一为同一个具体 API。
        assert_eq!(
            "Direct3D-11".parse(),
            Ok(GraphicsSelection::Explicit(GraphicsApi::D3d11))
        );
        // OpenGL ES 简写归一为同一个具体 API。
        assert_eq!(
            "gles".parse(),
            Ok(GraphicsSelection::Explicit(GraphicsApi::OpenGlEs))
        );
        // 显式策略诊断只展示所请求的具体 API。
        assert_eq!(
            GraphicsSelection::Explicit(GraphicsApi::OpenGlEs).to_string(),
            "opengles"
        );
    }
}

/// Opaque, thread-affine native surface handle used for graphics assembly.
///
/// Platform windows create the underlying pointer. Crossing that pointer into
/// the graphics lifecycle requires this explicit unsafe conversion, and the
/// marker prevents safe transfer to another thread.
///
/// ```compile_fail
/// use uix::native::present::NativeSurfaceHandle;
///
/// fn needs_send<T: Send>(_value: T) {}
///
/// let handle = unsafe { NativeSurfaceHandle::from_raw(std::ptr::null_mut()) };
/// needs_send(handle);
/// ```
#[derive(Clone, Copy)]
pub struct NativeSurfaceHandle {
    raw: *mut std::ffi::c_void,
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl NativeSurfaceHandle {
    /// # Safety
    ///
    /// `raw` must remain a valid native surface for the whole graphics
    /// assembly/recovery use, and the handle must stay on its creating thread.
    pub unsafe fn from_raw(raw: *mut std::ffi::c_void) -> Self {
        Self {
            raw,
            _thread_bound: std::marker::PhantomData,
        }
    }

    pub(crate) fn as_raw(self) -> *mut std::ffi::c_void {
        self.raw
    }
}

impl std::fmt::Debug for NativeSurfaceHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeSurfaceHandle")
            .field("is_null", &self.raw.is_null())
            .finish()
    }
}

/// GPU graphics context lifecycle and presentation contract.
// 薄 RHI 作为迁移期 platform 私有契约，不向使用方公开原生句柄。
pub(crate) mod rhi;

// 生产 GPU recipe 通过构造期验证的窄 owner 进入 draw backend。
mod gpu_recipe_owner;

// CPU PixelUpload recipe 通过构造期验证的窄 owner 进入 renderer presentation。
mod pixel_upload_recipe_owner;

// 所有 native context 在离开 factory 前收敛为已验证 recipe owner。
mod recipe_owner;

// 兼容期高层 graphics context，逐步由 `rhi` 替代。
mod traits;

// 图形 context 与 recipe 专用呈现 SPI 只供 crate 内部 backend 与 bootstrap 使用。
pub(crate) use self::traits::{GpuRecipeContext, IGraphicsContext, PixelUploadSurface};

// 保存 adapter 创建层尚未通过 registry row 校验的 context 与静态 capability。
pub(crate) struct GraphicsContextCandidate {
    // 持有仍由 native factory 路由消费的原生 context。
    context: Box<dyn IGraphicsContext>,
    // 持有 adapter 创建层一次组装的静态 capability 快照。
    caps: GraphicsContextCaps,
}

// 提供 candidate 的唯一构造与所有权转移边界。
impl GraphicsContextCandidate {
    // 从同一 adapter 创建事务组装 context 与 capability。
    pub(crate) fn new(
        // 接收新创建且尚未通过 registry row 校验的 context。
        context: Box<dyn IGraphicsContext>,
        // 接收与该 context 同源的静态 capability 快照。
        caps: GraphicsContextCaps,
    ) -> Self {
        // 保存不可拆分的候选记录。
        Self { context, caps }
    }

    // 把 candidate 所有权一次性交给 registry 校验层。
    pub(crate) fn into_parts(self) -> (Box<dyn IGraphicsContext>, GraphicsContextCaps) {
        // 返回同一创建事务产生的 context 与 capability。
        (self.context, self.caps)
    }
}

// 共享 RHI resize 只向实际 GPU-native backend 与内部测试重导出。
#[cfg(any(test, all(windows, feature = "d3d11"), feature = "opengles"))]
pub(crate) use self::traits::resize_native_rhi_surface;
// 只向 crate 内图形装配与 backend 暴露已验证 GPU owner。
pub(crate) use gpu_recipe_owner::GpuRecipeOwner;
// 只向 crate 内 renderer 暴露已验证 PixelUpload owner。
pub(crate) use pixel_upload_recipe_owner::PixelUploadRecipeOwner;
// 只向 crate 内 factory 与 renderer 暴露正交 recipe owner。
pub(crate) use recipe_owner::GraphicsRecipeOwner;
