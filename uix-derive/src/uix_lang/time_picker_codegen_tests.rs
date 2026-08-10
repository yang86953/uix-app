// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 TimePicker 时间绑定与公共属性生成。
#[test]
fn generates_bound_time_picker_contract() {
    // 生成包含受控时间选择器和公共属性的文档。
    let snapshot =
        generate(r#"<TimePicker value={selected_time} width="200px" automationId="time" />"#)
            // 合法属性必须成功映射到公开 TimePicker API。
            .expect("文档属性应映射到公开 TimePicker API");
    // 构造器必须使用公开时间选择类型。
    assert!(snapshot.contains("TimePicker :: new"));
    // 时间状态必须借用给运行时受控入口。
    assert!(snapshot.contains("value (& (selected_time))"));
    // 公共宽度与自动化标识必须继续映射。
    assert!(snapshot.contains("width (200.0)") && snapshot.contains("automation_id (\"time\")"));
}

// 验证 TimePicker 拒绝缺失绑定与字面量。
#[test]
fn rejects_invalid_time_picker_values() {
    // 缺少时间状态时无法兑现双向绑定。
    let missing_value = generate(r#"<TimePicker />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺少 value 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_value.message.contains("value"));
    // 字符串字面量不能提供 State<Time> 句柄。
    let literal_value = generate(r#"<TimePicker value="14:30" />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量 value 必须失败");
    // 诊断必须明确状态类型。
    assert!(literal_value.message.contains("State<Time>"));
}

// 验证 TimePicker 叶组件形状。
#[test]
fn rejects_time_picker_children() {
    // 子节点不能被生成器静默丢弃。
    let child = generate(r#"<TimePicker value={selected_time}><Text>lost</Text></TimePicker>"#)
        // 子树形状必须失败。
        .expect_err("TimePicker 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
