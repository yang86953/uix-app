//! Presentation contracts for CPU presenters and GPU graphics contexts.

use crate::core::error::{Error, Result};
pub use crate::core::{
    PresentCoherency, PresentDamage, PresentImage, PresentSurface, PresentTransform,
};
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
/// - `outline_mesh`：本地像素边列表 `[ax,ay,bx,by,…]`，由共享 wgpu MSDF cover pass
///   写入 RGBA8 atlas（缩放 / 仿射字形）；近 1:1 UI 字不走此路径
/// - `coverage`：解析 AA / soft / tofu mask → R8 atlas（含严格 GPU 近 1:1 outline）
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
    pub color_inner: [f32; 4],
    pub color_outer: [f32; 4],
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
    pub opacity: f32,
    pub additive: bool,
    pub pixels: std::sync::Arc<[u32]>,
    pub pixel_w: u32,
    pub pixel_h: u32,
}

/// Fine-grained native raster capabilities exposed by a GPU context.
///
/// The draw-side native backend uses this table to route every Canvas2D
/// operation either to a supported native command or to deterministic CPU
/// soft fallback. A context must not advertise an operation whose trait method
/// still returns `NotImplemented`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeRasterCaps {
    pub clear_target: bool,
    pub clear_rects: bool,
    pub soft_blit: bool,
    pub solid_rects: bool,
    pub stroke_rects: bool,
    pub glyphs: bool,
    pub linear_gradients: bool,
    pub radial_gradients: bool,
    pub solid_meshes: bool,
    pub box_shadows: bool,
    /// GPU texture RT + blit（Picture 离屏）；非 CPU 像素池。
    pub offscreen_targets: bool,
    /// 主色缓冲在提交间保留像素，允许绘制侧 partial redraw；
    /// 与 present coherency（仍可为 FullOnly）正交。
    pub retained_framebuffer: bool,
}

impl NativeRasterCaps {
    /// Shared production baseline implemented once by the wgpu renderer.
    pub const fn wgpu_full() -> Self {
        Self {
            clear_target: true,
            clear_rects: true,
            soft_blit: false,
            solid_rects: true,
            stroke_rects: true,
            glyphs: true,
            linear_gradients: true,
            radial_gradients: true,
            solid_meshes: true,
            box_shadows: true,
            offscreen_targets: true,
            // 主路径经保留色缓冲绘制，再全幅 blit 到 swapchain。
            retained_framebuffer: true,
        }
    }

    /// Complete capability set currently implemented by the D3D11 context.
    pub const fn d3d11_full() -> Self {
        Self {
            clear_target: true,
            clear_rects: true,
            soft_blit: true,
            solid_rects: true,
            stroke_rects: true,
            glyphs: true,
            linear_gradients: true,
            radial_gradients: true,
            solid_meshes: true,
            box_shadows: true,
            offscreen_targets: true,
            retained_framebuffer: false,
        }
    }

    pub const fn has_hybrid_baseline(self) -> bool {
        self.clear_target && self.soft_blit
    }

    pub const fn has_gpu_only_baseline(self) -> bool {
        self.clear_target
            && self.solid_rects
            && self.stroke_rects
            && self.glyphs
            && self.linear_gradients
            && self.radial_gradients
            && self.solid_meshes
            && self.box_shadows
    }
}

/// Opaque GPU offscreen render-target id ([`IGraphicsContext`] Picture cache).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OffscreenTargetId(pub u32);

/// Unified present payload for [`IGraphicsContext::present`] (M7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentFrame<'a> {
    /// GPU swapchain / equivalent (native raster path).
    Swapchain { damage: PresentDamage },
    /// CPU raster upload (upload-present path).
    PixelBuffer {
        pixels: &'a [u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    },
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

/// Bounded, tightly packed CPU soft-raster segment.
///
/// `pixels` passed to [`IGraphicsContext::blit_soft_fallback_tile`] contain
/// exactly this tile in top-left row-major order. The destination is separate
/// from that compact source so callers never need to retain or upload a full
/// frame merely to place one fallback segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftFallbackTile {
    pub dst_x: i32,
    pub dst_y: i32,
    pub width: i32,
    pub height: i32,
}

