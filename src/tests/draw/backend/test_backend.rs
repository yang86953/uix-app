use crate::draw::backend::contract::{BackendKind, RenderBackend};
use crate::draw::backend::test_backend::*;
use crate::draw::renderer::lifecycle;
use crate::draw::UpdateStrategy;
use crate::tests::common::*;

#[test]
fn test_backend_kind_and_capabilities() {
    let backend = TestBackend::new();
    assert_eq!(backend.kind(), BackendKind::Test);
    assert!(backend.capabilities().partial_redraw);
    assert!(!backend.capabilities().offscreen);
}

#[test]
fn test_backend_begin_frame_idle_on_empty_dirty() {
    let mut backend = TestBackend::new();
    let caps = backend.capabilities();
    let outcome = lifecycle::begin_frame(
        UpdateStrategy::DirtyRects(vec![]),
        backend.surface(),
        800,
        600,
        caps,
    );
    assert_eq!(outcome, RenderOutcome::Idle);
}
