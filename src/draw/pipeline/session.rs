//! 绘图层会话 — Pipeline + Backend 组合入口（Phase 1 骨架）。

use crate::core::Error;
use crate::native::traits::present::IGraphicsContext;

use crate::draw::backend::{
    create_backend, BackendCapabilities, BackendKind, CpuBackend, GpuBackend, NullBackend,
    RenderBackend,
};
use crate::draw::engine::RenderOutcome;
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, UpdateStrategy};

/// 绘图层会话：持有可切换后端与共享帧逻辑。
pub struct RenderSession {
    backend: Box<dyn RenderBackend>,
    width: i32,
    height: i32,
    /// 后端切换后下一帧强制全帧重绘。
    force_full_frame: bool,
    /// 保留 GPU 上下文以便 Cpu ↔ Gpu 切换。
    gpu_ctx: Option<Box<dyn IGraphicsContext>>,
}

impl RenderSession {
    /// 以指定后端种类创建会话。
    pub fn new(kind: BackendKind) -> Result<Self, Error> {
        let resolved = resolve_kind(kind);
        let backend = create_backend(resolved, None)?;
        Ok(Self {
            backend,
            width: 0,
            height: 0,
            force_full_frame: false,
            gpu_ctx: None,
        })
    }

    /// 注入已有后端实例。
    pub fn with_backend(backend: Box<dyn RenderBackend>) -> Self {
        Self {
            backend,
            width: 0,
            height: 0,
            force_full_frame: false,
            gpu_ctx: None,
        }
    }

    /// 绑定 GPU 上下文，供后续 `set_backend(Gpu)` 使用。
    pub fn set_gpu_context(&mut self, ctx: Box<dyn IGraphicsContext>) {
        self.gpu_ctx = Some(ctx);
    }

    pub fn backend_kind(&self) -> BackendKind {
        self.backend.kind()
    }

    pub fn capabilities(&self) -> BackendCapabilities {
        self.backend.capabilities()
    }

    pub fn graphics_capabilities(&self) -> GraphicsCapabilities {
        self.capabilities().into()
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.width = width;
        self.height = height;
        self.backend.resize(width, height)
    }

    pub fn shutdown(&mut self) {
        self.backend.shutdown();
        self.width = 0;
        self.height = 0;
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        if self.backend.resize(width, height).is_err() {
            crate::core::log::error_fn("RenderSession::resize 失败");
        }
    }

    /// 运行时切换后端；下一帧将强制 FullRedraw。
    pub fn set_backend(&mut self, kind: BackendKind) -> Result<(), Error> {
        let resolved = resolve_kind(kind);
        let gpu_ctx = self.gpu_ctx.take();
        self.backend.shutdown();
        self.backend = create_backend(resolved, gpu_ctx)?;
        if self.width > 0 && self.height > 0 {
            self.backend.resize(self.width, self.height)?;
        }
        self.force_full_frame = true;
        Ok(())
    }

    pub fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        if self.force_full_frame {
            self.force_full_frame = false;
            let caps = self.backend.capabilities();
            return crate::draw::pipeline::frame::begin_frame(
                UpdateStrategy::FullRedraw,
                self.backend.surface(),
                self.width,
                self.height,
                caps,
            );
        }
        let caps = self.backend.capabilities();
        crate::draw::pipeline::frame::begin_frame(
            strategy,
            self.backend.surface(),
            self.width,
            self.height,
            caps,
        )
    }

    pub fn end_frame(&mut self) -> RenderOutcome {
        crate::draw::pipeline::frame::end_frame(self.backend.surface())
    }

    pub fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.backend.surface().canvas()
    }

    pub fn cpu_backend(&self) -> Option<&CpuBackend> {
        self.backend.as_any().downcast_ref()
    }

    pub fn cpu_backend_mut(&mut self) -> Option<&mut CpuBackend> {
        self.backend.as_any_mut().downcast_mut()
    }

    pub fn gpu_backend(&self) -> Option<&GpuBackend> {
        self.backend.as_any().downcast_ref()
    }

    pub fn gpu_backend_mut(&mut self) -> Option<&mut GpuBackend> {
        self.backend.as_any_mut().downcast_mut()
    }

    pub fn null_backend_mut(&mut self) -> Option<&mut NullBackend> {
        self.backend.as_any_mut().downcast_mut()
    }

    pub fn backend_mut(&mut self) -> &mut dyn RenderBackend {
        &mut *self.backend
    }
}

fn resolve_kind(kind: BackendKind) -> BackendKind {
    match kind {
        BackendKind::Auto => BackendKind::Cpu,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::backend::DamageRegion;

    #[test]
    fn set_backend_to_null_forces_full_frame_once() {
        let mut session = RenderSession::new(BackendKind::Cpu).expect("Cpu 会话");
        session.initialize(10, 10).expect("init");
        session.set_backend(BackendKind::Null).expect("switch");
        assert_eq!(session.backend_kind(), BackendKind::Null);
        let outcome = session.begin_frame(UpdateStrategy::DirtyRects(vec![]));
        assert_eq!(outcome, RenderOutcome::Present(DamageRegion::full()));
        session.end_frame();
        let outcome = session.begin_frame(UpdateStrategy::DirtyRects(vec![]));
        assert_eq!(outcome, RenderOutcome::Idle);
    }

    #[test]
    fn auto_resolves_to_cpu() {
        let session = RenderSession::new(BackendKind::Auto).expect("Auto 会话");
        assert_eq!(session.backend_kind(), BackendKind::Cpu);
    }
}
