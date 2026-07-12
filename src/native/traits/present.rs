//! Presentation contracts for CPU presenters and GPU graphics contexts.

use crate::core::error::{Error, Result};
pub use crate::core::PresentDamage;
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

/// CPU-rasterized glyph coverage blit for GPU-native text (#169).
///
/// `coverage` is a row-major `cov_w * cov_h` alpha mask (0..255), matching
/// soft `blit_glyph`. Dest `(x,y,w,h)` is logical top-left; typically
/// `w == cov_w as f32` and `h == cov_h as f32`. Context packs into a glyph
/// atlas and draws textured quads — not full GPU shaping.
#[derive(Debug, Clone)]
pub struct GpuGlyphBlit {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub rgba: [f32; 4],
    pub coverage: std::sync::Arc<[u8]>,
    pub cov_w: u32,
    pub cov_h: u32,
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

/// Axis-aligned box / drop shadow for GPU-native Canvas2D (#169).
///
/// Matches CPU `draw_box_shadow` / `draw_box_shadow_ambient`: shadow is drawn
/// at `(x+offset_x, y+offset_y)` with the same size / corner radii, soft edge
/// via SDF coverage (`blur`). `rgba` is straight (non-premultiplied) 0..1.
/// Non-identity transforms and exotic blends soft-fallback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuBoxShadow {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub rgba: [f32; 4],
    pub radius: [f32; 4],
    /// `true` → ambient (softer) coverage curve.
    pub ambient: bool,
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
}

impl NativeRasterCaps {
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
        }
    }

    pub const fn has_hybrid_baseline(self) -> bool {
        self.clear_target && self.soft_blit
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

/// Bounded visible portion of a CPU soft-raster segment.
///
/// Coordinates are in the full logical CPU surface. The source buffer passed
/// to [`IGraphicsContext::blit_soft_fallback_tile`] remains full-surface so a
/// native implementation can preserve its source pitch without making a
/// per-frame copy. Native backends must upload and sample only this rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftFallbackTile {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl SoftFallbackTile {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// CPU pixel presenter.
pub trait IPresenter {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<(), Error>;

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
}

/// Raster axis — how Canvas2D content is produced ([#169](docs/决策.md#d169)).
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

/// Present axis — how pixels reach the screen ([#169](docs/决策.md#d169)).
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
    /// Whether this context can present partial damage regions natively.
    pub partial_present: bool,
    pub device_pixel_ratio: f32,
}

impl GraphicsContextCaps {
    /// Legal combo: [`RasterMode::GpuNative`] × [`PresentMode::Swapchain`].
    pub fn gpu_native_swapchain(
        backend: GraphicsBackend,
        partial_present: bool,
        device_pixel_ratio: f32,
    ) -> Self {
        Self {
            backend,
            raster: RasterMode::GpuNative,
            present: PresentMode::Swapchain,
            partial_present,
            device_pixel_ratio,
        }
    }

    /// Legal combo: [`RasterMode::Cpu`] × [`PresentMode::PixelUpload`].
    pub fn cpu_pixel_upload(backend: GraphicsBackend, device_pixel_ratio: f32) -> Self {
        Self {
            backend,
            raster: RasterMode::Cpu,
            present: PresentMode::PixelUpload,
            partial_present: false,
            device_pixel_ratio,
        }
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

/// Opaque native runtime lease acquired from a graphics context.
///
/// Its public surface exposes only API-neutral identity. API objects and
/// symbol loading remain private to `native/graphics/<api>`; crate-local draw
/// adapters can consume the lease only through the matching native module.
pub struct NativeGraphicsRuntime {
    backend: GraphicsBackend,
    #[cfg(feature = "opengles")]
    opengles: Option<crate::native::graphics::opengl::NativeOpenGlRuntime>,
}

impl std::fmt::Debug for NativeGraphicsRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeGraphicsRuntime")
            .field("backend", &self.backend)
            .finish_non_exhaustive()
    }
}

impl NativeGraphicsRuntime {
    pub fn backend(&self) -> GraphicsBackend {
        self.backend
    }

    #[cfg(feature = "opengles")]
    pub(crate) fn opengles(runtime: crate::native::graphics::opengl::NativeOpenGlRuntime) -> Self {
        Self {
            backend: GraphicsBackend::OpenGlEs,
            opengles: Some(runtime),
        }
    }

    #[cfg(feature = "opengles")]
    pub(crate) fn into_opengles(
        self,
    ) -> Result<crate::native::graphics::opengl::NativeOpenGlRuntime, Error> {
        if self.backend != GraphicsBackend::OpenGlEs {
            return Err(Error::new(
                crate::core::error::Errc::InvalidArgument,
                format!("native graphics runtime is {}, not opengles", self.backend),
            ));
        }
        self.opengles.ok_or_else(|| {
            Error::new(
                crate::core::error::Errc::InvalidState,
                "opengles runtime lease did not contain a native runtime",
            )
        })
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
    /// New lifecycle code must use this method.  The legacy [`Self::shutdown`]
    /// hook remains temporarily so platform implementations can migrate
    /// independently; its default preserves the old behavior but cannot
    /// surface a native teardown error.
    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.shutdown();
        Ok(())
    }

    fn shutdown(&mut self);

