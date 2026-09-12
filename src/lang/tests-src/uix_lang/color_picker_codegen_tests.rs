// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 ColorPicker 颜色绑定与公共属性生成。
#[test]
fn generates_bound_color_picker_contract() {
    // 生成包含受控颜色选择器和公共属性的文档。
    let snapshot =
        generate(r#"<ColorPicker value={selected_color} width="200px" automationId="color" />"#)
            // 合法属性必须成功映射到公开 ColorPicker API。
            .expect("文档属性应映射到公开 ColorPicker API");
    // 构造器必须使用公开颜色选择类型。
    assert!(snapshot.contains("ColorPicker :: new"));
    // 颜色状态必须借用给运行时受控入口。
    assert!(snapshot.contains("value (& (selected_color))"));
    // 公共宽度与自动化标识必须继续映射。
    assert!(snapshot.contains("width (200.0)") && snapshot.contains("automation_id (\"color\")"));
}

// 验证 ColorPicker 拒绝缺失绑定与字面量。
#[test]
fn rejects_invalid_color_picker_values() {
    // 缺少颜色状态时无法兑现双向绑定。
    let missing_value = generate(r#"<ColorPicker />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺少 value 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_value.message.contains("value"));
    // 字符串字面量不能提供 State<Color> 句柄。
    let literal_value = generate(r##"<ColorPicker value="#1677ff" />"##)
        // 字面量绑定必须失败。
        .expect_err("字面量 value 必须失败");
    // 诊断必须明确状态类型。
    assert!(literal_value.message.contains("State<Color>"));
}

// 验证 ColorPicker 叶组件形状。
#[test]
fn rejects_color_picker_children() {
    // 子节点不能被生成器静默丢弃。
    let child = generate(r#"<ColorPicker value={selected_color}><Text>lost</Text></ColorPicker>"#)
        // 子树形状必须失败。
        .expect_err("ColorPicker 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
