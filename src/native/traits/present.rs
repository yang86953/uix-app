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

/// Unified present payload for [`IGraphicsContext::present`] (M7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentFrame<'a> {
    /// GPU swapchain / equivalent (native raster path).
    Swapchain {
        damage: PresentDamage,
    },
    /// CPU raster upload (upload-present path).
    PixelBuffer {
        pixels: &'a [u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    },
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

/// Raster axis — how Canvas2D content is produced ([#169](docs/decisions.md#d169)).
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

/// Present axis — how pixels reach the screen ([#169](docs/decisions.md#d169)).
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

/// GPU graphics context lifecycle and presentation contract.
pub trait IGraphicsContext {
    fn caps(&self) -> GraphicsContextCaps;

    fn graphics_backend(&self) -> GraphicsBackend {
        self.caps().backend
    }

    fn initialize(
        &mut self,
        native_window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> Result<(), Error>;

    fn resize(&mut self, width: i32, height: i32);
    fn make_current(&mut self);
    fn swap_buffers(&mut self, damage: PresentDamage);
    fn shutdown(&mut self);
    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Vec<u32>;
    fn width(&self) -> i32;
    fn height(&self) -> i32;

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
                self.make_current();
                self.swap_buffers(damage.clone());
                Ok(())
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

    fn get_proc_address(&self, name: &str) -> Option<*const std::ffi::c_void> {
        let _ = name;
        None
    }

    /// Clear the current GPU render target (GpuNative × Swapchain).
    ///
    /// Default: not implemented. OpenGL ES clears via the draw GL backend;
    /// D3D11 implements this on the swapchain RTV.
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

    /// Whether this context can draw solid/rounded rects on the GPU.
    fn supports_native_geometry(&self) -> bool {
        false
    }

    /// Draw solid-color (optionally rounded) quads into the current RTV.
    ///
    /// `scissor` is optional logical-pixel AABB `(x, y, w, h)` top-left origin.
    /// Used by D3D11 `GpuNative` for hot Canvas2D `fill_rect` / `fill_circle`.
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
    /// Same scissor convention as [`Self::draw_solid_rects`]. Used by D3D11
    /// `GpuNative` for hot Canvas2D `stroke_rect` / `stroke_circle`.
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

    /// Whether this context can draw glyph coverage via an atlas.
    fn supports_native_glyphs(&self) -> bool {
        false
    }

    /// Pack CPU glyph coverage into a GPU atlas and draw textured quads.
    ///
    /// Same scissor convention as [`Self::draw_solid_rects`]. Used by D3D11
    /// `GpuNative` for identity-transform solid `blit_glyph` / text.
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
    /// Same scissor convention as [`Self::draw_solid_rects`]. Used by D3D11
    /// `GpuNative` for identity-transform `fill_linear_gradient`.
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
    /// Same scissor convention as [`Self::draw_solid_rects`]. Used by D3D11
    /// `GpuNative` for identity-transform `fill_radial_gradient`.
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
    /// Same scissor convention as [`Self::draw_solid_rects`]. Used by D3D11
    /// `GpuNative` for identity-transform simple `fill_path` / `stroke_path`
    /// (CPU tessellate → GPU triangles).
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
    /// Same scissor convention as [`Self::draw_solid_rects`]. Used by D3D11
    /// `GpuNative` for identity-transform `draw_box_shadow` /
    /// `draw_box_shadow_ambient` (SDF outer glow; matches CPU coverage).
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
}

#[cfg(test)]
mod tests {
    use super::GraphicsBackend;
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
}
