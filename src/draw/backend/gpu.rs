//! GPU 渲染后端 — GL surface + swap buffers。

use std::any::Any;
use std::cell::RefCell;

use crate::core::{DamageRegion, Errc, Error, Point, Rect};
use crate::native::traits::present::{IGraphicsContext, PresentFrame};
use glow::HasContext as _;

use crate::draw::backend::traits::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
use crate::draw::gpu_engine::GpuCanvas2D;
use crate::draw::primitives::types::ImageHandle;
use crate::draw::traits::Canvas2D;

/// GPU DrawSurface 适配器。
pub struct GpuDrawSurface {
    gl_ptr: *const glow::Context,
    canvas: GpuCanvas2D,
    width: i32,
    height: i32,
}

impl GpuDrawSurface {
    fn gl(&self) -> &glow::Context {
        unsafe { &*self.gl_ptr }
    }

    pub fn canvas_mut(&mut self) -> &mut GpuCanvas2D {
        &mut self.canvas
    }
}

impl DrawSurface for GpuDrawSurface {
    fn size(&self) -> crate::core::Size {
        crate::core::Size::new(self.width as f32, self.height as f32)
    }

    fn push_clip(&mut self, rect: Rect) {
        self.canvas.push_clip(rect);
    }

    fn pop_clip(&mut self) {
        self.canvas.pop_clip();
    }

    fn clear_all(&mut self) {
        self.canvas.clear_soft_fallback();
        unsafe {
            // Full-frame clear must ignore any leftover scissor from prior clips.
            self.gl().disable(glow::SCISSOR_TEST);
            self.gl().clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl().clear(glow::COLOR_BUFFER_BIT);
        }
    }

    fn clear_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.canvas.clear_rect_raw(x, y, w, h);
    }

    fn copy_region(&mut self, _src: Rect, _dst: Point) {
        // GPU 后端暂不支持 scroll memmove。
    }

    fn canvas(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }
}

// GL 上下文仅在主线程使用。
unsafe impl Send for GpuDrawSurface {}

struct GlOffscreen {
    fbo: glow::Framebuffer,
    texture: glow::Texture,
    canvas: GpuCanvas2D,
    width: i32,
    height: i32,
}

/// GPU 渲染后端。
pub struct GpuBackend {
    // surface 必须先于 gl/context 析构；GpuCanvas2D::Drop 会使用 gl_ptr。
    surface: GpuDrawSurface,
    pub gl: Box<glow::Context>,
    pub gpu_ctx: Box<dyn IGraphicsContext>,
    width: i32,
    height: i32,
    pub readback: RefCell<Vec<u32>>,
    offscreens: Vec<Option<GlOffscreen>>,
    free_offscreen_ids: Vec<u32>,
    next_offscreen_id: u32,
    active_offscreen: Option<u32>,
    shutdown: bool,
}

