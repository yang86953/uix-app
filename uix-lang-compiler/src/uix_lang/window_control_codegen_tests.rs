// 引入待验证的文档解析与 View 生成入口。
use super::{generate_view, parse_document};

// 解析单根 UIX 文档并返回稳定令牌文本。
fn generate(source: &str) -> Result<String, super::Diagnostic> {
    // 解析输入文档。
    let document = parse_document(source)?;
    // 生成根 View 令牌。
    let tokens = generate_view(&document.root)?;
    // 使用过程宏令牌的稳定空白格式返回快照。
    Ok(tokens.to_string())
}

// 验证窗控组公开映射与图标前景色定制契约。
#[test]
fn generates_and_validates_window_control_contract() {
    // 未声明图标前景色时必须保持既有默认组合入口。
    let plain = generate(r#"<WindowControl showMinimize showMaximize showClose />"#)
        // 文档化 WindowControl 必须成功生成。
        .expect("WindowControl 文档契约应生成 Rust View");
    // 必须复用公开标准组合构造器。
    assert!(plain.contains("window_controls"));
    // 未指定前景色时不得进入定制入口。
    assert!(!plain.contains("window_controls_with_icon_color"));

    // 声明图标前景色时必须进入定制组合并把颜色转换为 ColorValue。
    let tinted =
        generate(r##"<WindowControl showMinimize showMaximize showClose iconColor="#E7F2ED" />"##)
            // 定制前景色文档必须成功生成。
            .expect("WindowControl iconColor 文档契约应生成 Rust View");
    // 必须复用公开定制组合构造器。
    assert!(tinted.contains("window_controls_with_icon_color"));
    // 颜色字面量必须经 into 转换进入 ColorValue 参数。
    assert!(tinted.contains("#E7F2ED") && tinted.contains(". into ()"));

    // 子内容继续走统一形状拒绝路径。
    let child = generate(r#"<WindowControl><Text>内容</Text></WindowControl>"#)
        // 提取预期不接受子节点诊断。
        .expect_err("WindowControl 子节点必须失败");
    // 诊断必须保留自闭合写法指引。
    assert!(child.message.contains("不接受子节点") && child.suggestion.contains("showMinimize"));

    // 未登记普通属性必须继续走统一拒绝路径。
    let unknown = generate(r#"<WindowControl mystery="value" />"#)
        // 提取预期未知属性诊断。
        .expect_err("WindowControl 未登记属性必须失败");
    // 诊断必须保留具体属性名。
    assert!(unknown.message.contains("mystery"));
}
