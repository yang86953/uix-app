use super::*;
use crate::draw::backend::DamageRegion;
use crate::native::traits::present::{
    GraphicsBackend, GraphicsContextCaps, IGraphicsContext, PresentDamage,
};
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountingGraphicsContext {
    shutdowns: Arc<AtomicUsize>,
}

struct FailingShutdownGraphicsContext {
    attempts: Arc<AtomicUsize>,
    message: &'static str,
}

impl IGraphicsContext for CountingGraphicsContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, false, 1.0)
    }

    fn initialize(
        &mut self,
        _native_window: *mut c_void,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<()> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> crate::core::Result<()> {
        Ok(())
    }

    fn make_current(&mut self) -> crate::core::Result<()> {
        Ok(())
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        self.shutdowns.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn read_pixels(
        &mut self,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<Vec<u32>> {
        Ok(Vec::new())
    }

    fn width(&self) -> i32 {
        1
    }

    fn height(&self) -> i32 {
        1
    }
}

fn counting_context(shutdowns: Arc<AtomicUsize>) -> Box<dyn IGraphicsContext> {
    Box::new(CountingGraphicsContext { shutdowns })
}

impl IGraphicsContext for FailingShutdownGraphicsContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, false, 1.0)
    }

    fn initialize(
        &mut self,
        _native_window: *mut c_void,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<()> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> crate::core::Result<()> {
        Ok(())
    }

    fn make_current(&mut self) -> crate::core::Result<()> {
        Ok(())
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Err(crate::core::Error::new(
            crate::core::Errc::PlatformError,
            self.message,
        ))
    }

    fn read_pixels(
        &mut self,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<Vec<u32>> {
        Ok(Vec::new())
    }

    fn width(&self) -> i32 {
        1
    }

    fn height(&self) -> i32 {
        1
    }
}

#[test]
fn set_backend_to_null_forces_full_frame_once() {
    let mut session = RenderSession::new(BackendKind::Cpu).expect("Cpu 会话");
    session.initialize(10, 10).expect("init");
    session.set_backend(BackendKind::Null).expect("switch");
    assert_eq!(session.backend_kind(), BackendKind::Null);
    let outcome = session.begin_frame(UpdateStrategy::DirtyRects(vec![]));
    assert_eq!(outcome, RenderOutcome::FrameReady(DamageRegion::full()));
    session.end_frame();
    let outcome = session.begin_frame(UpdateStrategy::DirtyRects(vec![]));
    assert_eq!(outcome, RenderOutcome::Idle);
}

#[test]
fn auto_resolves_to_cpu() {
    let session = RenderSession::new(BackendKind::Auto).expect("Auto 会话");
    assert_eq!(session.backend_kind(), BackendKind::Cpu);
}

#[test]
fn replacing_staged_context_shuts_the_previous_context_once() {
    let previous_shutdowns = Arc::new(AtomicUsize::new(0));
    let current_shutdowns = Arc::new(AtomicUsize::new(0));
    let mut session = RenderSession::new(BackendKind::Cpu).expect("Cpu 会话");

    session
        .set_gpu_context(counting_context(Arc::clone(&previous_shutdowns)))
        .expect("stage first context");
    session
        .set_gpu_context(counting_context(Arc::clone(&current_shutdowns)))
        .expect("replace staged context");

    assert_eq!(previous_shutdowns.load(Ordering::SeqCst), 1);
    assert_eq!(current_shutdowns.load(Ordering::SeqCst), 0);
    session.shutdown();
    assert_eq!(current_shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn failed_staged_context_shutdown_keeps_the_old_context_and_closes_the_replacement() {
    let failed_attempts = Arc::new(AtomicUsize::new(0));
    let replacement_shutdowns = Arc::new(AtomicUsize::new(0));
    let mut session = RenderSession::new(BackendKind::Cpu).expect("Cpu session");

    session
        .set_gpu_context(Box::new(FailingShutdownGraphicsContext {
            attempts: Arc::clone(&failed_attempts),
            message: "injected shutdown failure",
        }))
        .expect("stage context");

    let error = session
        .set_gpu_context(counting_context(Arc::clone(&replacement_shutdowns)))
        .expect_err("shutdown failure must be propagated");
    assert_eq!(error.code(), crate::core::Errc::PlatformError);
    assert_eq!(failed_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(replacement_shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn session_shutdown_retries_a_staged_checked_shutdown_failure() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let mut session = RenderSession::new(BackendKind::Cpu).expect("Cpu session");
    session
        .set_gpu_context(Box::new(FailingShutdownGraphicsContext {
            attempts: Arc::clone(&attempts),
            message: "injected shutdown failure",
        }))
        .expect("stage context");

    session.shutdown();
    assert_eq!(attempts.load(Ordering::SeqCst), 1);

    session.shutdown();
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[test]
fn staged_replacement_preserves_both_checked_shutdown_failures() {
    let previous_attempts = Arc::new(AtomicUsize::new(0));
    let replacement_attempts = Arc::new(AtomicUsize::new(0));
    let mut session = RenderSession::new(BackendKind::Cpu).expect("Cpu session");
    session
        .set_gpu_context(Box::new(FailingShutdownGraphicsContext {
            attempts: Arc::clone(&previous_attempts),
            message: "injected retained context shutdown failure",
        }))
        .expect("stage context");

    let error = session
        .set_gpu_context(Box::new(FailingShutdownGraphicsContext {
            attempts: Arc::clone(&replacement_attempts),
            message: "injected replacement context shutdown failure",
        }))
        .expect_err("both checked shutdown failures must remain observable");

    assert_eq!(
        error.message(),
        "injected replacement context shutdown failure"
    );
    assert_eq!(
        error
            .source_error()
            .expect("retained context failure")
            .message(),
        "injected retained context shutdown failure"
    );
    assert_eq!(previous_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(replacement_attempts.load(Ordering::SeqCst), 1);
}

#[test]
fn cpu_or_null_switch_keeps_staged_context_until_session_shutdown() {
    let shutdowns = Arc::new(AtomicUsize::new(0));
    let mut session = RenderSession::new(BackendKind::Cpu).expect("Cpu 会话");
    session
        .set_gpu_context(counting_context(Arc::clone(&shutdowns)))
        .expect("stage context");

    session
        .set_backend(BackendKind::Null)
        .expect("switch to null");
    assert_eq!(shutdowns.load(Ordering::SeqCst), 0);

    session.shutdown();
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn dropping_session_closes_a_staged_context_once() {
    let shutdowns = Arc::new(AtomicUsize::new(0));
    {
        let mut session = RenderSession::new(BackendKind::Cpu).expect("Cpu 会话");
        session
            .set_gpu_context(counting_context(Arc::clone(&shutdowns)))
            .expect("stage context");
    }
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}