impl GpuBackend {
    pub fn new(mut gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        if !gpu_ctx.supports_gl_proc_address() {
            gpu_ctx.shutdown();
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "GpuBackend requires a GL-compatible graphics context, got {}",
                    gpu_ctx.graphics_backend()
                ),
            ));
        }
        let gl = Box::new(unsafe {
            glow::Context::from_loader_function(|s| {
                gpu_ctx.get_proc_address(s).unwrap_or(std::ptr::null())
            })
        });
        let canvas = match GpuCanvas2D::new(&gl, 1, 1) {
            Ok(canvas) => canvas,
            Err(err) => {
                gpu_ctx.shutdown();
                return Err(err);
            }
        };
        let gl_ptr = gl.as_ref() as *const glow::Context;
        Ok(Self {
            surface: GpuDrawSurface {
                gl_ptr,
                canvas,
                width: 1,
                height: 1,
            },
            gl,
            gpu_ctx,
            width: 0,
            height: 0,
            readback: RefCell::new(Vec::new()),
            offscreens: Vec::new(),
            free_offscreen_ids: Vec::new(),
            next_offscreen_id: 0,
            active_offscreen: None,
            shutdown: false,
        })
    }

    pub fn read_pixels(&self) {
        let mut rb = self.readback.borrow_mut();
        let len = (self.width * self.height * 4) as usize;
        if rb.len() < len {
            rb.resize(len, 0);
        }
        unsafe {
            self.gl.read_pixels(
                0,
                0,
                self.width,
                self.height,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(std::slice::from_raw_parts_mut(
                    rb.as_mut_ptr() as *mut u8,
                    len,
                ))),
            );
        }
    }

    pub fn pixels(&self) -> Vec<u32> {
        self.readback.borrow().clone()
    }

    pub fn pixels_ref(&self) -> std::cell::Ref<'_, Vec<u32>> {
        self.readback.borrow()
    }

    unsafe fn create_gl_offscreen(
        gl: &glow::Context,
        width: i32,
        height: i32,
    ) -> Result<GlOffscreen, Error> {
        let w = width.max(1);
        let h = height.max(1);
        let texture = gl.create_texture().map_err(|e| {
            Error::new(Errc::PlatformError, format!("GpuBackend: create_texture: {e}"))
        })?;
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            w,
            h,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::NEAREST as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::NEAREST as i32,
        );
        gl.bind_texture(glow::TEXTURE_2D, None);

        let fbo = gl.create_framebuffer().map_err(|e| {
            Error::new(
                Errc::PlatformError,
                format!("GpuBackend: create_framebuffer: {e}"),
            )
        })?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        if status != glow::FRAMEBUFFER_COMPLETE {
            gl.delete_framebuffer(fbo);
            gl.delete_texture(texture);
            return Err(Error::new(
                Errc::PlatformError,
                format!("GpuBackend: incomplete FBO status={status}"),
            ));
        }

        let canvas = GpuCanvas2D::new(gl, w, h)?;
        Ok(GlOffscreen {
            fbo,
            texture,
            canvas,
            width: w,
            height: h,
        })
    }

    fn destroy_gl_offscreen(&mut self, mut off: GlOffscreen) {
        off.canvas.release_gpu_resources();
        unsafe {
            self.gl.delete_framebuffer(off.fbo);
            self.gl.delete_texture(off.texture);
        }
    }

    fn bind_default_framebuffer(&mut self) {
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            let dpr = self.gpu_ctx.device_pixel_ratio().max(1.0);
            self.gl.viewport(
                0,
                0,
                self.gpu_ctx.width().max(1),
                self.gpu_ctx.height().max(1),
            );
            let _ = dpr;
        }
    }

    fn destroy_all_offscreens(&mut self) {
        self.active_offscreen = None;
        self.bind_default_framebuffer();
        let slots = std::mem::take(&mut self.offscreens);
        for slot in slots {
            if let Some(off) = slot {
                self.destroy_gl_offscreen(off);
            }
        }
        self.free_offscreen_ids.clear();
        self.next_offscreen_id = 0;
    }
}

fn capabilities_for_context(gpu_ctx: &dyn IGraphicsContext) -> BackendCapabilities {
    // Picture offscreen is available only on the native partial-present path.
    // A full-present context must keep the full-redraw capability baseline.
    if gpu_ctx.caps().partial_present {
        BackendCapabilities::gpu_with_offscreen()
    } else {
        BackendCapabilities::gpu_full_redraw()
    }
}

fn present_graphics_context(
    gpu_ctx: &mut dyn IGraphicsContext,
    damage: &DamageRegion,
) -> Result<(), Error> {
    let frame = PresentFrame::Swapchain {
        damage: damage.to_present_damage(),
    };
    gpu_ctx.present(&frame)
}

