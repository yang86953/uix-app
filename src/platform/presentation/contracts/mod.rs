//! Presentation contracts for CPU presenters and GPU graphics contexts.

use crate::core::error::{Error, Result};
pub(crate) use crate::core::{PresentCoherency, PresentDamage, PresentImage, PresentSurface};
use std::fmt;
use std::str::FromStr;

/// Result of a non-presenting availability test while a swapchain is idle.
///
/// This probe is not an entry detector: callers invoke it only after a normal
/// present reported that the window was occluded. No frame data is submitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresentTestResult {
    Presentable,
    Occluded,
}

/// Validates a CPU pixel payload before it crosses a native presentation
/// boundary.  A short slice must be a typed error: native image constructors
/// cannot infer the intended row layout safely from missing pixels.
// 所有 CPU 像素上传 presenter（Wayland SHM / macOS CALayer / Windows GDI）
// 共用同一校验强度；短缓冲一律 typed error，禁止静默截断。
pub(crate) fn validate_pixel_buffer(pixels: &[u32], width: i32, height: i32) -> Result<(), Error> {
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

/// Raster axis — how Canvas2D content is produced ([架构 · 图形](docs/架构.md#图形-api与帧提交硬约束)).
///
/// Orthogonal to [`PresentMode`] and [`GraphicsApi`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RasterMode {
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
pub(crate) enum PresentMode {
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

/// Native-side capability snapshot assembled by an adapter creation boundary.
///
/// Does not replace draw's `GraphicsCapabilities`. Engine dispatch uses
/// `raster` × `present` (× `backend` for GPU raster pairing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GraphicsContextCaps {
    pub(crate) backend: GraphicsApi,
    pub(crate) raster: RasterMode,
    pub(crate) present: PresentMode,
    /// Typed proof controlling partial redraw and present damage.
    pub(crate) present_coherency: PresentCoherency,
}

impl GraphicsContextCaps {
    /// Legal combo: [`RasterMode::GpuNative`] × [`PresentMode::Swapchain`].
    // 仅在 GPU-native backend、测试或显式测试门面需要时编译该构造器。
    #[cfg(any(
        test,
        feature = "test-harness",
        all(windows, any(feature = "d3d11", feature = "d3d12")),
        feature = "vulkan",
        feature = "opengles",
        all(target_os = "macos", feature = "metal")
    ))]
    pub(crate) fn gpu_native_swapchain(
        backend: GraphicsApi,
        present_coherency: PresentCoherency,
    ) -> Self {
        Self {
            backend,
            raster: RasterMode::GpuNative,
            present: PresentMode::Swapchain,
            present_coherency,
        }
    }

    /// Legal combo: [`RasterMode::Cpu`] × [`PresentMode::PixelUpload`].
    // CPU PixelUpload recipe 目前没有生产 registry 消费者，只保留内部测试构造入口。
    #[cfg(test)]
    pub(crate) fn cpu_pixel_upload(backend: GraphicsApi) -> Self {
        Self {
            backend,
            raster: RasterMode::Cpu,
            present: PresentMode::PixelUpload,
            present_coherency: PresentCoherency::FullOnly,
        }
    }
}

/// 原生工厂内部使用的具体 GPU API 身份。
///
/// 该类型只承载 registry 与运行时诊断事实，不属于公开选择面，也不包含自动或回退策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GraphicsApi {
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

