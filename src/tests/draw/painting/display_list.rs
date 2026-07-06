use super::*;
    use crate::draw::font::font_service::FontService;
    use crate::draw::traits::GraphicsEngine;
    use crate::draw::Color;
    use crate::draw::NullEngine;

    const RED_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    #[test]
    fn display_list_stores_ops() {
        let mut list = DisplayList::new();
        list.push(PaintOp::FillRect {
            rect: Rect::new(1.0, 2.0, 10.0, 10.0),
            color: Color::red(),
            radius: None,
        });
        assert_eq!(list.len(), 1);
        assert!(!list.is_empty());
    }

    #[test]
    fn display_list_stores_draw_text_in_frame_and_set_font() {
        let mut list = DisplayList::new();
        list.push(PaintOp::SetFont {
            font: FontHandle::new(1),
        });
        list.push(PaintOp::DrawTextInFrame {
            text: "Nav".into(),
            rect: Rect::new(0.0, 0.0, 40.0, 20.0),
            color: Color::black(),
            font_size: 14.0,
        });
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn replay_canvas_draw_image_with_service() {
        let svc = ImageService::new();
        let handle = svc.load_from_bytes(RED_PNG).expect("load png");

        let mut list = DisplayList::new();
        list.push(PaintOp::DrawImage {
            handle,
            bounds: Rect::new(0.0, 0.0, 4.0, 4.0),
            fit: true,
        });

        let mut engine = NullEngine::new();
        engine.initialize(8, 8).expect("init");
        let canvas = engine.canvas_2d();
        list.replay_canvas(
            canvas,
            Default::default(),
            &FontService::new(),
            Some(&svc),
            8.0,
        );
    }
