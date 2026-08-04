//! 表单正则 pattern capability 的行为契约。

// 仅在默认集合或显式 feature 启用时编译本测试。
#![cfg(feature = "form-pattern")]

// 导入表单公开入口。
use uix::ui::Form;

// 验证正则规则接受匹配值并拒绝不匹配值。
#[test]
fn pattern_rule_validates_form_text() {
    // 构造一个符合规则的字段模型。
    let accepted = Form::new()
        .field("code", "编码")
        .initial("UIX-42")
        .validate_pattern(r"^UIX-[0-9]+$", "编码格式错误")
        .build();
    // 匹配值必须通过整表校验。
    assert!(accepted.validate().is_ok());

    // 构造一个不符合规则的字段模型。
    let rejected = Form::new()
        .field("code", "编码")
        .initial("invalid")
        .validate_pattern(r"^UIX-[0-9]+$", "编码格式错误")
        .build();
    // 读取不匹配值产生的首个字段错误。
    let errors = rejected.validate().expect_err("不匹配值必须校验失败");
    // 错误文案必须保持调用方声明值。
    assert_eq!(errors[0].message(), "编码格式错误");
}
