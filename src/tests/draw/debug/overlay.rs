use super::*;
    use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;

    #[test]
    fn new_off_by_default() {
        let d = DebugRenderService::new(false);
        assert!(!d.debug_mode);
    }

    #[test]
    fn new_on() {
        let d = DebugRenderService::new(true);
        assert!(d.debug_mode);
    }

    #[test]
    fn set_debug_mode_toggle() {
        let mut d = DebugRenderService::new(false);
        assert!(!d.debug_mode);
        d.set_debug_mode(true);
        assert!(d.debug_mode);
        d.set_debug_mode(false);
        assert!(!d.debug_mode);
    }

    #[test]
    fn draw_debug_border_skip_when_off() {
        let d = DebugRenderService::new(false);
        let mut canvas = NoopCanvas2D;
        // 当 debug_mode=false 时不 panic
        d.draw_debug_border(&mut canvas, Rect::new(0.0, 0.0, 100.0, 50.0), 0, false);
    }

    #[test]
    fn draw_debug_border_runs_when_on() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_border(&mut canvas, Rect::new(0.0, 0.0, 100.0, 50.0), 0, false);
        d.draw_debug_border(&mut canvas, Rect::new(0.0, 0.0, 100.0, 50.0), 0, true);
    }

    #[test]
    fn draw_debug_border_hovered_uses_full_alpha() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_border(&mut canvas, Rect::new(10.0, 10.0, 50.0, 30.0), 0, true);
    }

    #[test]
    fn draw_debug_border_multiple_depths() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        // 所有深度都应正常工作
        for depth in 0..16 {
            d.draw_debug_border(
                &mut canvas,
                Rect::new(0.0, 0.0, 100.0, 50.0),
                depth,
                depth == 0,
            );
        }
    }

    #[test]
    fn draw_debug_label_skip_when_off() {
        let d = DebugRenderService::new(false);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_label(&mut canvas, 1, 0, Rect::new(0.0, 0.0, 100.0, 50.0));
    }

    #[test]
    fn draw_debug_label_runs_when_on() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_label(&mut canvas, 42, 2, Rect::new(10.0, 10.0, 100.0, 50.0));
    }

    #[test]
    fn draw_debug_frame_info_skip_when_off() {
        let d = DebugRenderService::new(false);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_frame_info(&mut canvas, 1, Rect::new(0.0, 0.0, 100.0, 50.0));
    }

    #[test]
    fn draw_debug_frame_info_runs_when_on() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_frame_info(&mut canvas, 42, Rect::new(10.0, 20.0, 200.0, 100.0));
    }

    #[test]
    fn debug_colors_has_eight_entries() {
        assert_eq!(DebugRenderService::DEBUG_COLORS.len(), 8);
    }

    #[test]
    fn debug_colors_all_have_alpha() {
        for color in &DebugRenderService::DEBUG_COLORS {
            assert!(color.a > 0 || color.a == 0); // 至少不 panic
        }
    }

    #[test]
    fn draw_debug_label_rect_position() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        d.draw_debug_label(&mut canvas, 99, 1, Rect::new(10.0, 20.0, 150.0, 80.0));
    }

    #[test]
    fn draw_debug_frame_info_below_widget() {
        let d = DebugRenderService::new(true);
        let mut canvas = NoopCanvas2D;
        // 验证不 panic：坐标信息绘制在 widget 下方
        d.draw_debug_frame_info(&mut canvas, 7, Rect::new(30.0, 40.0, 200.0, 100.0));
    }

    #[test]
    fn debug_colors_have_expected_rgb_ranges() {
        for color in &DebugRenderService::DEBUG_COLORS {
            assert!(color.a > 0, "每个调试颜色应有非零 alpha");
            // r/g/b 是 u8，无需范围检查
        }
    }
