//! 绘图层会话 — Pipeline + Backend 组合入口（Phase 1 骨架）。

use crate::core::Error;
use crate::native::traits::present::IGraphicsContext;
use std::thread::ThreadId;

#[cfg(all(test, feature = "opengles"))]
use crate::draw::backend::NativeGpuBackend;
use crate::draw::backend::{
    create_backend, BackendCapabilities, BackendKind, CpuBackend, NullBackend, RenderBackend,
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
    /// The construction thread owns every live backend/context beneath this
    /// session. Backends are non-Send; this makes the affinity explicit at
    /// lifecycle boundaries as well.
    owner_thread: ThreadId,
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
            owner_thread: std::thread::current().id(),
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
            owner_thread: std::thread::current().id(),
        }
    }

    /// Binds a GPU context for a later `set_backend(Gpu)` call.
    ///
    /// A staged context is still a live thread-affine native resource. Replacing
    /// it therefore closes the old one on the owner thread rather than letting
    /// `Drop` silently skip its native shutdown protocol.
    #[allow(dead_code)] // Retained for crate-local staged recipe transitions and regression coverage.
    pub(crate) fn set_gpu_context(
        &mut self,
        mut ctx: Box<dyn IGraphicsContext>,
    ) -> Result<(), Error> {
        self.require_owner("set_gpu_context")?;
        if let Some(mut previous) = self.gpu_ctx.take() {
            if let Err(error) = previous.try_shutdown() {
                // The old native resource remains live after a failed checked
                // teardown. Keep it owned by this session and close the
                // incoming resource before returning the typed failure.
                self.gpu_ctx = Some(previous);
                return match ctx.try_shutdown() {
                    Ok(()) => Err(error),
                    Err(cleanup_error) => Err(cleanup_error.with_source(error)),
                };
            }
        }
        self.gpu_ctx = Some(ctx);
        Ok(())
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
        self.require_owner("initialize")?;
        self.width = width;
        self.height = height;
        self.backend.resize(width, height)
    }

    /// Starts a session on a native target the factory has already prepared.
    /// The backend reports the drawable extent it actually adopted so the
    /// session never assumes the requested logical size matches a corrected
    /// swapchain/client extent.
    pub(crate) fn initialize_prepared(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.require_owner("initialize_prepared")?;
        let (actual_width, actual_height) = self.backend.initialize_prepared(width, height)?;
        self.width = actual_width.max(1);
        self.height = actual_height.max(1);
        Ok(())
    }

    pub fn shutdown(&mut self) {
        if let Err(error) = self.try_shutdown() {
            crate::core::log::error_fn(format!("RenderSession: {}", error.short_what()));
        }
    }

    /// Closes the active backend and any staged context on the owner thread.
    /// A failure deliberately leaves the still-live owner in place for a
    /// later retry by the recovery or Drop path.
    pub(crate) fn try_shutdown(&mut self) -> Result<(), Error> {
        self.require_owner("shutdown")?;
        self.backend.try_shutdown()?;
        if let Some(mut staged_context) = self.gpu_ctx.take() {
            if let Err(error) = staged_context.try_shutdown() {
                self.gpu_ctx = Some(staged_context);
                return Err(error);
            }
        }
        self.width = 0;
        self.height = 0;
        Ok(())
    }

    pub fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.require_owner("resize")?;
        self.backend.resize(width, height)?;
        self.width = width;
        self.height = height;
        // swapchain/缓冲 resize 后内容丢失，下一帧须全帧重绘。
        self.force_full_frame = true;
        Ok(())
    }

    /// 运行时切换后端；下一帧将强制 FullRedraw。
    pub fn set_backend(&mut self, kind: BackendKind) -> Result<(), Error> {
        self.require_owner("set_backend")?;
        let resolved = resolve_kind(kind);
        // CPU/Null backends do not consume a staged GPU context. Keep it
        // alive for a later explicit GPU switch; shutdown owns it otherwise.
        let gpu_ctx = if resolved == BackendKind::Gpu {
            self.gpu_ctx.take()
        } else {
            None
        };
        let replacement = create_backend(resolved, gpu_ctx)?;
        self.backend.shutdown();
        self.backend = replacement;
        if self.width > 0 && self.height > 0 {
            self.backend.resize(self.width, self.height)?;
        }
        self.force_full_frame = true;
        Ok(())
    }

    pub fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        if let Err(error) = self.require_owner("begin_frame") {
            return RenderOutcome::Failed(crate::draw::engine::GraphicsFailure::from_error(error));
        }
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
        if let Err(error) = self.require_owner("end_frame") {
            return RenderOutcome::Failed(crate::draw::engine::GraphicsFailure::from_error(error));
        }
        crate::draw::pipeline::frame::end_frame(self.backend.surface())
    }

    pub fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        debug_assert!(self.require_owner("canvas_2d").is_ok());
        self.backend.surface().canvas()
    }

    pub fn cpu_backend(&self) -> Option<&CpuBackend> {
        self.backend.as_any().downcast_ref()
    }

    pub fn cpu_backend_mut(&mut self) -> Option<&mut CpuBackend> {
        self.backend.as_any_mut().downcast_mut()
    }

    #[cfg(all(test, feature = "opengles"))]
    pub(crate) fn native_gpu_backend_mut(&mut self) -> Option<&mut NativeGpuBackend> {
        self.backend.as_any_mut().downcast_mut()
    }

    pub fn null_backend_mut(&mut self) -> Option<&mut NullBackend> {
        self.backend.as_any_mut().downcast_mut()
    }

    pub fn backend_mut(&mut self) -> &mut dyn RenderBackend {
        debug_assert!(self.require_owner("backend_mut").is_ok());
        &mut *self.backend
    }

    pub fn backend(&self) -> &dyn RenderBackend {
        debug_assert!(self.require_owner("backend").is_ok());
        &*self.backend
    }

    fn require_owner(&self, operation: &str) -> Result<(), Error> {
        if std::thread::current().id() == self.owner_thread {
            Ok(())
        } else {
            Err(Error::new(
                crate::core::Errc::InvalidState,
                format!("RenderSession::{operation} must run on its owning graphics thread"),
            ))
        }
    }
}

impl Drop for RenderSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn resolve_kind(kind: BackendKind) -> BackendKind {
    match kind {
        BackendKind::Auto => BackendKind::Cpu,
        other => other,
    }
}

#[cfg(test)]
#[path = "../../tests/draw/pipeline/session.rs"]
mod tests;
