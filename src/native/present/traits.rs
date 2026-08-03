//! 图形上下文公开契约。

use super::*;

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

    /// Draw analytically antialiased circular sectors into the current RTV.
    ///
    /// Same scissor convention as [`Self::draw_solid_rects`].
    fn draw_sectors(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        _scissor: Option<(i32, i32, i32, i32)>,
        _sectors: &[GpuSector],
    ) -> Result<(), Error> {
        Err(Error::new(
            crate::core::error::Errc::NotImplemented,
            format!(
                "GraphicsBackend {} does not support draw_sectors",
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
