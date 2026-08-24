// 引入 Message 的 UIX 唯一视觉源与公开组件。
use super::{MESSAGE_VISUAL_REF, Message};

// 验证直接构造与 View 构建都共享同一份 UIX 视觉静态项。
#[test]
fn view_build_uses_the_shared_uix_visual() {
    let message = Message::new();
    assert!(std::ptr::eq(message.visual, MESSAGE_VISUAL_REF));
    let node = crate::ui::view::View::build(message);
    let message = node
        .widget
        .as_any()
        .downcast_ref::<Message>()
        .expect("UIX 根必须保留 Message Rust 内核");
    assert!(std::ptr::eq(message.visual, MESSAGE_VISUAL_REF));
}

// 验证 UIX 持有四类提示时长和静态几何，避免 Rust 默认表回流。
#[test]
fn uix_visual_preserves_message_defaults() {
    use crate::platform::capabilities::StatusLevel;

    assert_eq!(
        MESSAGE_VISUAL_REF.defaults.placement,
        crate::ui::Placement::Top
    );
    assert_eq!(MESSAGE_VISUAL_REF.duration_ms(StatusLevel::Success), 3000);
    assert_eq!(MESSAGE_VISUAL_REF.duration_ms(StatusLevel::Info), 3000);
    assert_eq!(MESSAGE_VISUAL_REF.duration_ms(StatusLevel::Warning), 4000);
    assert_eq!(MESSAGE_VISUAL_REF.duration_ms(StatusLevel::Error), 5000);
    assert_eq!(MESSAGE_VISUAL_REF.layout.max_width, 380.0);
    assert_eq!(MESSAGE_VISUAL_REF.layout.item_height, 40.0);
    assert_eq!(MESSAGE_VISUAL_REF.layout.item_stride, 48.0);
}

// 验证普通单行动作文本走借用快路径，不为测量无条件分配 String。
#[test]
fn single_line_action_text_is_borrowed() {
    let normalized = Message::normalized_single_line("重试");
    assert!(matches!(normalized, std::borrow::Cow::Borrowed("重试")));
    assert_eq!(Message::normalized_single_line("重新\n尝试"), "重新 尝试");
}
