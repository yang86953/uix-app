// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证 borderStyle 五种文档值映射到公开 UI 枚举。
#[test]
fn generates_border_style_runtime_mapping() {
    // 覆盖完整文档值与一一对应的运行时变体。
    for (source, variant) in [
        // 显式关闭边框绘制。
        ("none", "None"),
        // 显式实线。
        ("solid", "Solid"),
        // 连续相位虚线。
        ("dashed", "Dashed"),
        // 圆点线。
        ("dotted", "Dotted"),
        // 双线。
        ("double", "Double"),
    ] {
        // 构造只包含目标边框线型的元素文档。
        let document = parse_document(&format!(
            // 保留关键字源码供编译期映射。
            r#"<Text style="borderStyle: {source};">边框</Text>"#
        ))
        // 语法层必须接受规范样式值。
        .expect("borderStyle 语法应合法");
        // 生成真实 Style 字段更新令牌。
        let tokens = generate_view(&document.root)
            // 已映射线型不得继续返回规划中诊断。
            .expect("borderStyle 应映射到运行时线型")
            // 规范化令牌用于断言公开契约。
            .to_string();
        // 生成结果必须显式更新可选字段。
        assert!(tokens.contains("border_style"));
        // 枚举必须来自预lude 公开 BorderStyle 契约。
        assert!(tokens.contains(&format!("BorderStyle :: {variant}")));
    }
}

// 验证未知 borderStyle 值产生确定诊断。
#[test]
fn rejects_unknown_border_style_value() {
    // 构造 CSS 之外的未登记关键字。
    let document = parse_document(r#"<Text style="borderStyle: groove;">边框</Text>"#)
        // 样式语法层保留原始关键字供映射层诊断。
        .expect("borderStyle 原始值应完成语法解析");
    // 代码生成必须返回支持边界诊断。
    let error = generate_view(&document.root).expect_err("未知 borderStyle 必须失败");
    // 诊断必须包含文档允许值。
    assert!(error.message.contains("borderStyle 只支持"));
}