/// Opaque, thread-affine native surface handle used for graphics assembly.
///
/// Platform windows create the underlying pointer. Crossing that pointer into
/// the graphics lifecycle requires this explicit unsafe conversion, and the
/// marker prevents safe transfer to another thread.
///
/// ```compile_fail
/// use uix::platform::presentation::NativeSurfaceHandle;
///
/// fn needs_send<T: Send>(_value: T) {}
///
/// let handle = unsafe { NativeSurfaceHandle::from_raw(std::ptr::null_mut()) };
/// needs_send(handle);
/// ```
#[derive(Clone, Copy)]
pub(crate) struct NativeSurfaceHandle {
    raw: *mut std::ffi::c_void,
    _thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl NativeSurfaceHandle {
    /// # Safety
    ///
    /// `raw` must remain a valid native surface for the whole graphics
    /// assembly/recovery use, and the handle must stay on its creating thread.
    pub(crate) unsafe fn from_raw(raw: *mut std::ffi::c_void) -> Self {
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

// 生产 GPU recipe 通过构造期验证的窄 owner 进入 draw backend。
mod gpu_recipe_owner;

// CPU PixelUpload recipe 通过构造期验证的窄 owner 进入 renderer presentation。
mod pixel_upload_recipe_owner;

// 所有 native context 在离开 factory 前收敛为已验证 recipe owner。
mod recipe_owner;

// recipe 专用 context 与共享生命周期契约。
mod traits;

// 图形 context 生命周期与 recipe 专用呈现 SPI 只供 crate 内部 backend 与 bootstrap 使用。
pub(crate) use self::traits::{GpuRecipeContext, GraphicsContextLifecycle, PixelUploadSurface};

// 保存 adapter 创建层已经确定类型的唯一 recipe context。
#[allow(dead_code)]
pub(crate) enum GraphicsRecipeContext {
    // GPU-native recipe 直接持有不可选的 thin RHI 生命周期契约。
    Gpu(Box<dyn GpuRecipeContext>),
    // CPU PixelUpload recipe 直接持有不可选的上传 surface 契约。
    PixelUpload(Box<dyn PixelUploadSurface>),
}

// 提供跨 recipe 共用且不降级为可选视图的生命周期操作。
impl GraphicsRecipeContext {
    // 检查式关闭当前 recipe 唯一持有的原生资源。
    pub(crate) fn try_shutdown(&mut self) -> crate::core::Result<()> {
        // 按构造期确定的 recipe 委托给唯一具体 owner。
        match self {
            // GPU recipe 关闭其线程亲和资源。
            Self::Gpu(context) => context.try_shutdown(),
            // PixelUpload recipe 关闭其线程亲和资源。
            Self::PixelUpload(context) => context.try_shutdown(),
        }
    }
}

// 保存 adapter 创建层尚未通过 registry row 校验的 context 与静态 capability。
pub(crate) struct GraphicsContextCandidate {
    // 持有仍由 native factory 路由消费的类型化 recipe context。
    context: GraphicsRecipeContext,
    // 持有 adapter 创建层一次组装的静态 capability 快照。
    caps: GraphicsContextCaps,
}

// 提供 candidate 的唯一构造与所有权转移边界。
impl GraphicsContextCandidate {
    // 从同一 adapter 创建事务组装 GPU context 与 capability。
    pub(crate) fn gpu(
        // 接收新创建且尚未通过 registry row 校验的 GPU context。
        context: Box<dyn GpuRecipeContext>,
        // 接收与该 context 同源的静态 capability 快照。
        caps: GraphicsContextCaps,
    ) -> Self {
        // 保存不可拆分且类型已确定的 GPU 候选记录。
        Self {
            // 把具体 owner 固化为 GPU recipe 分支。
            context: GraphicsRecipeContext::Gpu(context),
            // 保存同源 capability 快照。
            caps,
        }
    }

    // 从同一 adapter 创建事务组装 PixelUpload context 与 capability。
    #[allow(dead_code)]
    pub(crate) fn pixel_upload(
        // 接收新创建且尚未通过 registry row 校验的 PixelUpload context。
        context: Box<dyn PixelUploadSurface>,
        // 接收与该 context 同源的静态 capability 快照。
        caps: GraphicsContextCaps,
    ) -> Self {
        // 保存不可拆分且类型已确定的 PixelUpload 候选记录。
        Self {
            // 把具体 owner 固化为 PixelUpload recipe 分支。
            context: GraphicsRecipeContext::PixelUpload(context),
            // 保存同源 capability 快照。
            caps,
        }
    }

    // 把 candidate 所有权一次性交给 registry 校验层。
    pub(crate) fn into_parts(self) -> (GraphicsRecipeContext, GraphicsContextCaps) {
        // 返回同一创建事务产生的 context 与 capability。
        (self.context, self.caps)
    }
}

// 共享 RHI resize 只向实际 GPU-native backend 与内部测试重导出。
#[cfg(any(
    test,
    all(windows, feature = "d3d11"),
    feature = "vulkan",
    feature = "opengles"
))]
pub(crate) use self::traits::resize_native_rhi_surface;
// 只向 crate 内图形装配与 backend 暴露已验证 GPU owner。
pub(crate) use gpu_recipe_owner::GpuRecipeOwner;
// 只向 crate 内 renderer 暴露已验证 PixelUpload owner。
pub(crate) use pixel_upload_recipe_owner::PixelUploadRecipeOwner;
// 只向 crate 内 factory 与 renderer 暴露正交 recipe owner。
pub(crate) use recipe_owner::GraphicsRecipeOwner;