impl SoftFallbackTile {
    pub const fn at_destination(dst_x: i32, dst_y: i32, width: i32, height: i32) -> Self {
        Self {
            dst_x,
            dst_y,
            width,
            height,
        }
    }

    pub fn required_pixels(self) -> Option<usize> {
        if self.width <= 0 || self.height <= 0 {
            return None;
        }
        usize::try_from(i64::from(self.width) * i64::from(self.height)).ok()
    }

    pub fn validate_payload(self, pixels: &[u32]) -> Result<()> {
        if self.dst_x < 0 || self.dst_y < 0 {
            return Err(Error::new(
                crate::core::error::Errc::InvalidArgument,
                format!(
                    "soft fallback destination must be nonnegative, got {},{}",
                    self.dst_x, self.dst_y
                ),
            ));
        }
        let Some(required) = self.required_pixels() else {
            return Err(Error::new(
                crate::core::error::Errc::InvalidArgument,
                format!(
                    "soft fallback tile extent must be positive, got {}x{}",
                    self.width, self.height
                ),
            ));
        };
        if pixels.len() != required {
            return Err(Error::new(
                crate::core::error::Errc::InvalidArgument,
                format!(
                    "soft fallback tile payload has {} pixels, need {required}",
                    pixels.len()
                ),
            ));
        }
        Ok(())
    }
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
/// Orthogonal to [`PresentMode`] and [`GraphicsBackend`].
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
/// Orthogonal to [`RasterMode`] and [`GraphicsBackend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresentMode {
    /// GPU swapchain / equivalent via [`IGraphicsContext::present`].
    Swapchain,
    /// CPU pixels uploaded via [`IGraphicsContext::present`] (`PixelBuffer`).
    PixelUpload,
    /// Pure CPU + [`IPresenter`]; no [`IGraphicsContext`] (app/bootstrap only).
    CpuPresenter,
}

impl fmt::Display for PresentMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Swapchain => "swapchain",
            Self::PixelUpload => "pixel_upload",
            Self::CpuPresenter => "cpu_presenter",
        })
    }
}

/// Native-side capability snapshot for a live [`IGraphicsContext`].
///
/// Does not replace draw's `GraphicsCapabilities`. Engine dispatch uses
/// `raster` × `present` (× `backend` for GPU raster pairing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphicsContextCaps {
    pub backend: GraphicsBackend,
    pub raster: RasterMode,
    pub present: PresentMode,
    /// 当前 live recipe 的逐窗呈现遮挡能力。
    pub present_occlusion: PresentOcclusionSupport,
    /// Typed proof controlling partial redraw and present damage.
    pub present_coherency: PresentCoherency,
    pub device_pixel_ratio: f32,
}

impl GraphicsContextCaps {
    /// Legal combo: [`RasterMode::GpuNative`] × [`PresentMode::Swapchain`].
    pub fn gpu_native_swapchain(
        backend: GraphicsBackend,
        present_coherency: PresentCoherency,
        device_pixel_ratio: f32,
    ) -> Self {
        Self {
            backend,
            raster: RasterMode::GpuNative,
            present: PresentMode::Swapchain,
            present_occlusion: PresentOcclusionSupport::Unsupported,
            present_coherency,
            device_pixel_ratio,
        }
    }

    /// Legal combo: [`RasterMode::Cpu`] × [`PresentMode::PixelUpload`].
    pub fn cpu_pixel_upload(backend: GraphicsBackend, device_pixel_ratio: f32) -> Self {
        Self {
            backend,
            raster: RasterMode::Cpu,
            present: PresentMode::PixelUpload,
            present_occlusion: PresentOcclusionSupport::Unsupported,
            present_coherency: PresentCoherency::FullOnly,
            device_pixel_ratio,
        }
    }

    /// 为具备可靠 present-status 入口与无数据退出探测的 context 提升能力。
    pub const fn with_present_occlusion(mut self, support: PresentOcclusionSupport) -> Self {
        self.present_occlusion = support;
        self
    }
}

