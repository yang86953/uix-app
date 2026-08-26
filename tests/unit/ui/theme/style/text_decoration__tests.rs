// 引入文本装饰和统一样式契约。
use super::{super::Style, TextDecoration};

// 显式 none 必须能够覆盖继承的下划线。
#[test]
fn text_decoration_explicit_none_overrides_inherited_underline() {
    // 构造带下划线的基础样式。
    let base = Style::default().with_text_decoration(TextDecoration::Underline);
    // 构造显式关闭文本装饰的覆盖样式。
    let overlay = Style::default().with_text_decoration(TextDecoration::None);
    // 合并后必须保留显式默认值。
    let merged = base.apply(overlay);
    // 有效装饰必须变为 none。
    assert_eq!(merged.effective_text_decoration(), TextDecoration::None);
}
