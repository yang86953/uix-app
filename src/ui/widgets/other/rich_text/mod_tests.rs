use super::*;
use crate::core::Point;
use crate::ui::component::traits::EventHandler;
use crate::ui::event::SystemEvent;
use crate::ui::{KeyMod, MouseButton};

fn dummy_event() -> SystemEvent {
    SystemEvent::PointerUp {
        pos: Point::new(0.0, 0.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

#[test]
fn on_link_callback_fires_when_submit_emitted() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let hook = calls.clone();
    let rich = RichText::new().on_link(move |url| hook.borrow_mut().push(url.to_string()));
    rich.pending_submit
        .replace(Some("https://example.com".to_string()));

    let event = rich.semantic_event(ComponentId::default(), &dummy_event());
    assert!(event.is_some(), "应发出 Submit 语义事件");
    assert_eq!(*calls.borrow(), vec!["https://example.com".to_string()]);
}

#[test]
fn on_link_not_called_without_pending_submit() {
    let calls = Rc::new(RefCell::new(0usize));
    let hook = calls.clone();
    let rich = RichText::new().on_link(move |_| *hook.borrow_mut() += 1);

    let event = rich.semantic_event(ComponentId::default(), &dummy_event());
    assert!(event.is_none());
    assert_eq!(*calls.borrow(), 0, "无待提交链接时不应触发回调");
}

#[test]
fn on_link_and_submit_semantic_event_coexist() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let hook = calls.clone();
    let rich = RichText::new().on_link(move |url| hook.borrow_mut().push(url.to_string()));
    rich.pending_submit
        .replace(Some("https://uix.dev/route".to_string()));

    let Some(event) = rich.semantic_event(ComponentId::default(), &dummy_event()) else {
        panic!("Submit 语义事件保留");
    };
    assert_eq!(
        event.kind,
        crate::ui::SemanticKind::Submit,
        "与 SemanticKind::Submit 共存"
    );
    assert!(
        matches!(&event.payload, crate::ui::SemanticPayload::Text(url) if url == "https://uix.dev/route")
    );
    assert_eq!(*calls.borrow(), vec!["https://uix.dev/route".to_string()]);
}

// 验证跨样式段的组合序列仍形成单一可选择字素簇。
#[test]
fn selection_expands_across_style_segments_to_complete_grapheme() {
    // 基字符和组合音标故意拆到两个不同样式段。
    let rich = RichText::new().content(vec![
        // 第一段保存基础拉丁字母。
        RichTextSegment::Text {
            // 使用基础字符正文。
            content: "a".to_owned(),
            // 使用默认样式。
            style: RichTextStyle::default(),
        },
        // 第二段保存组合音标并改变样式。
        RichTextSegment::Text {
            // 使用组合锐音正文。
            content: "\u{0301}".to_owned(),
            // 使用粗体样式验证跨段拼接。
            style: RichTextStyle {
                // 启用粗体以形成不同样式 run。
                bold: true,
                // 其余样式沿用默认值。
                ..RichTextStyle::default()
            },
        },
        // 第三段提供相邻普通字符。
        RichTextSegment::Text {
            // 使用尾随拉丁字符。
            content: "z".to_owned(),
            // 使用默认样式。
            style: RichTextStyle::default(),
        },
    ]);
    // 模拟旧调用方从组合序列内部选择到其终点。
    rich.set_selection_range(1, 2);
    // 选择必须向前扩展并包含基础字符。
    assert_eq!(rich.selection.get(), Some((0, 2)));
    // 提取文本必须保留完整组合序列。
    assert_eq!(rich.selected_text().as_deref(), Some("a\u{0301}"));
    // 同一点空选择不得意外选中整个字素簇。
    rich.set_selection_range(1, 1);
    // 空选择应被清除。
    assert_eq!(rich.selection.get(), None);
}
