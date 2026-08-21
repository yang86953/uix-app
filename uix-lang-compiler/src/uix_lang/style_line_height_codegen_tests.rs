// 引入核心 View 生成与文档解析入口。
use super::{generate_view, parse_document};

// 验证 lineHeight 倍率与像素值映射到公开 UI 构造器。
#[test]
fn generates_line_height_factor_and_pixel_mapping() {
    // 覆盖两种文档登记单位与对应构造器。
    for (source, constructor) in [
        // 无单位值按字号倍率解析。
        ("1.5", "factor"),
        // px 值按固定逻辑像素解析。
        ("24px", "pixels"),
    ] {
        // 构造只包含目标行高的元素文档。
        let document = parse_document(&format!(
            // 保留单位源码供编译期映射。
            r#"<Text style="lineHeight: {source};">行高</Text>"#
        ))
        // 语法层必须接受规范样式值。
        .expect("lineHeight 语法应合法");
        // 生成真实 Style 字段更新令牌。
        let tokens = generate_view(&document.root)
            // 已映射行高不得继续返回规划中诊断。
            .expect("lineHeight 应映射到运行时行高")
            // 规范化令牌用于断言公开契约。
            .to_string();
        // 生成结果必须显式更新可选字段。
        assert!(tokens.contains("line_height"));
        // 数值必须进入对应公开受控构造器。
        assert!(tokens.contains(&format!("LineHeight :: {constructor}")));
    }
}

// 验证非法数值、百分比与未知单位产生确定诊断。
#[test]
fn rejects_invalid_line_height_values() {
    // 覆盖全部文档明确拒绝的值类别。
    for source in [
        // 零值不能形成行盒。
        "0",      // 负倍率不能形成行盒。
        "-1",     // 百分比未登记。
        "120%",   // normal 只作为未声明默认值。
        "normal", // pt 未登记为样式长度单位。
        "24pt",   // NaN 不是有限几何。
        "NaN",    // 无穷值不是有限几何。
        "inf",
    ] {
        // 构造保留非法值的元素文档。
        let document = parse_document(&format!(
            // 把当前非法值放入统一样式入口。
            r#"<Text style="lineHeight: {source};">行高</Text>"#
        ))
        // 语法层只负责保留原始值。
        .expect("lineHeight 原始值应完成语法解析");
        // 代码生成必须在值映射阶段失败。
        let error = generate_view(&document.root).expect_err("非法 lineHeight 必须失败");
        // 诊断必须明确指向 lineHeight。
        assert!(error.message.contains("lineHeight"));
    }
}
