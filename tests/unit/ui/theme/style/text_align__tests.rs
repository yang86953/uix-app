// 引入文本对齐和统一样式契约。
use super::{super::Style, TextAlign};

// 显式 left 必须能够覆盖继承的 right。
#[test]
fn text_align_explicit_left_overrides_inherited_right() {
    // 构造右对齐基础样式。
    let base = Style::default().with_text_align(TextAlign::Right);
    // 构造显式左对齐覆盖样式。
    let overlay = Style::default().with_text_align(TextAlign::Left);
    // 合并后必须保留显式默认值。
    let merged = base.apply(overlay);
    // 有效对齐必须变为 left。
    assert_eq!(merged.effective_text_align(), TextAlign::Left);
}
