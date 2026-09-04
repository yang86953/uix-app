// 声明本文件只在启用 test-harness 时编译输入框编辑命令契约测试。
#![cfg(feature = "test-harness")]

//! Input 的选区包裹与插入编辑命令契约：`WrapSelection` 在无选区时于光标处
//! 插入成对标记，有选区时用前缀与后缀包裹选中文本；`InsertText` 保持既有
//! 光标处插入语义。键位修饰键提供 `has_ctrl` 等便捷判断供键盘事件消费。

use uix::prelude::*;
use uix::ui::test_harness::{AutomationAction, TestApp};

fn editor() -> ViewNode {
    // 受控 State 必须由调用方持有：reconcile 重建时不得重置业务值。
    let text = State::new("默认正文".to_owned());
    input()
        .value(&text)
        .placeholder("正文")
        .build()
        .automation_id("doc.body")
}

#[test]
fn wrap_selection_inserts_pair_at_caret_without_selection() {
    let text = State::new("默认正文".to_owned());
    let mut app = TestApp::new((320.0, 200.0), move || {
        input()
            .value(&text)
            .placeholder("正文")
            .build()
            .automation_id("doc.body")
    });

    // 无选区时动作为在光标处（初始为文本末尾）插入成对标记。
    app.perform(
        "doc.body",
        AutomationAction::WrapSelection {
            prefix: "**".to_owned(),
            suffix: "**".to_owned(),
        },
    )
    .expect("无选区包裹动作应成功");
    let value = app
        .snapshot()
        .find("doc.body")
        .expect("输入框应在语义树中")
        .accessibility
        .state
        .value_text
        .clone()
        .expect("输入框应有值文本");
    assert_eq!(value, "默认正文****");
}

#[test]
fn insert_text_keeps_caret_insertion_semantics() {
    let text = State::new("默认正文".to_owned());
    let mut app = TestApp::new((320.0, 200.0), move || {
        input()
            .value(&text)
            .placeholder("正文")
            .build()
            .automation_id("doc.body")
    });

    app.perform(
        "doc.body",
        AutomationAction::InsertText("插入片段".to_owned()),
    )
    .expect("插入动作应成功");
    let value = app
        .snapshot()
        .find("doc.body")
        .expect("输入框应在语义树中")
        .accessibility
        .state
        .value_text
        .clone()
        .expect("输入框应有值文本");
    assert!(value.contains("插入片段"), "插入片段应进入值文本：{value}");
}

#[test]
fn key_mod_provides_documented_modifier_predicates() {
    // 便捷判断必须与公开常量位语义一致。
    assert!(KeyMod::CTRL.has_ctrl());
    assert!(!KeyMod::CTRL.has_shift());
    assert!(KeyMod::SHIFT.has_shift());
    assert!(KeyMod::ALT.has_alt());
    assert!(KeyMod::SUPER.has_super());
    assert!(!KeyMod::NONE.has_ctrl());
    // 组合修饰键包含任一目标位即命中。
    let combo = KeyMod::CTRL | KeyMod::ALT;
    assert!(combo.has_ctrl() && combo.has_alt() && !combo.has_super());
}