/// Concrete GPU API selected by the native factory.
///
/// This is diagnostic and init-time selection data; draw continues to expose
/// [`crate::draw::BackendKind`] as the engine-level raster preference
/// (`Gpu` ≈ try GPU path, not a specific API).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicsBackend {
    Auto,
    D3d12,
    D3d11,
    Vulkan,
    Metal,
    OpenGlEs,
}

impl GraphicsBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::D3d12 => "d3d12",
            Self::D3d11 => "d3d11",
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::OpenGlEs => "opengles",
        }
    }
}

impl fmt::Display for GraphicsBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for GraphicsBackend {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', ' '], "");
        match normalized.as_str() {
            "" | "auto" => Ok(Self::Auto),
            "d3d12" | "direct3d12" | "directx12" => Ok(Self::D3d12),
            "d3d11" | "direct3d11" | "directx11" => Ok(Self::D3d11),
            "vulkan" | "vk" => Ok(Self::Vulkan),
            "metal" => Ok(Self::Metal),
            "opengles" | "gles" | "gl" => Ok(Self::OpenGlEs),
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
/// use uix::native::traits::present::NativeSurfaceHandle;
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

    pub fn is_null(self) -> bool {
        self.raw.is_null()
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
pub trait IGraphicsContext {
    fn caps(&self) -> GraphicsContextCaps;

    /// Per-operation native raster support for `GpuNative` contexts.
    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps::default()
    }

    fn graphics_backend(&self) -> GraphicsBackend {
        self.caps().backend
    }

    fn initialize(
        &mut self,
        native_window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> Result<(), Error>;

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
    fn make_current(&mut self) -> Result<(), Error>;
    fn swap_buffers(&mut self, damage: PresentDamage) -> Result<(), Error>;

    /// Checked shutdown boundary for thread-affine native resources.
    ///
    /// Callers and Drop paths must use this method. Teardown failures stay
    /// typed so recovery can retain the previous owner instead of logging only.
    fn try_shutdown(&mut self) -> Result<(), Error>;

    /// Reads native pixels through the checked, thread-affine lifecycle
    /// boundary. Readback failure is never represented as an empty pixel
    /// buffer: callers must receive the typed error and retain recovery state.
    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>, Error>;
    fn width(&self) -> i32;
    fn height(&self) -> i32;

    /// Current drawable metadata used at the final damage conversion boundary.
    /// A non-identity or same-extent-rebuilding context must override this.
    fn present_surface(&self) -> PresentSurface {
        PresentSurface::identity(self.width(), self.height(), self.device_pixel_ratio(), 0)
    }

    /// Acquired image identity required by tracked multi-buffer presentation.
    fn present_image(&self) -> Option<PresentImage> {
        None
    }

    /// Legacy capability query retained for tests and diagnostics during the
    /// runtime-lease migration. It never exposes a raw proc loader.
    fn supports_gl_proc_address(&self) -> bool {
        self.caps().raster == RasterMode::GpuNative
            && self.caps().backend == GraphicsBackend::OpenGlEs
    }

    fn supports_pixel_present(&self) -> bool {
        self.caps().present == PresentMode::PixelUpload
    }

    fn present_pixels(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        _damage: PresentDamage,
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support CPU pixel present",
                self.graphics_backend()
            ),
        ))
    }

    /// Unified present entry (M7). Default forwards to legacy methods.
    fn present(&mut self, frame: &PresentFrame) -> Result<(), Error> {
        match frame {
            PresentFrame::Swapchain { damage } => {
                self.make_current()?;
                self.swap_buffers(damage.clone())
            }
            PresentFrame::PixelBuffer {
                pixels,
                width,
                height,
                damage,
            } => self.present_pixels(pixels, *width, *height, damage.clone()),
        }
    }

