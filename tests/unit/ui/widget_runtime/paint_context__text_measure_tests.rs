// 引入同模块私有算法供直接契约测试。
use super::{
    conservative_width_from_measurement, elide_normalized_single_line_by, elide_single_line_by,
    elide_single_line_cow_by,
};

// 注册字体缺失时的后备宽度测试。
#[test]
// 验证真实度量为零时仍使用稳定估算宽度。
fn conservative_width_uses_estimate_when_font_measurement_is_unavailable() {
    // 模拟字体后端不可用时返回零宽度。
    let width = conservative_width_from_measurement(0.0, "界", 16.0);
    // 后备估算必须保持非零布局宽度。
    assert!(width > 0.0);
    // 结束字体缺失后备测试。
}

// 注册 Unicode 前缀截断测试。
#[test]
// 验证省略算法不会切断多字节 Unicode 字符。
fn elision_preserves_unicode_character_boundaries() {
    // 使用字符数模拟单调且可预测的保守宽度。
    let value = elide_single_line_by("你🙂好", 2.0, |candidate| {
        candidate.chars().count() as f32
    });
    // 两个可用字符宽度应保留首字符和完整省略号。
    assert_eq!(value.as_deref(), Some("你…"));
    // 结束 Unicode 截断测试。
}

// 注册换行与完整文本语义测试。
#[test]
// 验证可容纳文本只规范化换行而不添加省略号。
fn elision_normalizes_newlines_without_truncating_fitting_text() {
    // 为完整规范化字符串提供足够宽度。
    let value = elide_single_line_by("甲\n乙", 3.0, |candidate| {
        candidate.chars().count() as f32
    });
    // 换行必须按既有契约替换为单个空格。
    assert_eq!(value.as_deref(), Some("甲 乙"));
    // 结束换行规范化测试。
}

// 注册普通无换行文本的借用型快路径测试。
#[test]
fn single_line_cow_borrows_fitting_input_and_owns_normalized_input() {
    let fitting = elide_single_line_cow_by("完整文本", 4.0, |candidate| {
        candidate.chars().count() as f32
    });
    assert!(matches!(fitting, Some(std::borrow::Cow::Borrowed(_))));
    let normalized = elide_single_line_cow_by("甲\n乙", 3.0, |candidate| {
        candidate.chars().count() as f32
    });
    assert_eq!(normalized.as_deref(), Some("甲 乙"));
    assert!(matches!(normalized, Some(std::borrow::Cow::Owned(_))));
}

// 注册已规范化窄入口的等价输出测试。
#[test]
fn normalized_elision_preserves_fitting_and_truncated_text() {
    // 已规范化完整文本应保持内容且不附加省略号。
    let fitting = elide_normalized_single_line_by("甲 乙", 3.0, |candidate| {
        candidate.chars().count() as f32
    });
    assert_eq!(fitting.as_deref(), Some("甲 乙"));
    // 完整容纳路径必须借用输入，不为每帧绘制分配 String。
    assert!(matches!(fitting, Some(std::borrow::Cow::Borrowed(_))));
    // 已规范化超宽文本仍必须按 Unicode 标量安全截断。
    let truncated = elide_normalized_single_line_by("你🙂好", 2.0, |candidate| {
        candidate.chars().count() as f32
    });
    assert_eq!(truncated.as_deref(), Some("你…"));
    // 只有真实截断时才需要拥有型结果缓冲。
    assert!(matches!(truncated, Some(std::borrow::Cow::Owned(_))));
}

// 注册极窄宽度门禁测试。
#[test]
// 验证连省略号都无法容纳时返回空值。
fn elision_rejects_width_narrower_than_ellipsis() {
    // 使用小于一个字符的宽度模拟极窄组件。
    let value = elide_single_line_by("text", 0.5, |candidate| candidate.chars().count() as f32);
    // 不得返回会越界绘制的省略号。
    assert_eq!(value, None);
    // 结束极窄宽度测试。
}

// 注册仅能容纳省略号的边界测试。
#[test]
// 验证最小有效宽度返回唯一省略号。
fn elision_returns_ellipsis_at_minimum_valid_width() {
    // 只提供一个字符的显示宽度。
    let value = elide_single_line_by("text", 1.0, |candidate| candidate.chars().count() as f32);
    // 最终结果必须恰好是一个完整省略号。
    assert_eq!(value.as_deref(), Some("…"));
    // 结束省略号边界测试。
}
// 结束共享文本度量测试模块。
