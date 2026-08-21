// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证 float 所有规范值都返回架构决策与 Flex 替代建议。
#[test]
fn style_float_rejects_css_values_with_flex_guidance() {
    // 覆盖完整 float 关键字集合。
    for source in ["left", "right", "none"] {
        // 构造单一 float 属性文档。
        let document = parse_document(&format!(
            // 保留规范 CSS 值供映射层识别。
            r#"<Text style="float: {source};">浮动</Text>"#
        ))
        // 样式语法层必须保留原始值。
        .expect("float 原始值应完成语法解析");
        // 代码生成必须拒绝不存在的浮动格式上下文。
        let error = generate_view(&document.root).expect_err("float 必须返回专用诊断");
        // 诊断必须说明不是目标设计而非仍在规划。
        assert!(error.message.contains("明确不属于目标设计"));
        // 修复建议必须指向原生 Flex 能力。
        assert!(error.suggestion.contains("flexDirection"));
    }
}

// 验证 clear 所有规范值都返回重分组替代建议。
#[test]
fn style_float_rejects_clear_values_with_grouping_guidance() {
    // 覆盖完整 clear 关键字集合。
    for source in ["left", "right", "both", "none"] {
        // 构造单一 clear 属性文档。
        let document = parse_document(&format!(
            // 保留规范 CSS 值供映射层识别。
            r#"<Text style="clear: {source};">清除</Text>"#
        ))
        // 样式语法层必须保留原始值。
        .expect("clear 原始值应完成语法解析");
        // 代码生成必须拒绝不存在的浮动兄弟。
        let error = generate_view(&document.root).expect_err("clear 必须返回专用诊断");
        // 诊断必须说明不是目标设计而非等待实现。
        assert!(error.message.contains("明确不属于目标设计"));
        // 修复建议必须指向 Row/Column 或 Grid 重分组。
        assert!(error.suggestion.contains("Column/Row"));
    }
}

// 验证非法关键词同时得到 CSS 值边界和 UIX 架构说明。
#[test]
fn style_float_rejects_unknown_values_without_planned_message() {
    // 覆盖两个属性各自的非法代表值。
    for (name, source, expected) in [
        // float 不支持 center。
        ("float", "center", "left、right 或 none"),
        // clear 不支持 all。
        ("clear", "all", "left、right、both 或 none"),
    ] {
        // 构造非法属性值文档。
        let document = parse_document(&format!(
            // 保留属性名和值供专用诊断分派。
            r#"<Text style="{name}: {source};">布局</Text>"#
        ))
        // 样式语法层不负责支持矩阵。
        .expect("float/clear 原始值应完成语法解析");
        // 映射层必须返回专用属性诊断。
        let error = generate_view(&document.root).expect_err("非法浮动值必须失败");
        // 诊断必须列出该属性的 CSS 闭合集合。
        assert!(error.message.contains(expected));
        // 已决策非目标不得继续声称规划中。
        assert!(!error.message.contains("规划中"));
    }
}
