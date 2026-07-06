use super::*;
    use crate::draw::font::font_service::FontService;

    #[test]
    fn new_sets_font_and_service() {
        let fs = FontService::new();
        let fh = FontHandle::default();
        let trs = TextRenderService::new(fh, &fs, 500.0);
        assert_eq!(trs.font, fh);
        assert_eq!(trs.max_text_width, 500.0);
    }

    #[test]
    fn set_font_updates_handle() {
        let fs = FontService::new();
        let fh1 = FontHandle::new(1);
        let fh2 = FontHandle::new(2);
        let mut trs = TextRenderService::new(fh1, &fs, 500.0);
        assert_eq!(*trs.font(), fh1);
        trs.set_font(fh2);
        assert_eq!(*trs.font(), fh2);
    }

    #[test]
    fn set_max_text_width_updates() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        assert_eq!(trs.max_text_width, 500.0);
        trs.set_max_text_width(800.0);
        assert_eq!(trs.max_text_width, 800.0);
        trs.set_max_text_width(0.0);
        assert_eq!(trs.max_text_width, 0.0);
    }

    #[test]
    fn draw_text_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.draw_text(&mut canvas, "", Point::new(0.0, 0.0), Color::black(), 14.0);
    }

    #[test]
    fn draw_text_baseline_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.draw_text_baseline(&mut canvas, "", 0.0, 0.0, Color::black(), 14.0);
    }

    #[test]
    fn text_center_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.text_center(
            &mut canvas,
            "",
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Color::black(),
            14.0,
        );
    }

    #[test]
    fn draw_text_in_frame_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.draw_text_in_frame(
            &mut canvas,
            "",
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Color::black(),
            14.0,
        );
    }

    #[test]
    fn draw_text_wrapped_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.draw_text_wrapped(
            &mut canvas,
            "",
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Color::black(),
            14.0,
        );
    }

    #[test]
    fn selection_rects_empty_text() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        let rects = trs.selection_rects(&mut canvas, "", 14.0, Point::new(0.0, 0.0), 0, 0);
        assert!(rects.is_empty());
    }

    #[test]
    fn selection_rects_invalid_range() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        // start > end → empty
        let rects = trs.selection_rects(&mut canvas, "hello", 14.0, Point::new(0.0, 0.0), 3, 1);
        assert!(rects.is_empty());
    }

    #[test]
    fn visual_center_y_default_font() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let center = trs.visual_center_y(Rect::new(0.0, 0.0, 100.0, 50.0), 14.0);
        // 只是验证不 panic 且有合理返回值
        assert!(center >= 0.0);
    }

    #[test]
    fn draw_text_with_selection_no_selection() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        // selection=None 时只绘制文本
        trs.draw_text_with_selection(
            &mut canvas,
            "",
            Point::new(0.0, 0.0),
            Color::black(),
            14.0,
            None,
            Color::blue(),
        );
    }

    #[test]
    fn font_service_ref_accessible() {
        let fs = FontService::new();
        let trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        // font_service 指针应指向传入的 fs
        assert_eq!(trs.font_service as *const _, &fs as *const _);
    }

    #[test]
    fn draw_text_spatial_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas_s = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        let spatial = crate::draw::spatial::SpatialContext::new(
            &mut canvas_s,
            96.0,
            1.0,
            crate::draw::spatial::Orientation::YDown,
            800,
            600,
        );
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.draw_text_spatial(
            &mut canvas,
            &spatial,
            "",
            Vec3::zero(),
            Color::black(),
            PhysicalUnit::Px(14.0),
        );
    }

    #[test]
    fn text_center_spatial_empty_returns_early() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas_s = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        let spatial = crate::draw::spatial::SpatialContext::new(
            &mut canvas_s,
            96.0,
            1.0,
            crate::draw::spatial::Orientation::YDown,
            800,
            600,
        );
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        trs.text_center_spatial(
            &mut canvas,
            &spatial,
            "",
            AABB3D::new(Vec3::zero(), Vec3::new(100.0, 50.0, 0.0)),
            Color::black(),
            PhysicalUnit::Px(14.0),
        );
    }

    #[test]
    fn measure_text_returns_size() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let sz = trs.measure_text("hello", 14.0);
        // 默认字体（ab_glyph）能成功布局，返回实际宽高
        assert!(sz.w > 0.0);
        assert!(sz.h > 0.0);
    }

    #[test]
    fn measure_text_empty_returns_minimal_height() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let sz = trs.measure_text("", 14.0);
        // 空文本：宽度为 0，高度为行高
        assert_eq!(sz.w, 0.0);
        assert!(sz.h > 0.0);
    }

    #[test]
    fn measure_text_wrapped_returns_size() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let sz = trs.measure_text_wrapped("hello world", 14.0, 100.0);
        // 换行模式下返回实际布局尺寸
        assert!(sz.w > 0.0);
        assert!(sz.h > 0.0);
    }

    #[test]
    fn text_hit_test_works_with_default_font() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let result = trs.text_hit_test("hello", 14.0, Point::new(0.0, 0.0));
        // 默认字体可用，命中测试返回有效索引
        assert!(result.is_some());
    }

    #[test]
    fn text_cursor_x_returns_position() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let x = trs.text_cursor_x("hello", 14.0, 0);
        // 默认字体可用，返回有效 x 坐标
        assert!(x >= 0.0);
        // 索引 3 应在索引 0 之后
        let x3 = trs.text_cursor_x("hello", 14.0, 3);
        assert!(x3 >= x);
    }

    #[test]
    fn draw_text_with_selection_empty_renders_text() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        // 空 selection 应只绘制文本
        trs.draw_text_with_selection(
            &mut canvas,
            "text",
            Point::new(0.0, 0.0),
            Color::black(),
            14.0,
            None,
            Color::blue(),
        );
    }

    #[test]
    fn draw_text_with_selection_empty_range_skips_bg() {
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        // selection(0,0) 是空范围 → 跳过背景矩形
        trs.draw_text_with_selection(
            &mut canvas,
            "text",
            Point::new(0.0, 0.0),
            Color::black(),
            14.0,
            Some((0, 0)),
            Color::blue(),
        );
    }

    #[test]
    fn blit_to_with_empty_layout_no_panic() {
        use crate::draw::font::text_backend::TextLayout;
        let fs = FontService::new();
        let mut trs = TextRenderService::new(FontHandle::default(), &fs, 500.0);
        let mut canvas = crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
        let layout = TextLayout {
            width: 0.0,
            height: 0.0,
            lines: vec![],
            glyphs: vec![],
        };
        trs.blit_to(
            &mut canvas,
            &layout,
            Point::new(0.0, 0.0),
            Color::black(),
            14.0,
        );
    }
