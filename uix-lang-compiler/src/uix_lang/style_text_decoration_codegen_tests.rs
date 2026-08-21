// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证 textDecoration 四种文档值映射到公开 UI 枚举。
#[test]
fn generates_text_decoration_runtime_mapping() {
    // 覆盖完整文档值与一一对应的运行时变体。
    for (source, variant) in [
        // 显式关闭文本装饰。
        ("none", "None"),
        // 绘制下划线。
        ("underline", "Underline"),
        // 绘制上划线。
        ("overline", "Overline"),
        // 绘制删除线。
        ("line-through", "LineThrough"),
    ] {
        // 构造只包含目标文本装饰的元素文档。
        let document = parse_document(&format!(
            // 保留关键字源码供编译期映射。
            r#"<Text style="textDecoration: {source};">装饰</Text>"#
        ))
        // 语法层必须接受规范样式值。
        .expect("textDecoration 语法应合法");
        // 生成真实 Style 字段更新令牌。
        let tokens = generate_view(&document.root)
            // 已映射装饰不得继续返回规划中诊断。
            .expect("textDecoration 应映射到运行时装饰")
            // 规范化令牌用于断言公开契约。
            .to_string();
        // 生成结果必须显式更新可选字段。
        assert!(tokens.contains("text_decoration"));
        // 枚举必须来自 prelude 公开 TextDecoration 契约。
        assert!(tokens.contains(&format!("TextDecoration :: {variant}")));
    }
}

// 验证未知 textDecoration 值产生确定诊断。
#[test]
fn rejects_unknown_text_decoration_value() {
    // 构造 CSS 之外的未登记关键字。
    let document = parse_document(r#"<Text style="textDecoration: blink;">装饰</Text>"#)
        // 样式语法层保留原始关键字供映射层诊断。
        .expect("textDecoration 原始值应完成语法解析");
    // 代码生成必须返回支持边界诊断。
    let error = generate_view(&document.root).expect_err("未知 textDecoration 必须失败");
    // 诊断必须包含文档允许值。
    assert!(error.message.contains("textDecoration 只支持"));
}