    /// Checked readback boundary.  This lets a thread-affine wrapper reject a
    /// foreign caller before it reaches the native context.
    fn try_read_pixels(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Vec<u32>, Error> {
        Ok(self.read_pixels(x, y, width, height))
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Vec<u32>;
    fn width(&self) -> i32;
    fn height(&self) -> i32;

    /// Acquire an opaque native runtime for the context's selected API.
    ///
    /// Draw code must not receive raw API symbol loaders through this trait.
    /// Context implementations that do not expose a runtime keep the default
    /// typed rejection; API-specific implementations create the runtime under
    /// `native/graphics/<api>/` after making the context current.
    fn acquire_native_runtime(&mut self) -> Result<NativeGraphicsRuntime, Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not expose a native runtime",
                self.graphics_backend()
            ),
        ))
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
    /// Same role as GL `GpuCanvas2D::flush_soft_fallback`: unsupported Canvas2D
    /// ops stay on CPU and composite on top of native geometry.
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
    /// The default rejects a partial tile instead of silently expanding it to
    /// a full texture transfer. That makes an unimplemented damage path a
    /// typed failure, not a false performance capability. Full-surface tiles
    /// retain compatibility with contexts that only implement the legacy API.
    fn blit_soft_fallback_tile(
        &mut self,
        pixels: &[u32],
        surface_width: i32,
        surface_height: i32,
        tile: SoftFallbackTile,
    ) -> Result<(), Error> {
        if tile.x == 0
            && tile.y == 0
            && tile.width == surface_width
            && tile.height == surface_height
        {
            return self.blit_soft_fallback(pixels, surface_width, surface_height);
        }
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support bounded CPU soft fallback uploads",
                self.graphics_backend()
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

    /// Sample offscreen SRV into the **current** RT as an alpha-blended textured quad.
    ///
    /// `src` / `dst` are in logical pixels (top-left origin), relative to the
    /// offscreen and current target respectively.
    fn blit_offscreen_target(
        &mut self,
        _id: OffscreenTargetId,
        _src: crate::core::Rect,
        _dst: crate::core::Rect,
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support blit_offscreen_target",
                self.graphics_backend()
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        validate_pixel_buffer, GraphicsBackend, GraphicsContextCaps, IGraphicsContext,
        NativeRasterCaps, PresentDamage, PresentFrame,
    };
    use crate::core::{Errc, Error, Result};
    use std::str::FromStr;

    #[test]
    fn graphics_backend_parses_common_aliases() {
        assert_eq!(
            GraphicsBackend::from_str("direct3d-12").unwrap(),
            GraphicsBackend::D3d12
        );
        assert_eq!(
            GraphicsBackend::from_str("directx_11").unwrap(),
            GraphicsBackend::D3d11
        );
        assert_eq!(
            GraphicsBackend::from_str("OpenGL ES").unwrap(),
            GraphicsBackend::OpenGlEs
        );
        assert_eq!(
            GraphicsBackend::from_str("vk").unwrap(),
            GraphicsBackend::Vulkan
        );
    }

    #[test]
    fn graphics_backend_display_uses_config_tokens() {
        assert_eq!(GraphicsBackend::Auto.to_string(), "auto");
        assert_eq!(GraphicsBackend::OpenGlEs.to_string(), "opengles");
        assert_eq!(GraphicsBackend::Metal.as_str(), "metal");
    }

    #[test]
    fn d3d11_caps_advertise_offscreen_after_crop_and_scissor_are_correct() {
        assert!(
            NativeRasterCaps::d3d11_full().offscreen_targets,
            "D3D11 Picture offscreen support requires source crop and scissor restoration"
        );
    }

    #[test]
    fn pixel_buffer_validation_rejects_invalid_extent_and_short_payload() {
        assert!(validate_pixel_buffer(&[0; 4], 2, 2).is_ok());
        assert_eq!(
            validate_pixel_buffer(&[0; 3], 2, 2)
                .expect_err("short payload")
                .code(),
            Errc::InvalidArgument
        );
        assert_eq!(
            validate_pixel_buffer(&[], 0, 1)
                .expect_err("empty width")
                .code(),
            Errc::InvalidArgument
        );
    }

    struct DefaultPresentFailure {
        fail_make_current: bool,
        swap_calls: usize,
    }

    impl IGraphicsContext for DefaultPresentFailure {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, false, 1.0)
        }

        fn initialize(
            &mut self,
            _native_window: *mut std::ffi::c_void,
            _width: i32,
            _height: i32,
        ) -> Result<()> {
            Ok(())
        }

        fn resize(&mut self, _width: i32, _height: i32) -> Result<()> {
            Ok(())
        }

        fn make_current(&mut self) -> Result<()> {
            if self.fail_make_current {
                Err(Error::new(Errc::PlatformError, "make current failed"))
            } else {
                Ok(())
            }
        }

        fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
            self.swap_calls += 1;
            Err(Error::new(Errc::PlatformError, "swap failed"))
        }

        fn shutdown(&mut self) {}

        fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
            Vec::new()
        }

        fn width(&self) -> i32 {
            1
        }

        fn height(&self) -> i32 {
            1
        }
    }

    #[test]
    fn default_present_propagates_make_current_failure_without_swapping() {
        let mut context = DefaultPresentFailure {
            fail_make_current: true,
            swap_calls: 0,
        };

        let error = context
            .present(&PresentFrame::Swapchain {
                damage: PresentDamage::Full,
            })
            .expect_err("make_current failure must escape default present");

        assert!(error.message().contains("make current failed"));
        assert_eq!(context.swap_calls, 0);
    }

    #[test]
    fn default_present_propagates_swap_failure() {
        let mut context = DefaultPresentFailure {
            fail_make_current: false,
            swap_calls: 0,
        };

        let error = context
            .present(&PresentFrame::Swapchain {
                damage: PresentDamage::Full,
            })
            .expect_err("swap failure must escape default present");

        assert!(error.message().contains("swap failed"));
        assert_eq!(context.swap_calls, 1);
    }
}
