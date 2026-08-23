    // 复用被测模块中的单选组组件与事件类型。
    use super::*;
    // 引入无像素副作用画布与绘制事实录制入口。
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::painting::{DisplayList, PaintContext as DrawPaintContext, PaintOp};
    use crate::draw::resources::font::font_service::FontService;
    // 引入 UI 绘制上下文、主题与渲染 trait。
    use crate::ui::theme::Theme;
    use crate::ui::widget_runtime::paint_context::PaintContext as UiPaintContext;
    use crate::ui::widget_runtime::traits::WidgetRender;
    // 引入事件行为 trait 与修饰键类型。
    use crate::ui::{KeyMod, widget_runtime::traits::EventHandler};

    // 分数坐标下焦点圈、外圈和内点必须保持同心。
    #[test]
    fn fractional_position_records_concentric_indicator_circles() {
        // 构造选中首项且持有焦点的单选组。
        let mut radio = Radio::new().options(["female", "male"]).default_selected(0);
        radio.focused = true;
        // 创建不写真实像素的录制画布。
        let mut canvas = NoopCanvas2D;
        let font_service = FontService::new();
        let image_service = crate::draw::resources::image::ImageService::new();
        let mut draw_context = DrawPaintContext::new_for_test(
            &mut canvas,
            crate::draw::FontHandle::new(0),
            &font_service,
            &image_service,
            96.0,
            1.0,
            crate::draw::geometry::spatial::Orientation::YDown,
            160,
            64,
        );
        let mut list = DisplayList::new();
        let tokens = Theme::antd_light().tokens_arc();
        // 使用非整数位置触发此前圆角矩形与圆形不同的对齐路径。
        draw_context.with_recorder(&mut list, |draw_context| {
            let mut ui_context = UiPaintContext::new(draw_context, tokens);
            radio.render_radio_item(&mut ui_context, 0, "female", 10.3, 20.6, 70.0, true);
        });

        // 前两项分别是焦点圈和外圈，第三项是选中内点。
        let circle_centers = list
            .ops()
            .iter()
            .take(3)
            .map(|op| match op {
                PaintOp::StrokeCircle { cx, cy, .. } | PaintOp::FillCircle { cx, cy, .. } => {
                    (*cx, *cy)
                }
                other => panic!("单选指示器应只使用圆形绘制事实，实际为 {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(circle_centers.len(), 3);
        for (x, y) in circle_centers {
            assert!((x - 17.3).abs() < f32::EPSILON * 8.0);
            assert!((y - 20.6).abs() < f32::EPSILON * 8.0);
        }
    }

    // Radio 的脏区必须包含越过布局边界的焦点圈。
    #[test]
    fn dirty_rect_covers_focus_indicator_overflow() {
        let radio = Radio::new().options(["A"]);
        let frame = Rect::new(10.25, 20.75, 120.0, 32.0);
        let dirty = WidgetRender::dirty_rect(&radio, frame);
        // 中等尺寸焦点圈最左像素位于布局边界外 1.75px。
        assert!(dirty.x <= frame.x - 1.75);
        assert!(dirty.y < frame.y);
        assert!(dirty.x + dirty.w > frame.x + frame.w);
        assert!(dirty.y + dirty.h > frame.y + frame.h);
    }

    // 方向键导航有界：边界处不再循环回绕。
    #[test]
    fn arrow_navigation_is_bounded_at_edges() {
        // 构造三个选项、选中首项的单选组。
        let mut radio = Radio::new().options(["A", "B", "C"]).default_selected(0);
        // 在首项向左移动。
        let _ = EventHandler::on_event(
            &mut radio,
            &SystemEvent::KeyDown {
                key: KeyCode::Left,
                mods: KeyMod::NONE,
            },
        );
        // 有界语义下停在首项，不回绕到末项。
        assert_eq!(radio.current_index(), Some(0));
        // 在末项向右移动。
        let mut radio = Radio::new().options(["A", "B", "C"]).default_selected(2);
        let _ = EventHandler::on_event(
            &mut radio,
            &SystemEvent::KeyDown {
                key: KeyCode::Right,
                mods: KeyMod::NONE,
            },
        );
        // 有界语义下停在末项，不回绕到首项。
        assert_eq!(radio.current_index(), Some(2));
        // 组内中间项仍可正常双向移动。
        let mut radio = Radio::new().options(["A", "B", "C"]).default_selected(1);
        let _ = EventHandler::on_event(
            &mut radio,
            &SystemEvent::KeyDown {
                key: KeyCode::Down,
                mods: KeyMod::NONE,
            },
        );
        assert_eq!(radio.current_index(), Some(2));
    }

    // Home/End 分别跳到组内首项与末项。
    #[test]
    fn home_and_end_jump_to_group_edges() {
        // 构造三个选项、选中中项的单选组。
        let mut radio = Radio::new().options(["A", "B", "C"]).default_selected(1);
        // Home 跳到首项。
        let _ = EventHandler::on_event(
            &mut radio,
            &SystemEvent::KeyDown {
                key: KeyCode::Home,
                mods: KeyMod::NONE,
            },
        );
        assert_eq!(radio.current_index(), Some(0));
        // End 跳到末项。
        let _ = EventHandler::on_event(
            &mut radio,
            &SystemEvent::KeyDown {
                key: KeyCode::End,
                mods: KeyMod::NONE,
            },
        );
        assert_eq!(radio.current_index(), Some(2));
    }
