use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::view::{embed, ViewAdapter};
use crate::ui::widgets::{ResultType, ResultView};
use crate::ui::{AccessibilityRole, WidgetComponent};
use std::cell::Cell;
use std::rc::Rc;

fn render_display_list(result: &ResultView, frame: Rect, surface_size: (i32, i32)) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::command::DisplayList::new();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            font,
            &fonts,
            &images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            surface_size.0,
            surface_size.1,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(result, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn constrained_result_clips_and_wraps_content_inside_the_actual_frame() {
    let result = ResultView::new(ResultType::Warning)
        .title("这是一段很长的结果标题，需要在窄宽度内保持可读")
        .subtitle("副标题同样不能越过组件边界或覆盖下方的操作按钮")
        .extra_text("查看完整的质量核验报告");
    let display_list = render_display_list(&result, Rect::new(10.0, 8.0, 140.0, 180.0), (170, 210));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 8.0, w: 140.0, h: 180.0 } }"),
        "all ResultView paint must be clipped to its frame: {display_list}"
    );
    assert!(
        display_list.contains("DrawTextWrapped") || display_list.contains('…'),
        "long content must wrap or elide: {display_list}"
    );
    assert!(
        !display_list.contains("w: -") && !display_list.contains("h: -"),
        "{display_list}"
    );
}

#[test]
fn result_action_hit_geometry_uses_the_current_constrained_frame_before_render() {
    let result = ResultView::new(ResultType::Success).extra_text("查看完整报告");
    let frame = Rect::new(40.0, 30.0, 140.0, 180.0);
    let hit = EventHandler::hit_test_frame(&result, frame);

    assert!(hit.x >= frame.x && hit.y >= frame.y, "{hit:?}");
    assert!(hit.x + hit.w <= frame.x + frame.w, "{hit:?}");
    assert!(hit.y + hit.h <= frame.y + frame.h, "{hit:?}");
    assert!(hit.w >= 24.0 && hit.h >= 24.0, "{hit:?}");
}

#[test]
fn releasing_pointer_outside_result_action_cancels_activation() {
    let mut result = ResultView::new(ResultType::Error).extra_text("重试");
    let action = EventHandler::hit_test_frame(&result, Rect::new(0.0, 0.0, 400.0, 300.0));
    let center = Point::new(action.x + action.w * 0.5, action.y + action.h * 0.5);
    assert_eq!(
        result.on_event(&SystemEvent::PointerDown {
            pos: center,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        result.on_event(&SystemEvent::PointerUp {
            pos: Point::new(0.0, 0.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}

#[test]
fn result_default_localized_content_is_accessible() {
    let success = ResultView::new(ResultType::Success)
        .snapshot_fields()
        .accessibility();
    assert_eq!(success.role, AccessibilityRole::Status);
    assert_eq!(success.name.as_deref(), Some("操作成功"));

    let not_found = ResultView::new(ResultType::NotFound)
        .snapshot_fields()
        .accessibility();
    assert_eq!(not_found.name.as_deref(), Some("页面不存在"));
    assert_eq!(
        not_found.state.value_text.as_deref(),
        Some("请检查您访问的地址是否正确")
    );
}

#[test]
fn result_action_is_focusable_and_only_claims_its_button_region() {
    let mut result = ResultView::new(ResultType::Error)
        .title("保存失败")
        .extra_text("重试");
    let outside = SystemEvent::PointerDown {
        pos: Point::new(20.0, 20.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    let action = SystemEvent::PointerDown {
        pos: Point::new(200.0, 208.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&result), 1);
    assert_eq!(result.on_event(&outside), EventResult::NotHandled);
    assert_eq!(result.on_event(&action), EventResult::Handled);
    let accessibility = result.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.name.as_deref(), Some("重试"));
    assert_eq!(accessibility.state.value_text.as_deref(), Some("保存失败"));
}

#[test]
fn result_pointer_and_keyboard_activation_reach_public_click_handler() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut tree = ViewAdapter::build(
        embed(ResultView::new(ResultType::Info).extra_text("继续")).on_click_fn(move || {
            observed.set(observed.get() + 1);
        }),
    );
    let id = tree.root_id().expect("result root");
    tree.get_mut(id)
        .expect("result node")
        .set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
    let pos = Point::new(200.0, 208.0);

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);

    let body = Point::new(20.0, 20.0);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: body,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos: body,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(calls.get(), 1);

    tree.set_focus(Some(id));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyUp {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn result_without_action_remains_a_noninteractive_status() {
    let mut result = ResultView::new(ResultType::Success).title("完成");
    assert_eq!(WidgetComponent::tab_index(&result), 0);
    assert_eq!(
        result.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        result.snapshot_fields().accessibility().role,
        AccessibilityRole::Status
    );
}
