//! Presentation contracts for CPU presenters and GPU graphics contexts.

use crate::core::error::{Error, Result};
pub use crate::core::{PresentCoherency, PresentDamage, PresentImage, PresentSurface};
use std::fmt;
use std::str::FromStr;

/// Solid-color axis-aligned rect for GPU-native Canvas2D fills (#169).
///
/// Coordinates are logical (dip) top-left origin, matching Canvas2D.
/// `rgba` is straight (non-premultiplied) 0..1; `radius` is tl/tr/br/bl.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuSolidRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub rgba: [f32; 4],
    pub radius: [f32; 4],
}

/// Axis-aligned stroked rect for GPU-native Canvas2D (#169).
///
/// Same coordinate / color / radius conventions as [`GpuSolidRect`].
/// `line_width` is the full stroke width in logical pixels (centered on the
/// rect edge, matching CPU `stroke_rect` / `stroke_circle` SDF).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuStrokeRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub rgba: [f32; 4],
    pub radius: [f32; 4],
    pub line_width: f32,
}

/// Glyph coverage blit for GPU-native text (#169).
///
/// Destination is the device-space quad `corners = [TL, TR, BR, BL]`；
/// 轴对齐时与 `(x,y,w,h)` AABB 一致，旋转 / 剪切时 coverage 经仿射四边形采样。
///
/// Coverage 来源二选一：
/// - `outline_mesh`：本地像素边列表 `[ax,ay,bx,by,…]`，由共享 MSDF cover pass
///   写入 RGBA8 atlas（缩放 / 仿射 / 高 DPR 字形）；物理 1:1 UI 字不走此路径
/// - `coverage`：字体面积 coverage / soft / tofu mask → R8 atlas（含严格 GPU 物理 1:1 outline）
#[derive(Debug, Clone)]
pub struct GpuGlyphBlit {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 设备坐标四角：左上、右上、右下、左下。
    pub corners: [[f32; 2]; 4],
    pub rgba: [f32; 4],
    pub coverage: std::sync::Arc<[u8]>,
    pub cov_w: u32,
    pub cov_h: u32,
    /// NonZero 轮廓边列表（相对 glyph 本地原点）；优先于 `coverage`，走 MSDF atlas。
    pub outline_mesh: Option<std::sync::Arc<[f32]>>,
}

impl GpuGlyphBlit {
    /// 由轴对齐 AABB 构造四角（identity / 纯平移缩放常用）。
    pub fn axis_aligned_corners(x: f32, y: f32, w: f32, h: f32) -> [[f32; 2]; 4] {
        [[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
    }
}

/// Axis-aligned linear gradient fill for GPU-native Canvas2D (#169).
///
/// Matches CPU `fill_linear_gradient`: sharp rect (no corner radius),
/// straight (non-premultiplied) colors, `dir` = Horizontal/Vertical/
/// DiagonalTLBR/DiagonalBLTR as 0..=3.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuLinearGradientRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 目标矩形经当前 affine 变换后的 TL/TR/BR/BL 四角。
    pub corners: [[f32; 2]; 4],
    pub color_a: [f32; 4],
    pub color_b: [f32; 4],
    /// 0=Horizontal, 1=Vertical, 2=DiagonalTLBR, 3=DiagonalBLTR.
    pub dir: u32,
}

/// Radial gradient fill (disk) for GPU-native Canvas2D (#169).
///
/// Matches CPU `fill_radial_gradient`: center `(cx,cy)`, inner/outer radius,
/// colors lerp by distance. Pixels outside `outer_r` are discarded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuRadialGradient {
    pub cx: f32,
    pub cy: f32,
    pub inner_r: f32,
    pub outer_r: f32,
    /// 径向渐变逻辑圆盘包围矩形经当前 affine 变换后的四角。
    pub corners: [[f32; 2]; 4],
    pub color_inner: [f32; 4],
    pub color_outer: [f32; 4],
}

/// Analytically antialiased solid circular sector for GPU-native Canvas2D.
///
/// `start_angle` is normalized to `[0, TAU)` and `sweep_angle` is in
/// `(0, TAU]`, advancing clockwise in the top-left-origin canvas space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuSector {
    pub cx: f32,
    pub cy: f32,
    pub radius: f32,
    pub start_angle: f32,
    pub sweep_angle: f32,
    pub rgba: [f32; 4],
}

/// Solid-color triangle mesh for GPU-native path fills/strokes (#169).
///
/// `vertices` is an interleaved xy triangle-list in logical (dip) top-left
/// coordinates (same as Canvas2D). Tessellation is CPU-side; the GPU only
/// draws the triangles. Complex paths soft-fallback instead of using this.
#[derive(Debug, Clone)]
pub struct GpuSolidMesh {
    pub vertices: std::sync::Arc<[f32]>,
    pub rgba: [f32; 4],
}

