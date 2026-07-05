//! SoftwareEngine — CPU 软件渲染引擎。
//!
//! 内部委托 `RenderSession` + `CpuBackend`，帧逻辑由 `pipeline/frame.rs` 统一处理。

use crate::native::Error;
use crate::native::Rect;

use crate::draw::backend::{BackendKind, CpuBackend, DamageRegion};
use crate::draw::primitives::color::Color;
use crate::draw::engine::RenderOutcome;
use crate::draw::pipeline::RenderSession;
use crate::draw::traits::{Canvas2D, GraphicsEngine, UpdateStrategy};
use crate::draw::ImageHandle;

/// CPU 软件渲染引擎。
pub struct SoftwareEngine {
    session: RenderSession,
    /// 脏区域清除时使用的背景色（默认透明黑，上层可设为主题背景色）。
    pub clear_color: Color,
}

impl Default for SoftwareEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SoftwareEngine {
    /// 创建新的软件渲染引擎实例。
    pub fn new() -> Self {
        let session = match RenderSession::new(BackendKind::Cpu) {
            Ok(s) => s,
            Err(e) => {
                crate::core::log::error_fn(format!(
                    "RenderSession 创建失败: {}",
                    e.short_what()
                ));
                RenderSession::with_backend(Box::new(CpuBackend::new()))
            }
        };
        Self {
            session,
            clear_color: Color::from_rgba(0, 0, 0, 0),
        }
    }

    /// 访问内部绘图层会话（后端切换等）。
    pub fn session(&self) -> &RenderSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut RenderSession {
        &mut self.session
    }

    fn sync_clear_color(&mut self) {
        if let Some(cpu) = self.session.cpu_backend_mut() {
            cpu.set_clear_color(self.clear_color);
        }
    }

    fn cpu(&mut self) -> Option<&mut CpuBackend> {
        self.sync_clear_color();
        self.session.cpu_backend_mut()
    }
}

impl GraphicsEngine for SoftwareEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.sync_clear_color();
        self.session.initialize(width, height)
    }

    fn shutdown(&mut self) {
        self.session.shutdown();
    }

    fn resize(&mut self, width: i32, height: i32) {
        self.sync_clear_color();
        self.session.resize(width, height);
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.sync_clear_color();
        self.session.begin_frame(strategy)
    }

    fn end_frame(&mut self, _present_damage: &DamageRegion) -> RenderOutcome {
        self.session.end_frame()
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.session.canvas_2d()
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        self.session.backend_mut().create_offscreen(width, height)
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        self.session.backend_mut().destroy_offscreen(handle);
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.session.backend_mut().offscreen_canvas(handle)
    }

    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        if let Some(cpu) = self.cpu() {
            cpu.blit_offscreen(handle, dst_rect);
        }
    }

    fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        if let Some(cpu) = self.cpu() {
            cpu.blit_offscreen_src(handle, src_rect, dst_rect);
        }
    }

    fn blit_offscreen_to_canvas(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
        canvas: &mut dyn Canvas2D,
    ) {
        if let Some(cpu) = self.session.cpu_backend() {
            cpu.blit_offscreen_to_canvas(handle, src_rect, dst_rect, canvas);
        }
    }

    fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.session.cpu_backend()?.copy_offscreen_pixels(handle)
    }

    fn memory_usage(&self) -> usize {
        self.session
            .cpu_backend()
            .map(|cpu| cpu.memory_usage())
            .unwrap_or(0)
    }

    fn diagnose_memory(&self) {
        crate::core::log::info_fn(format!(
            "SoftwareEngine memory: {} bytes",
            self.memory_usage()
        ));
    }
}