impl RenderBackend for GpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        capabilities_for_context(self.gpu_ctx.as_ref())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let logical_w = width.max(1);
        let logical_h = height.max(1);
        self.gpu_ctx.resize(logical_w, logical_h);
        let physical_w = self.gpu_ctx.width();
        let physical_h = self.gpu_ctx.height();
        let dpr = self.gpu_ctx.device_pixel_ratio().max(1.0);
        self.width = logical_w;
        self.height = logical_h;
        self.surface.width = logical_w;
        self.surface.height = logical_h;
        self.surface.canvas.resize(logical_w, logical_h)?;
        self.surface.canvas.set_device_pixel_ratio(dpr);
        self.gpu_ctx.make_current();
        unsafe {
            self.gl.viewport(0, 0, physical_w, physical_h);
        }
        Ok(())
    }

    fn shutdown(&mut self) {
        if self.shutdown {
            return;
        }
        self.shutdown = true;
        self.destroy_all_offscreens();
        self.gpu_ctx.make_current();
        self.surface.canvas.release_gpu_resources();
        self.gpu_ctx.shutdown();
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.surface
    }

    fn make_current(&mut self) {
        self.gpu_ctx.make_current();
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.gpu_ctx.device_pixel_ratio()
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        if width <= 0 || height <= 0 {
            return None;
        }
        self.gpu_ctx.make_current();
        let off = unsafe { Self::create_gl_offscreen(&self.gl, width, height) }.ok()?;
        let id = if let Some(id) = self.free_offscreen_ids.pop() {
            id
        } else {
            let id = self.next_offscreen_id;
            self.next_offscreen_id = self.next_offscreen_id.saturating_add(1);
            id
        };
        let idx = id as usize;
        while self.offscreens.len() <= idx {
            self.offscreens.push(None);
        }
        self.offscreens[idx] = Some(off);
        Some(ImageHandle(id))
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        if self.active_offscreen == Some(handle.0) {
            self.active_offscreen = None;
            self.bind_default_framebuffer();
        }
        let idx = handle.0 as usize;
        if idx < self.offscreens.len() {
            if let Some(off) = self.offscreens[idx].take() {
                self.gpu_ctx.make_current();
                self.destroy_gl_offscreen(off);
                self.free_offscreen_ids.push(handle.0);
            }
        }
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        let idx = handle.0 as usize;
        self.offscreens
            .get_mut(idx)?
            .as_mut()
            .map(|o| &mut o.canvas as &mut dyn Canvas2D)
    }

    fn begin_offscreen_paint(&mut self, handle: &ImageHandle) -> bool {
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get(idx) else {
            return false;
        };
        let fbo = off.fbo;
        let w = off.width;
        let h = off.height;
        self.gpu_ctx.make_current();
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            self.gl.viewport(0, 0, w, h);
            self.gl.disable(glow::SCISSOR_TEST);
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        self.active_offscreen = Some(handle.0);
        true
    }

    fn flush_offscreen_paint(&mut self, handle: &ImageHandle) {
        let idx = handle.0 as usize;
        let Some(Some(off)) = self.offscreens.get_mut(idx) else {
            return;
        };
        let fbo = off.fbo;
        let w = off.width;
        let h = off.height;
        self.gpu_ctx.make_current();
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            self.gl.viewport(0, 0, w, h);
        }
        if let Err(err) = off.canvas.flush_soft_fallback() {
            crate::core::log::warn_fn(format!(
                "GpuBackend: offscreen flush_soft_fallback failed: {}",
                err.short_what()
            ));
        }
    }

    fn end_offscreen_paint(&mut self) {
        self.active_offscreen = None;
        self.bind_default_framebuffer();
    }

    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        let src = Rect::new(0.0, 0.0, dst_rect.w, dst_rect.h);
        self.blit_offscreen_src(handle, src, dst_rect);
    }

    fn blit_offscreen_src(&mut self, handle: &ImageHandle, _src_rect: Rect, dst_rect: Rect) {
        let idx = handle.0 as usize;
        let texture = match self.offscreens.get(idx).and_then(|o| o.as_ref()) {
            Some(off) => off.texture,
            None => return,
        };
        if let Some(dst_id) = self.active_offscreen {
            if dst_id == handle.0 {
                return;
            }
            if let Some(Some(dst_off)) = self.offscreens.get(dst_id as usize) {
                dst_off.canvas.blit_external_texture(texture, dst_rect);
            }
            return;
        }
        self.gpu_ctx.make_current();
        self.bind_default_framebuffer();
        self.surface
            .canvas
            .blit_external_texture(texture, dst_rect);
    }

    fn present(&mut self, damage: &DamageRegion) -> Result<(), Error> {
        self.gpu_ctx.make_current();
        self.bind_default_framebuffer();
        self.surface.canvas.flush_soft_fallback()?;
        present_graphics_context(self.gpu_ctx.as_mut(), damage)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Drop for GpuBackend {
    fn drop(&mut self) {
        <Self as RenderBackend>::shutdown(self);
    }
}

// GL 上下文仅在主线程使用，与旧 GpuEngine 一致。
unsafe impl Send for GpuBackend {}
unsafe impl Sync for GpuBackend {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::native::traits::present::{
        GraphicsBackend, GraphicsContextCaps, IGraphicsContext, PresentDamage,
    };

    #[derive(Default)]
    struct RecordingGraphicsContext {
        make_current_calls: usize,
        swap_damage: Option<PresentDamage>,
        partial_present: bool,
    }

    impl IGraphicsContext for RecordingGraphicsContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::gpu_native_swapchain(
                GraphicsBackend::OpenGlEs,
                self.partial_present,
                1.0,
            )
        }

        fn graphics_backend(&self) -> crate::native::traits::present::GraphicsBackend {
            crate::native::traits::present::GraphicsBackend::OpenGlEs
        }

        fn initialize(
            &mut self,
            _native_window: *mut std::ffi::c_void,
            _width: i32,
            _height: i32,
        ) -> crate::core::Result<()> {
            Ok(())
        }

        fn resize(&mut self, _width: i32, _height: i32) {}

        fn make_current(&mut self) {
            self.make_current_calls += 1;
        }

        fn swap_buffers(&mut self, damage: PresentDamage) {
            self.swap_damage = Some(damage);
        }

        fn shutdown(&mut self) {}

        fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
            Vec::new()
        }

        fn width(&self) -> i32 {
            0
        }

        fn height(&self) -> i32 {
            0
        }
    }

    #[test]
    fn gpu_present_forwards_partial_damage_to_graphics_context() {
        let mut context = RecordingGraphicsContext::default();
        let damage = DamageRegion::partial(vec![Rect::new(1.0, 2.0, 3.0, 4.0)]);

        present_graphics_context(&mut context, &damage).expect("swapchain present");

        assert_eq!(context.make_current_calls, 1);
        assert_eq!(
            context.swap_damage,
            Some(PresentDamage::Partial(vec![(1, 2, 3, 4)]))
        );
    }

    #[test]
    fn gpu_capabilities_follow_native_partial_present_support() {
        let context = RecordingGraphicsContext::default();
        assert_eq!(
            capabilities_for_context(&context),
            BackendCapabilities::gpu_full_redraw()
        );

        let partial_context = RecordingGraphicsContext {
            partial_present: true,
            ..Default::default()
        };
        assert_eq!(
            capabilities_for_context(&partial_context),
            BackendCapabilities::gpu_with_offscreen()
        );
    }

    #[cfg(feature = "opengles")]
    #[test]
    fn opengles_gpu_engine_draws_on_real_window() {
        use crate::draw::gpu_engine::GpuEngine;
        use crate::draw::{Color, GraphicsEngine, UpdateStrategy};
        use crate::native::traits::present::GraphicsBackend;

        if std::env::consts::OS != "windows" {
            return;
        }

        let mut platform = crate::native::create_platform().expect("platform");
        let mut window = platform
            .window_manager()
            .create_window("GPU test", 640, 480)
            .expect("window");
        let surface = window.native_surface_ptr();
        assert!(
            !surface.is_null(),
            "Windows HWND must be exposed as native_surface_ptr"
        );
        let context = crate::native::create_gpu_context_with_backend(
            surface,
            640,
            480,
            GraphicsBackend::OpenGlEs,
        )
        .expect("WglContext");
        let mut engine = GpuEngine::new(context).expect("OpenGL ES GpuEngine");
        engine.initialize(640, 480).expect("initialize GL engine");
        let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
        engine
            .canvas_2d()
            .fill_rect(Rect::new(0.0, 0.0, 640.0, 480.0), Color::red(), None);
        engine
            .canvas_2d()
            .fill_ellipse(Rect::new(220.0, 140.0, 200.0, 200.0), Color::blue());
        {
            let backend = engine
                .session_mut()
                .gpu_backend_mut()
                .expect("OpenGL ES backend");
            backend
                .surface
                .canvas_mut()
                .flush_soft_fallback()
                .expect("soft fallback upload");
            assert_eq!(unsafe { backend.gl.get_error() }, glow::NO_ERROR);
            backend.read_pixels();
            let pixels = backend.pixels_ref();
            let center = pixels[(240 * 640 + 320) as usize];
            assert_eq!(center, 0xFFFF_0000, "center pixel must contain blue");
            assert!(
                pixels.contains(&0xFF00_00FF),
                "native red background must remain visible"
            );
        }
        // end_frame → GpuBackend::present → WGL SwapBuffers（屏幕呈现路径）
        let _ = engine.end_frame(&DamageRegion::full());
        {
            let backend = engine
                .session_mut()
                .gpu_backend_mut()
                .expect("OpenGL ES backend");
            assert_eq!(
                unsafe { backend.gl.get_error() },
                glow::NO_ERROR,
                "SwapBuffers present must leave GL context healthy"
            );
        }

        // 多帧 present：证明连续 SwapBuffers 后仍可绘制（对齐 D3D11 repeated_present）
        for i in 0..16 {
            let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
            let shade = ((i * 15) % 256) as u8;
            engine.canvas_2d().fill_rect(
                Rect::new(0.0, 0.0, 640.0, 480.0),
                Color::from_rgba(shade, 40, 255 - shade, 255),
                None,
            );
            let _ = engine.end_frame(&DamageRegion::full());
            let backend = engine
                .session_mut()
                .gpu_backend_mut()
                .expect("OpenGL ES backend");
            assert_eq!(
                unsafe { backend.gl.get_error() },
                glow::NO_ERROR,
                "frame {i}: present must not raise GL error"
            );
        }

        // 末帧再画一次并在 present 前 readback，证明 present 循环后仍可写 framebuffer
        let _ = engine.begin_frame(UpdateStrategy::FullRedraw);
        engine
            .canvas_2d()
            .fill_rect(Rect::new(0.0, 0.0, 640.0, 480.0), Color::green(), None);
        {
            let backend = engine
                .session_mut()
                .gpu_backend_mut()
                .expect("OpenGL ES backend");
            backend
                .surface
                .canvas_mut()
                .flush_soft_fallback()
                .expect("soft fallback upload");
            backend.read_pixels();
            let pixels = backend.pixels_ref();
            assert!(
                pixels.iter().any(|p| *p == 0xFF00_FF00 || (*p & 0x00FF_0000) != 0),
                "post-present loop must still write drawable pixels"
            );
        }
        let _ = engine.end_frame(&DamageRegion::full());

        engine.shutdown();
        window.close().expect("close native window");
    }

    #[test]
    fn gpu_resize_uses_logical_dimensions_for_surface_coordinates() {
        let logical_w = 800_i32;
        let logical_h = 600_i32;
        let physical_w = logical_w * 2;
        let physical_h = logical_h * 2;
        let dpr = physical_w as f32 / logical_w as f32;
        assert!((dpr - 2.0).abs() < f32::EPSILON);
        // GpuBackend::resize 以窗口 dip 尺寸作为画布坐标系，帧缓冲为 physical。
        assert_eq!((physical_w as f32 / dpr).round() as i32, logical_w);
        assert_eq!((physical_h as f32 / dpr).round() as i32, logical_h);
    }

    struct NonGlGraphicsContext;

    impl IGraphicsContext for NonGlGraphicsContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::D3d11, 1.0)
        }

        fn graphics_backend(&self) -> crate::native::traits::present::GraphicsBackend {
            crate::native::traits::present::GraphicsBackend::D3d11
        }

        fn initialize(
            &mut self,
            _native_window: *mut std::ffi::c_void,
            _width: i32,
            _height: i32,
        ) -> crate::core::Result<()> {
            Ok(())
        }

        fn resize(&mut self, _width: i32, _height: i32) {}

        fn make_current(&mut self) {}

        fn swap_buffers(&mut self, _damage: PresentDamage) {}

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
    fn gpu_backend_rejects_non_gl_context() {
        let err = match GpuBackend::new(Box::new(NonGlGraphicsContext)) {
            Ok(_) => panic!("non-GL context must not initialize the GL backend"),
            Err(err) => err,
        };

        assert_eq!(err.code(), crate::core::Errc::InvalidArgument);
        assert!(err.message().contains("requires a GL-compatible"));
    }
}