/// Axis-aligned or affine-mapped box / drop shadow for GPU-native Canvas2D (#169).
///
/// Matches CPU `draw_box_shadow` / `draw_box_shadow_ambient`: shadow body is
/// sized `(w,h)` with corner radii；软边经 SDF。`blur_x` / `blur_y` 支持各向异性。
/// `corners` 为扩展后阴影四边形的设备坐标（TL/TR/BR/BL），可承载旋转 / 剪切；
/// 局部 SDF 仍在逻辑扩展矩形空间计算。`rgba` is straight (non-premultiplied) 0..1。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuBoxShadow {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_x: f32,
    pub blur_y: f32,
    pub rgba: [f32; 4],
    pub radius: [f32; 4],
    /// `true` → ambient (softer) coverage curve.
    pub ambient: bool,
    /// 扩展后阴影四边形设备坐标角（TL/TR/BR/BL）。
    pub corners: [[f32; 2]; 4],
}

/// BGRA image blit for GPU-native Canvas2D（支持 1:1 与缩放）。
///
/// `pixels` is a tightly cropped row-major BGRA premultiplied buffer of
/// `pixel_w * pixel_h` texels. Destination `(x,y,w,h)` is logical top-left;
/// `w`/`h` may differ from the crop size（GPU 纹理采样缩放），`x`/`y` may be
/// fractional. `opacity` is the canvas opacity already folded for SrcOver。
/// `additive` 为 true 时走通道相加（与 CPU `BlendMode::Additive` 对齐），
/// 否则 premultiplied SrcOver。
#[derive(Debug, Clone)]
pub struct GpuImageBlit {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 设备坐标四角：左上、右上、右下、左下；支持旋转与剪切。
    pub corners: [[f32; 2]; 4],
    pub opacity: f32,
    pub additive: bool,
    pub pixels: std::sync::Arc<[u32]>,
    pub pixel_w: u32,
    pub pixel_h: u32,
}

/// 通用 renderer 从薄 RHI 能力快照派生的绘制事实。
///
/// 逐图元支持由固定 RHI probe 一次性验证，不再与 `IGraphicsContext`
/// 或原生 adapter 维护平行的 capability 声明。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeRasterCaps {
    /// 主色缓冲在提交间保留像素，允许绘制侧 partial redraw；
    /// 与 present coherency（仍可为 FullOnly）正交。
    pub retained_framebuffer: bool,
    /// retained RHI 路径可执行 premultiplied Additive pipeline；
    /// 不代表 legacy `draw_*` ABI 支持 Additive。
    pub rhi_additive_blend: bool,
}

impl NativeRasterCaps {
    /// 从同一次薄 RHI 事实快照投影 renderer 真正消费的能力。
    pub(crate) const fn from_rhi_capabilities(capabilities: rhi::GraphicsCapabilities) -> Self {
        // 只复制绘制侧需要的事实，不建立第二份 adapter capability 来源。
        Self {
            // 主颜色目标的跨帧保留语义直接来自薄 RHI 快照。
            retained_framebuffer: capabilities.retained_framebuffer,
            // Additive pipeline 事实直接来自同一个薄 RHI 快照。
            rhi_additive_blend: capabilities.additive_blend,
        }
    }

    pub const fn has_gpu_only_baseline(self) -> bool {
        // 逐图元 pipeline 已由固定 probe 验证，GPU-only 只需 retained surface 事实。
        self.retained_framebuffer
    }
}

// 覆盖生产 native raster profile 的 capability 接线。
#[cfg(test)]
mod native_raster_profile_tests {
    // 导入薄 RHI 事实快照与 renderer 投影类型。
    use super::{rhi::GraphicsCapabilities, NativeRasterCaps};

    // 验证 renderer profile 只从 retained 薄 RHI 快照派生。
    #[test]
    fn renderer_profile_projects_retained_and_additive_facts() {
        // 构造 D3D11 与 OpenGL ES 生产实现共同满足的 retained RHI 快照。
        let rhi_capabilities = GraphicsCapabilities::retained_gpu_baseline();
        // 从唯一事实来源派生 renderer 使用的窄能力投影。
        let renderer_capabilities = NativeRasterCaps::from_rhi_capabilities(rhi_capabilities);
        // 投影必须保留跨帧主颜色目标事实。
        assert!(renderer_capabilities.retained_framebuffer);
        // 投影必须保留真实的 RHI Additive 能力。
        assert!(renderer_capabilities.rhi_additive_blend);
        // retained 事实必须继续满足生产 GPU-only 绘制基线。
        assert!(renderer_capabilities.has_gpu_only_baseline());
    }
}

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
    PresentStatusAndTest,
}

impl fmt::Display for PresentOcclusionSupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unsupported => "unsupported",
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

// 兼容期高层 graphics context，逐步由 `rhi` 替代。
mod traits;

// 图形 context 与 recipe 专用呈现 SPI 只供 crate 内部 backend 与 bootstrap 使用。
pub(crate) use self::traits::{
    resize_native_rhi_surface, IGraphicsContext, PixelUploadSurface, RhiSurfaceLifecycle,
};
// 只向 crate 内图形装配与 backend 暴露已验证 GPU owner。
pub(crate) use gpu_recipe_owner::GpuRecipeOwner;
// 只向 crate 内 renderer 暴露已验证 PixelUpload owner。
pub(crate) use pixel_upload_recipe_owner::PixelUploadRecipeOwner;
