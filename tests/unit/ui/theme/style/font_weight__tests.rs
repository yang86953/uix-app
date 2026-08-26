// 引入字体粗细与统一样式契约。
use super::{super::Style, FontWeight};

// 文档闭区间必须完整保留且拒绝越界值。
#[test]
fn font_weight_preserves_supported_numeric_range() {
    // 最小值必须有效。
    assert_eq!(FontWeight::from_numeric(100), Some(FontWeight::THIN));
    // 区间内非百位步进值也必须精确保留。
    assert_eq!(
        FontWeight::from_numeric(550).map(FontWeight::value),
        Some(550)
    );
    // 最大值必须有效。
    assert_eq!(FontWeight::from_numeric(900), Some(FontWeight::BLACK));
    // 下界外数值必须拒绝。
    assert!(FontWeight::from_numeric(99).is_none());
    // 上界外数值必须拒绝。
    assert!(FontWeight::from_numeric(901).is_none());
}

// 显式 normal 必须覆盖继承的 bold。
#[test]
fn font_weight_explicit_normal_overrides_inherited_bold() {
    // 构造粗体基础样式。
    let base = Style::default().with_font_weight(FontWeight::BOLD);
    // 构造显式常规覆盖样式。
    let overlay = Style::default().with_font_weight(FontWeight::NORMAL);
    // 合并后必须保留显式默认值。
    let merged = base.apply(overlay);
    // 有效字重必须变为 normal。
    assert_eq!(merged.effective_font_weight(), FontWeight::NORMAL);
}
