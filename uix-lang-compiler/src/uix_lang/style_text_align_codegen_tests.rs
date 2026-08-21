// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证 textAlign 四种文档值映射到公开 UI 枚举。
#[test]
fn generates_text_align_runtime_mapping() {
    // 覆盖完整文档值与一一对应的运行时变体。
    for (source, variant) in [
        // 显式左对齐。
        ("left", "Left"),
        // 显式右对齐。
        ("right", "Right"),
        // 显式居中。
        ("center", "Center"),
        // 显式两端对齐。
        ("justify", "Justify"),
    ] {
        // 构造只包含目标文本对齐的元素文档。
        let document = parse_document(&format!(
            // 保留关键字源码供编译期映射。
            r#"<Text style="textAlign: {source};">对齐</Text>"#
        ))
        // 语法层必须接受规范样式值。
        .expect("textAlign 语法应合法");
        // 生成真实 Style 字段更新令牌。
        let tokens = generate_view(&document.root)
            // 已映射对齐不得继续返回规划中诊断。
            .expect("textAlign 应映射到运行时对齐")
            // 规范化令牌用于断言公开契约。
            .to_string();
        // 生成结果必须显式更新可选字段。
        assert!(tokens.contains("text_align"));
        // 枚举必须来自 prelude 公开 TextAlign 契约。
        assert!(tokens.contains(&format!("TextAlign :: {variant}")));
    }
}

// 验证未知 textAlign 值产生确定诊断。
#[test]
fn rejects_unknown_text_align_value() {
    // 构造闭合集合之外的未登记关键字。
    let document = parse_document(r#"<Text style="textAlign: start;">对齐</Text>"#)
        // 样式语法层保留原始关键字供映射层诊断。
        .expect("textAlign 原始值应完成语法解析");
    // 代码生成必须返回支持边界诊断。
    let error = generate_view(&document.root).expect_err("未知 textAlign 必须失败");
    // 诊断必须包含文档允许值。
    assert!(error.message.contains("textAlign 只支持"));
}
