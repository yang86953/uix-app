// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证关键字和数值映射到公开 FontWeight 契约。
#[test]
fn generates_font_weight_runtime_mapping() {
    // 覆盖两个关键字、边界与区间内非百位步进值。
    for (source, expected) in [
        // normal 使用公开常规常量。
        ("normal", "FontWeight :: NORMAL"),
        // bold 使用公开粗体常量。
        ("bold", "FontWeight :: BOLD"),
        // 下界通过公开受控构造器生成。
        ("100", "from_numeric"),
        // 区间中任意整数都保留精确值。
        ("550", "from_numeric"),
        // 上界通过公开受控构造器生成。
        ("900", "from_numeric"),
    ] {
        // 构造只包含目标字体粗细的元素文档。
        let document = parse_document(&format!(
            // 保留原始值供编译期映射。
            r#"<Text style="fontWeight: {source};">字重</Text>"#
        ))
        // 语法层必须接受规范样式值。
        .expect("fontWeight 语法应合法");
        // 生成真实 Style 字段更新令牌。
        let tokens = generate_view(&document.root)
            // 已映射字重不得继续返回规划中诊断。
            .expect("fontWeight 应映射到运行时字重")
            // 规范化令牌用于断言公开契约。
            .to_string();
        // 生成结果必须显式更新可选字段。
        assert!(tokens.contains("font_weight"));
        // 生成结果必须引用对应公开契约入口。
        assert!(tokens.contains(expected));
    }
}

// 验证非法关键字、数值与单位产生确定诊断。
#[test]
fn rejects_invalid_font_weight_values() {
    // 覆盖下界、上界、小数、单位与未知关键字。
    for source in ["99", "901", "400.5", "400px", "heavy"] {
        // 构造保留非法值的元素文档。
        let document = parse_document(&format!(
            // 把当前非法值放入统一样式入口。
            r#"<Text style="fontWeight: {source};">字重</Text>"#
        ))
        // 语法层只负责保留原始值。
        .expect("fontWeight 原始值应完成语法解析");
        // 代码生成必须在值映射阶段失败。
        let error = generate_view(&document.root).expect_err("非法 fontWeight 必须失败");
        // 诊断必须明确列出字段支持边界。
        assert!(error.message.contains("fontWeight 只支持"));
    }
}
