// 引入待验证的公开警告提示组件。
use super::{ALERT_VISUAL_REF, Alert};

// 验证 View 构建经过同目录 UIX，并保留调用方显式视觉覆写。
#[test]
fn view_build_uses_uix_visual_and_preserves_authored_values() {
    let alert = Alert::new("提示").show_icon(false).banner(true);
    assert!(std::ptr::eq(alert.visual, ALERT_VISUAL_REF));
    let node = crate::ui::view::View::build(alert);
    let alert = node
        .widget
        .as_any()
        .downcast_ref::<Alert>()
        .expect("UIX 根必须保留 Alert Rust 内核");
    assert!(std::ptr::eq(alert.visual, ALERT_VISUAL_REF));
    assert!(!alert.show_icon);
    assert!(alert.banner);
}

// 验证 UIX 几何表保持关闭区、操作区、图标与正文原有边界。
#[test]
fn uix_layout_preserves_interaction_geometry() {
    let alert = Alert::new("提示")
        .description("说明")
        .action("处理", || {})
        .closable();
    let layout = alert.layout(crate::core::Rect::new(0.0, 0.0, 300.0, 54.0));
    assert_eq!(layout.close, crate::core::Rect::new(264.0, 0.0, 36.0, 54.0));
    assert_eq!(
        layout.action,
        crate::core::Rect::new(196.0, 0.0, 64.0, 54.0)
    );
    assert_eq!(layout.icon, crate::core::Rect::new(8.0, 0.0, 28.0, 54.0));
    assert_eq!(layout.accent, crate::core::Rect::new(2.0, 4.0, 3.0, 46.0));
    assert_eq!(layout.message.x, 36.0);
    assert_eq!(layout.message.w, 152.0);
    assert_eq!(alert.intrinsic_size(), crate::core::Size::new(300.0, 54.0));
}

// 验证声明式刷新不会重新打开用户已经关闭的 Alert。
#[test]
// 声明关闭状态所有权测试。
fn refresh_preserves_runtime_closed_state() {
    // 首次物化一条可关闭警告。
    let mut current = Alert::warning("旧警告").closable();
    // 模拟用户通过运行时关闭入口完成关闭。
    current.close();
    // 后续声明更新消息与类型，但仍只声明关闭能力。
    let next = Alert::error("新错误").closable();
    // 执行真实组件同步路径。
    current.sync_from(next);
    // 用户关闭事实必须继续由运行时状态拥有。
    assert!(!current.is_visible());
}
