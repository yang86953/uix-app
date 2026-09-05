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

#[test]
fn wrap_after_toolbar_focus_preserves_chinese_selection_and_caret() {
    let text = State::new("前中文🙂后".to_owned());
    let view_text = text.clone();
    let mut app = TestApp::new((400.0, 240.0), move || {
        column_fit((
            input().value(&view_text).build().automation_id("body"),
            button("加粗").build().automation_id("toolbar"),
        ))
    });
    app.focus("body").unwrap();
    app.press_key(KeyCode::Home, KeyMod::NONE).unwrap();
    app.press_key(KeyCode::Right, KeyMod::NONE).unwrap();
    for _ in 0..3 {
        app.press_key(KeyCode::Right, KeyMod::SHIFT).unwrap();
    }
    app.focus("toolbar").unwrap();
    app.perform(
        "body",
        AutomationAction::WrapSelection {
            prefix: "**".to_owned(),
            suffix: "**".to_owned(),
        },
    )
    .unwrap();
    assert_eq!(text.get(), "前**中文🙂**后");
    // 包裹后仍选中内部正文，键入替换不能吞掉标记或后文。
    app.insert_text("body", "替换").unwrap();
    assert_eq!(text.get(), "前**替换**后");
    app.set_value("body", "前后").unwrap();
    app.press_key(KeyCode::Home, KeyMod::NONE).unwrap();
    app.press_key(KeyCode::Right, KeyMod::NONE).unwrap();
    app.perform(
        "body",
        AutomationAction::WrapSelection {
            prefix: "`".to_owned(),
            suffix: "`".to_owned(),
        },
    )
    .unwrap();
    app.insert_text("body", "光标🙂").unwrap();
    assert_eq!(text.get(), "前`光标🙂`后");
}

#[test]
fn lang_key_observer_keeps_input_navigation_and_reports_modifiers() {
    let text = State::new("前后".to_owned());
    let observed = State::new(false);
    let view_text = text.clone();
    let view_observed = observed.clone();
    let mut app = TestApp::new((400.0, 240.0), move || {
        let text = view_text.clone();
        let observed = view_observed.clone();
        uix!(
            r#"<Widget name="Editor" props="text: State<String>, observed: State<bool>"><Column @keyDown="setState(observed: $event.mods.has_ctrl() || $event.mods.has_super())"><Input value={text} automationId="body" /></Column></Widget><Editor text={text.clone()} observed={observed.clone()} />"#
        )
    });
    app.focus("body").unwrap();
    app.press_key(KeyCode::Home, KeyMod::NONE).unwrap();
    app.insert_text("body", "开始").unwrap();
    assert_eq!(text.get(), "开始前后");
    app.press_key(KeyCode::S, KeyMod::CTRL).unwrap();
    assert!(observed.get());
    observed.set(false);
    app.press_key(KeyCode::S, KeyMod::SUPER).unwrap();
    assert!(observed.get());
}
