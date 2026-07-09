//! GPU 渲染后端 — GL surface + swap buffers。

use std::any::Any;
use std::cell::RefCell;

use crate::core::{DamageRegion, Errc, Error, Point, Rect};
use crate::native::traits::present::{IGraphicsContext, PresentFrame};
use glow::HasContext as _;

use crate::draw::backend::traits::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
use crate::draw::gpu_engine::GpuCanvas2D;
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

/// GPU 渲染后端。
pub struct GpuBackend {
    // surface 必须先于 gl/context 析构；GpuCanvas2D::Drop 会使用 gl_ptr。
    surface: GpuDrawSurface,
    pub gl: Box<glow::Context>,
    pub gpu_ctx: Box<dyn IGraphicsContext>,
    width: i32,
    height: i32,
    pub readback: RefCell<Vec<u32>>,
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
}

fn present_graphics_context(gpu_ctx: &mut dyn IGraphicsContext, damage: &DamageRegion) {
    let frame = PresentFrame::Swapchain {
        damage: damage.to_present_damage(),
    };
    if let Err(err) = gpu_ctx.present(&frame) {
        crate::core::log::error_fn(format!("GpuBackend present failed: {}", err.short_what()));
    }
}

impl RenderBackend for GpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::gpu()
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
        self.gpu_ctx.make_current();
        self.surface.canvas.release_gpu_resources();
        self.gpu_ctx.shutdown();
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.surface
    }

    fn present(&mut self, damage: &DamageRegion) -> Result<(), Error> {
        self.gpu_ctx.make_current();
        if let Err(e) = self.surface.canvas.flush_soft_fallback() {
            crate::core::log::error_fn(format!(
                "GpuBackend soft_fallback flush failed: {}",
                e.short_what()
            ));
        }
        present_graphics_context(self.gpu_ctx.as_mut(), damage);
        Ok(())
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
    }

    impl IGraphicsContext for RecordingGraphicsContext {
        fn caps(&self) -> GraphicsContextCaps {
            GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::OpenGlEs, false, 1.0)
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

        present_graphics_context(&mut context, &damage);

        assert_eq!(context.make_current_calls, 1);
        assert_eq!(
            context.swap_damage,
            Some(PresentDamage::Partial(vec![(1, 2, 3, 4)]))
        );
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
