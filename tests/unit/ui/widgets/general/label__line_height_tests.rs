// 引入待验证的 Label 私有测量入口。
use super::{LABEL_VISUAL_REF, Label};
// 引入公开行高与样式构造器。
use crate::ui::theme::style::{LineHeight, Style};

// 验证固定像素行高成为单行 Label 的固有高度。
#[test]
fn line_height_changes_label_intrinsic_height() {
    // 创建二十四像素显式行高样式。
    let style = Style::default().with_line_height(
        // 正像素值必须构造成功。
        LineHeight::pixels(24.0).expect("正像素行高必须有效"),
    );
    // 把统一样式应用到真实 Label 组件。
    let label = Label::new("line height").style(style);
    // 固有高度必须使用显式行盒而非默认视觉字高。
    assert_eq!(label.intrinsic_size().h, 24.0);
}

// 验证直接构造与 View 构建共享同目录 UIX 的唯一静态视觉地址。
#[test]
fn view_build_uses_uix_visual_without_a_rust_default_copy() {
    let label = Label::new("UIX Label");
    assert!(std::ptr::eq(label.visual, LABEL_VISUAL_REF));
    let node = crate::ui::view::View::build(label);
    let label = node
        .widget
        .as_any()
        .downcast_ref::<Label>()
        .expect("UIX 根必须保留 Label Rust 内核");
    assert!(std::ptr::eq(label.visual, LABEL_VISUAL_REF));
}