    /// Tests whether an already-occluded swapchain can leave idle state
    /// without submitting frame data. Contexts that can report occlusion from
    /// normal presentation must override this method.
    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support idle present tests",
                self.graphics_backend()
            ),
        ))
    }

    /// Drawable pixels per logical client pixel (HiDPI). Default `1.0`.
    fn device_pixel_ratio(&self) -> f32 {
        1.0
    }

    /// Clear the current GPU render target (GpuNative × Swapchain).
    ///
    /// Default: not implemented. Non-GL native contexts advertise this through
    /// [`NativeRasterCaps`].
    fn clear_render_target(&mut self, _r: f32, _g: f32, _b: f32, _a: f32) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support clear_render_target",
                self.graphics_backend()
            ),
        ))
    }

    /// Upload CPU-rasterized pixels into the GPU backbuffer without presenting.
    ///
    /// Full overwrite of the backbuffer (test / legacy soft-only path). Prefer
    /// [`Self::blit_soft_fallback`] when native geometry was already drawn.
    fn upload_surface_pixels(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support upload_surface_pixels",
                self.graphics_backend()
            ),
        ))
    }

    /// Draw solid-color (optionally rounded) quads into the current RTV.
    ///
    /// `scissor` is optional logical-pixel AABB `(x, y, w, h)` top-left origin.
    /// Used by the capability-driven native GPU backend for hot Canvas2D
    /// `fill_rect` / `fill_circle`.
    fn draw_solid_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _rects: &[GpuSolidRect],
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support draw_solid_rects",
                self.graphics_backend()
            ),
        ))
    }

    /// Draw stroked (optionally rounded) rects into the current RTV.
    ///
    /// Same scissor convention as [`Self::draw_solid_rects`].
    fn draw_stroke_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _rects: &[GpuStrokeRect],
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support draw_stroke_rects",
                self.graphics_backend()
            ),
        ))
    }

    /// Pack CPU glyph coverage into a GPU atlas and draw textured quads.
    ///
    /// Same scissor convention as [`Self::draw_solid_rects`].
    fn draw_glyphs(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _glyphs: &[GpuGlyphBlit],
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support draw_glyphs",
                self.graphics_backend()
            ),
        ))
    }

    /// Alpha-blend a CPU soft-fallback buffer over the current RTV (no present).
    ///
    /// Unsupported Canvas2D ops stay on CPU and composite over native geometry.
    /// Draw axis-aligned linear gradient rects into the current RTV.
    ///
    /// Same scissor convention as [`Self::draw_solid_rects`].
    fn draw_linear_gradients(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _rects: &[GpuLinearGradientRect],
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support draw_linear_gradients",
                self.graphics_backend()
            ),
        ))
    }

    /// Draw radial gradient disks into the current RTV.
    ///
    /// Same scissor convention as [`Self::draw_solid_rects`].
    fn draw_radial_gradients(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _grads: &[GpuRadialGradient],
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support draw_radial_gradients",
                self.graphics_backend()
            ),
        ))
    }

    /// Draw solid-color triangle meshes into the current RTV.
    ///
    /// Same scissor convention as [`Self::draw_solid_rects`]. Used for
    /// identity-transform `fill_path` / `stroke_path` when advertised.
    fn draw_solid_meshes(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _meshes: &[GpuSolidMesh],
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support draw_solid_meshes",
                self.graphics_backend()
            ),
        ))
    }

    /// Draw axis-aligned box / ambient shadows into the current RTV.
    ///
    /// Same scissor convention as [`Self::draw_solid_rects`].
    fn draw_box_shadows(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _shadows: &[GpuBoxShadow],
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support draw_box_shadows",
                self.graphics_backend()
            ),
        ))
    }

    /// Upload tightly cropped BGRA images and draw textured quads.
    ///
    /// Same scissor convention as [`Self::draw_solid_rects`]. Production wgpu
    /// implements SrcOver blits with optional destination scaling; fractional
    /// destination origin is allowed. Non-identity canvas transforms stay at
    /// the Canvas2D boundary.
    fn draw_image_blits(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _blits: &[GpuImageBlit],
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support draw_image_blits",
                self.graphics_backend()
            ),
        ))
    }

    fn blit_soft_fallback(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support blit_soft_fallback",
                self.graphics_backend()
            ),
        ))
    }

    /// Alpha-blend one bounded CPU fallback segment without presenting.
    ///
    /// The payload is tightly packed to the tile extent; the implementation
    /// must validate both its byte count and its destination against the
    /// currently bound target. The default rejects the new compact protocol
    /// rather than silently expanding it to a full texture transfer.
    fn blit_soft_fallback_tile(
        &mut self,
        _pixels: &[u32],
        tile: SoftFallbackTile,
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support compact CPU soft fallback uploads to {},{} {}x{}",
                self.graphics_backend(),
                tile.dst_x,
                tile.dst_y,
                tile.width,
                tile.height
            ),
        ))
    }

    /// Replace-blend clear of logical rects (partial dirty clear).
    ///
    /// Default: not implemented. D3D11 uses this because `ClearRenderTargetView`
    /// always clears the full RTV.
    fn clear_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _rects: &[GpuSolidRect],
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support clear_rects",
                self.graphics_backend()
            ),
        ))
    }

    /// Create a GPU offscreen color target (RTV+SRV). Default: not implemented.
    fn create_offscreen_target(
        &mut self,
        _width: i32,
        _height: i32,
    ) -> Result<OffscreenTargetId, Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support create_offscreen_target",
                self.graphics_backend()
            ),
        ))
    }

    /// Checked destruction boundary for a native offscreen target.
    fn try_destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        self.destroy_offscreen_target(id);
        Ok(())
    }

    fn destroy_offscreen_target(&mut self, _id: OffscreenTargetId) {}

    /// Bind offscreen as the current draw target (viewport = target size).
    fn bind_offscreen_target(&mut self, _id: OffscreenTargetId) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support bind_offscreen_target",
                self.graphics_backend()
            ),
        ))
    }

    /// Restore swapchain / default backbuffer as the draw target.
    fn bind_swapchain_target(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Sample offscreen SRV into the **current** RT as a textured quad.
    ///
    /// `src` / `dst` are in logical pixels (top-left origin), relative to the
    /// offscreen and current target respectively. `opacity` scales the sampled
    /// premultiplied color (group / parent canvas opacity)；values ≤ 0 are a
    /// no-op, values ≥ 1 leave the sample unchanged。`additive` 为 true 时
    /// 使用通道相加 blend（父画布 `BlendMode::Additive`），否则 SrcOver。
    fn blit_offscreen_target(
        &mut self,
        _id: OffscreenTargetId,
        _src: crate::core::Rect,
        _dst: crate::core::Rect,
        _opacity: f32,
        _additive: bool,
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support blit_offscreen_target",
                self.graphics_backend()
            ),
        ))
    }

    /// 对离屏颜色目标做可分离高斯模糊（水平→垂直；大半径可降采样）。
    ///
    /// `region` 为逻辑像素矩形；半径语义与 CPU `gaussian_blur` 一致
    ///（`sigma = radius / 3`）。默认未实现；生产 wgpu 路径提供原生实现，
    /// 禁止用 CPU PixelUpload 冒充。
    fn blur_offscreen_target(
        &mut self,
        _id: OffscreenTargetId,
        _region: crate::core::Rect,
        _radius: f32,
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support blur_offscreen_target",
                self.graphics_backend()
            ),
        ))
    }

    /// 将保留主色缓冲快照为 overlay 干净背景（GPU 纹理复制，无 CPU readback）。
    ///
    /// 仅 `retained_framebuffer` 后端可实现；默认未实现。须在 `begin_frame`
    /// 清除之前调用。
    fn snapshot_overlay_backdrop(&mut self) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support snapshot_overlay_backdrop",
                self.graphics_backend()
            ),
        ))
    }

    /// 将 overlay 背景快照写回保留主色缓冲，供随后只绘制浮层。
    fn restore_overlay_backdrop(&mut self) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support restore_overlay_backdrop",
                self.graphics_backend()
            ),
        ))
    }

    /// 释放 overlay 背景快照。
    fn release_overlay_backdrop(&mut self) {}

    /// 是否持有有效的 overlay 背景快照。
    fn has_overlay_backdrop(&self) -> bool {
        false
    }
}
