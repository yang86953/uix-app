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
