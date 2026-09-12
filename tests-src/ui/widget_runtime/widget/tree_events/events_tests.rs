//! `ui/widget_runtime/widget/tree_events/events.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use crate::ui::adapter::ViewAdapter;
use crate::ui::view::ViewNode;
use crate::ui::AppState;

// 两块无事件消费方的普通容器：任何指针按下都不会被消费。
// 布局几何固定：根 frame 200×120，两行 200×40，行距 8。
fn plain_blocks_root() -> ViewNode {
    crate::uix!(
        r#"
        <Column width="200" height="120" gap="8">
          <Container direction="row" width="200" height="40" />
          <Container direction="row" width="200" height="40" />
        </Column>
        "#
    )
}

fn probe_tree() -> WidgetTree {
    let root = ViewAdapter::capture_root(plain_blocks_root);
    let mut tree = ViewAdapter::build_nodes(root);
    tree.set_app_state(AppState::new());
    if let Some(root_node) = tree.root_mut() {
        root_node.set_frame(crate::core::Rect::new(0.0, 0.0, 200.0, 120.0));
    }
    tree.layout();
    tree
}

// 被拒的合成左键按下不得残留按压手势：后续合成移动必须恢复悬停重定向。
#[test]
fn rejected_agent_pointer_down_does_not_lock_synthetic_hover() {
    let mut tree = probe_tree();
    let center_a = Point::new(100.0, 20.0);
    let center_b = Point::new(100.0, 68.0);
    assert!(
        tree.hit_test(center_a).is_some() && tree.hit_test(center_b).is_some(),
        "探针几何必须落在两块容器上"
    );

    // 悬停先落在 block-a。
    let _ = tree.dispatch_agent_event(&SystemEvent::PointerMove {
        pos: center_a,
        mods: KeyMod::NONE,
    });
    let hovered_a = tree.managers().interaction.hovered_widget();
    assert!(hovered_a.is_some(), "悬停应落在 block-a 命中链上");

    // 按下无人消费：命令必须报告未处理，且不得登记按压。
    let down = tree.dispatch_agent_event(&SystemEvent::PointerDown {
        pos: center_a,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(down, EventResult::NotHandled, "无消费方按下应报告未处理");
    assert!(
        tree.managers().interaction.pressed_widget().is_none(),
        "被拒的合成按下不得残留按压手势"
    );

    // 后续合成移动必须恢复悬停重定向，不再被按压捕获锁死。
    let _ = tree.dispatch_agent_event(&SystemEvent::PointerMove {
        pos: center_b,
        mods: KeyMod::NONE,
    });
    let hovered_b = tree.managers().interaction.hovered_widget();
    assert_ne!(
        hovered_b, hovered_a,
        "被拒按下之后的合成移动应把悬停切到新目标"
    );
}

// 真实指针契约保持不变：Down 建立按压后由 Up 清空（成对派发路径）。
#[test]
fn paired_pointer_down_up_clears_press_without_agent_path() {
    let mut tree = probe_tree();
    let center_a = Point::new(100.0, 20.0);
    let down = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: center_a,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(down, EventResult::NotHandled, "普通容器不消费裸按下");
    assert!(
        tree.managers().interaction.pressed_widget().is_some(),
        "真实路径按下后按压态照常登记（语义 Click 依赖该事实）"
    );
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos: center_a,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(
        tree.managers().interaction.pressed_widget().is_none(),
        "抬起后按压态必须清空"
    );
}
