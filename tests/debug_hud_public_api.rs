use uix::core::Point;
use uix::draw::debug::DebugHudState;

#[test]
fn hud_toggles_between_expanded_and_collapsed() {
    let hud = DebugHudState::new();
    assert!(!hud.collapsed());
    assert!(hud.toggle_collapsed());
    assert!(hud.collapsed());
    assert!(!hud.set_collapsed(false));
    assert!(!hud.collapsed());
}

#[test]
fn hud_layout_keeps_default_anchor_and_reports_button_and_handle() {
    let hud = DebugHudState::new();
    let layout = hud.layout(9, 800, 600);

    // 默认锚点：右上角，留 HUD_MARGIN 边距。
    assert_eq!(layout.bounds.y, 8.0);
    assert_eq!(
        layout.bounds.x + layout.bounds.w,
        800.0 - 8.0,
        "expanded HUD anchors to the right edge"
    );
    assert!(hud.bounds().is_some());
    // 折叠按钮贴面板右上，标题条是去掉按钮后的标题行。
    assert!(layout.collapse_button.x + layout.collapse_button.w <= layout.bounds.x + layout.bounds.w);
    assert!(layout.drag_handle.w < layout.bounds.w);
    assert!(!layout.collapsed);

    let collapsed = {
        hud.set_collapsed(true);
        hud.layout(9, 800, 600)
    };
    assert!(collapsed.collapsed);
    assert!(collapsed.bounds.h < layout.bounds.h);
}

#[test]
fn hud_drag_records_custom_position_and_clamps_into_surface() {
    let hud = DebugHudState::new();
    hud.layout(9, 800, 600);
    let bounds = hud.bounds().unwrap();
    assert!(hud.pointer_down(Point::new(bounds.x + 10.0, bounds.y + 8.0)));
    assert!(hud.is_dragging());
    assert!(hud.pointer_move(Point::new(200.0, 150.0)));
    assert!(hud.pointer_up(Point::new(200.0, 150.0)));
    assert!(!hud.is_dragging());

    let moved = hud.layout(9, 800, 600);
    assert_eq!(
        hud.position(),
        Some(Point::new(moved.bounds.x, moved.bounds.y))
    );
    assert!(moved.bounds.x >= 8.0 && moved.bounds.y >= 8.0);
    assert!(moved.bounds.x + moved.bounds.w <= 800.0);
    assert!(moved.bounds.y + moved.bounds.h <= 600.0);
}

#[test]
fn hud_collapse_button_toggles_without_starting_drag() {
    let hud = DebugHudState::new();
    let layout = hud.layout(9, 800, 600);
    let center = Point::new(
        layout.collapse_button.x + layout.collapse_button.w / 2.0,
        layout.collapse_button.y + layout.collapse_button.h / 2.0,
    );

    assert!(hud.pointer_down(center));
    assert!(hud.collapsed());
    assert!(!hud.is_dragging());
    assert!(!hud.pointer_move(Point::new(300.0, 300.0)));
    assert!(hud.position().is_none());

    // 面板外按下不消费，同时终结遗留会话；后续抬起不再属于 HUD。
    assert!(!hud.pointer_down(Point::new(700.0, 500.0)));
    assert!(!hud.pointer_up(Point::new(700.0, 500.0)));
}

#[test]
fn hud_ignores_pointer_events_before_first_layout() {
    let hud = DebugHudState::new();
    assert!(!hud.pointer_down(Point::new(10.0, 10.0)));
    assert!(!hud.pointer_move(Point::new(10.0, 10.0)));
    assert!(!hud.pointer_up(Point::new(10.0, 10.0)));
    assert_eq!(hud.bounds(), None);
}
