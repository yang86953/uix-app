//! graphics 会话与后端切换集成测试。

use uix_graphics::backend::BackendKind;
use uix_graphics::engine::RenderOutcome;
use uix_graphics::engine::cpu::software::SoftwareEngine;
use uix_graphics::backend::DamageRegion;
use uix_graphics::pipeline::RenderSession;
use uix_graphics::traits::{GraphicsEngine, UpdateStrategy};
use uix_platform::Rect;

#[test]
fn software_engine_delegates_to_shared_frame_logic() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(20, 20).expect("init");
    let outcome = engine.begin_frame(UpdateStrategy::DirtyRects(vec![]));
    assert_eq!(outcome, RenderOutcome::Idle);
}

#[test]
fn software_engine_set_backend_switches_to_null() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(10, 10).expect("init");
    assert_eq!(engine.session().backend_kind(), BackendKind::Cpu);
    engine
        .session_mut()
        .set_backend(BackendKind::Null)
        .expect("switch");
    assert_eq!(engine.session().backend_kind(), BackendKind::Null);
    let outcome = engine.begin_frame(UpdateStrategy::FullRedraw);
    assert_eq!(outcome, RenderOutcome::Present(DamageRegion::full()));
    engine.end_frame();
}

#[test]
fn render_session_cpu_dirty_rect_partial_redraw() {
    let mut session = RenderSession::new(BackendKind::Cpu).expect("session");
    session.initialize(50, 50).expect("init");
    let outcome = session.begin_frame(UpdateStrategy::DirtyRects(vec![Rect::new(
        0.0, 0.0, 10.0, 10.0,
    )]));
    assert_eq!(outcome, RenderOutcome::Present(DamageRegion::full()));
    session.end_frame();
}
