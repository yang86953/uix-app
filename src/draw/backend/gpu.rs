//! GPU 渲染后端 — GL surface + swap buffers。

use std::any::Any;
use std::cell::RefCell;

use glow::HasContext as _;
use crate::native::{Error, IGraphicsContext, Point, Rect};

use crate::draw::backend::traits::{
    BackendCapabilities, BackendKind, DamageRegion, DrawSurface, RenderBackend,
};
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
    fn size(&self) -> crate::native::Size {
        crate::native::Size::new(self.width as f32, self.height as f32)
    }

    fn push_clip(&mut self, rect: Rect) {
        self.canvas.push_clip(rect);
    }

    fn pop_clip(&mut self) {
        self.canvas.pop_clip();
    }

    fn clear_all(&mut self) {
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
    pub gl: Box<glow::Context>,
    pub gpu_ctx: Box<dyn IGraphicsContext>,
    width: i32,
    height: i32,
    pub readback: RefCell<Vec<u32>>,
    surface: GpuDrawSurface,
}

impl GpuBackend {
    pub fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let gl = Box::new(unsafe {
            glow::Context::from_loader_function(|s| {
                gpu_ctx.get_proc_address(s).unwrap_or(std::ptr::null())
            })
        });
        let canvas = GpuCanvas2D::new(&gl, 1, 1)?;
        let gl_ptr = gl.as_ref() as *const glow::Context;
        Ok(Self {
            gl,
            gpu_ctx,
            width: 0,
            height: 0,
            readback: RefCell::new(Vec::new()),
            surface: GpuDrawSurface {
                gl_ptr,
                canvas,
                width: 1,
                height: 1,
            },
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

impl RenderBackend for GpuBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gpu
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::gpu()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.width = width;
        self.height = height;
        self.surface.width = width;
        self.surface.height = height;
        self.surface.canvas = GpuCanvas2D::new(&self.gl, width, height)?;
        self.gpu_ctx.resize(width, height);
        unsafe {
            self.gl.viewport(0, 0, width, height);
        }
        Ok(())
    }

    fn shutdown(&mut self) {
        self.gpu_ctx.shutdown();
    }

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.surface
    }

    fn present(&mut self, _damage: &DamageRegion) -> Result<(), Error> {
        self.gpu_ctx.make_current();
        self.gpu_ctx.swap_buffers();
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// GL 上下文仅在主线程使用，与旧 GpuEngine 一致。
unsafe impl Send for GpuBackend {}
unsafe impl Sync for GpuBackend {}
