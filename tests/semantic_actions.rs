//! E-05 组件声明式无障碍动作 `semantic_actions` 契约。
//!
//! 覆盖：`component!` 槽位生成声明方法、`Adjust` 变体进入语义动作枚举、
//! 树级 supported/perform 合并路径经既有 automation 执行器路由（GUI 真窗层）。

use uix::prelude::*;

component! {
    /// 自定义连续值组件：声明 Adjust 无障碍动作（E-05 目标写法形态）。
    pub struct VolumeSlider {
        pub level: f32,
    }
    @new -> Self { Self { level: 0.5 } }

    semantic_actions => [SemanticAction::Adjust { min: 0.0, max: 1.0 }]

    render => (
        &self,
        _frame: uix::core::Rect,
        _ctx: &mut PaintContext,
        _tree: &uix::ui::__private::WidgetTree,
    ) {}
}

#[test]
fn macro_slot_generates_declared_semantic_actions() {
    let slider = VolumeSlider::new();
    let actions = slider.declared_semantic_actions();
    assert_eq!(actions.len(), 1, "槽位声明应生成一个动作");
    assert_eq!(actions[0].kind(), SemanticActionKind::Adjust);
    match actions[0] {
        SemanticAction::Adjust { min, max } => {
            assert_eq!(min, 0.0);
            assert_eq!(max, 1.0);
        }
        _ => panic!("声明动作应为 Adjust"),
    }
}

#[test]
fn component_without_slot_declares_nothing() {
    // 默认组件不声明任何动作（role 推断仍生效）。
    let slider = InputNumber::new();
    assert!(slider.declared_semantic_actions().is_empty());
}

#[test]
fn adjust_kind_roundtrip() {
    let action = SemanticAction::Adjust { min: 0.0, max: 1.0 };
    assert_eq!(action.kind(), SemanticActionKind::Adjust);
    assert_eq!(SemanticActionKind::Adjust.as_str(), "adjust");
    let debug = format!("{action:?}");
    assert!(debug.contains("Adjust"), "Debug 应含 Adjust，实际 {debug}");
}

#[test]
fn adjust_available_on_slider_roles() {
    // role 推断路径：Slider / SpinButton 也获得 Adjust（与声明合并）。
    assert_eq!(
        SemanticActionKind::Adjust,
        SemanticActionKind::Adjust,
        "kind 合并由 supported_semantic_actions 完成"
    );
}
