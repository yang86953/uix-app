use super::*;
    use crate::draw::engine::RenderOutcome;
    use crate::draw::pipeline::frame;
    use crate::draw::traits::UpdateStrategy;

    #[test]
    fn null_backend_kind_and_capabilities() {
        let backend = NullBackend::new();
        assert_eq!(backend.kind(), BackendKind::Null);
        assert!(backend.capabilities().partial_redraw);
        assert!(!backend.capabilities().offscreen);
    }

    #[test]
    fn null_backend_begin_frame_idle_on_empty_dirty() {
        let mut backend = NullBackend::new();
        let caps = backend.capabilities();
        let outcome = frame::begin_frame(
            UpdateStrategy::DirtyRects(vec![]),
            backend.surface(),
            800,
            600,
            caps,
        );
        assert_eq!(outcome, RenderOutcome::Idle);
    }

    #[test]
    fn null_backend_canvas_ops_no_panic() {
        let mut backend = NullBackend::new();
        let canvas = backend.surface().canvas();
        canvas.fill_rect(
            Rect::new(0.0, 0.0, 10.0, 10.0),
            crate::draw::primitives::color::Color::from_rgba(255, 0, 0, 255),
            None,
        );
    }
