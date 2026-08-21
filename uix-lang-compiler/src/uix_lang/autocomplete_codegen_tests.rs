// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 AutoComplete 候选、文本绑定、占位文本与公共属性生成。
#[test]
fn generates_bound_autocomplete_contract() {
    // 生成包含受控自动完成组件的文档。
    let snapshot = generate(
        r#"<AutoComplete value={query} options={suggestions} placeholder="请输入" width="240px" automationId="search" />"#,
    )
    // 合法属性必须成功映射公开 AutoComplete API。
    .expect("文档属性应映射到公开 AutoComplete API");
    // 构造器必须先接收候选表达式。
    let options = snapshot
        // 查找公开候选构建入口。
        .find("options ((suggestions) . clone ())")
        // 失败时输出完整令牌便于定位格式漂移。
        .unwrap_or_else(|| panic!("应生成 AutoComplete 候选映射：{snapshot}"));
    // 输入状态必须借用给运行时绑定入口。
    let value = snapshot
        // 查找公开受控绑定入口。
        .find("bind_value (& (query))")
        // 失败表示状态句柄未保留。
        .expect("应生成 AutoComplete 文本绑定");
    // 运行时契约要求先设置候选再同步绑定值。
    assert!(options < value);
    // 占位文本必须调用公开构建器并保留文字。
    assert!(snapshot.contains("placeholder") && snapshot.contains("请输入"));
    // 公共宽度与自动化标识必须继续映射。
    assert!(snapshot.contains("width (240.0)") && snapshot.contains("automation_id (\"search\")"));
}

// 验证 AutoComplete 拒绝缺失属性、字面量与子节点。
#[test]
fn rejects_invalid_autocomplete_contracts() {
    // 缺少候选时无法调用公开 options 构建器。
    let missing_options = generate(r#"<AutoComplete value={query} />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺少 options 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_options.message.contains("options"));
    // 缺少状态时无法兑现双向文本绑定。
    let missing_value = generate(r#"<AutoComplete options={suggestions} />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺少 value 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_value.message.contains("value"));
    // 字符串字面量不能提供 Vec<String> 所有权。
    let literal_options = generate(r#"<AutoComplete value={query} options="one" />"#)
        // 字面量候选必须失败。
        .expect_err("字面量 options 必须失败");
    // 诊断必须说明公开候选类型。
    assert!(literal_options.message.contains("Vec<String>"));
    // 字符串字面量不能提供 State<String> 句柄。
    let literal_value = generate(r#"<AutoComplete value="ap" options={suggestions} />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量 value 必须失败");
    // 诊断必须明确状态类型。
    assert!(literal_value.message.contains("State<String>"));
    // AutoComplete 子节点不能被生成器丢弃。
    let child = generate(
        r#"<AutoComplete value={query} options={suggestions}><Text>lost</Text></AutoComplete>"#,
    )
    // 子树形状必须失败。
    .expect_err("AutoComplete 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
