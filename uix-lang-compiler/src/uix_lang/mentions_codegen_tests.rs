// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Mentions 候选、完整文本绑定、占位文本与公共属性生成。
#[test]
fn generates_bound_mentions_contract() {
    // 生成包含受控提及输入组件的文档。
    let snapshot = generate(
        r#"<Mentions value={message} suggestions={members} placeholder="提及成员" width="240px" automationId="mentions" />"#,
    )
    // 合法属性必须成功映射到公开 Mentions API。
    .expect("文档属性应映射到公开 Mentions API");
    // 构造器必须使用公开 Mentions 类型并接收占位文本。
    assert!(snapshot.contains("Mentions :: new") && snapshot.contains("提及成员"));
    // 提及候选必须映射到运行时 options 构建器。
    let suggestions = snapshot
        // 查找公开候选构建入口。
        .find("options ((members) . clone ())")
        // 失败时输出完整令牌便于定位格式漂移。
        .unwrap_or_else(|| panic!("应生成 Mentions 候选映射：{snapshot}"));
    // 完整文本状态必须借用给运行时绑定入口。
    let value = snapshot
        // 查找公开受控绑定入口。
        .find("bind_value (& (message))")
        // 失败表示状态句柄未保留。
        .expect("应生成 Mentions 完整文本绑定");
    // 运行时契约要求先设置候选再同步绑定值。
    assert!(suggestions < value);
    // 公共宽度与自动化标识必须继续映射。
    assert!(
        snapshot.contains("width (240.0)") && snapshot.contains("automation_id (\"mentions\")")
    );
}

// 验证 Mentions 占位文本表达式映射。
#[test]
fn generates_mentions_placeholder_expression() {
    // 表达式占位文本必须保留调用侧字符串类型检查。
    let snapshot = generate(
        r#"<Mentions value={message} suggestions={members} placeholder={mention_placeholder} />"#,
    )
    // 合法字符串表达式必须成功生成。
    .expect("表达式占位文本应映射到 Mentions 构造器");
    // 构造器参数必须包含原始受限表达式。
    assert!(snapshot.contains("mention_placeholder"));
}

// 验证 Mentions 拒绝缺失属性、字面量与子节点。
#[test]
fn rejects_invalid_mentions_contracts() {
    // 缺少候选时无法调用公开 options 构建器。
    let missing_suggestions = generate(r#"<Mentions value={message} />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺少 suggestions 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_suggestions.message.contains("suggestions"));
    // 缺少状态时无法兑现完整文本双向绑定。
    let missing_value = generate(r#"<Mentions suggestions={members} />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺少 value 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_value.message.contains("value"));
    // 字符串字面量不能提供 Vec<String> 所有权。
    let literal_suggestions = generate(r#"<Mentions value={message} suggestions="one" />"#)
        // 字面量候选必须失败。
        .expect_err("字面量 suggestions 必须失败");
    // 诊断必须说明公开候选类型。
    assert!(literal_suggestions.message.contains("Vec<String>"));
    // 字符串字面量不能提供 State<String> 句柄。
    let literal_value = generate(r#"<Mentions value="hello" suggestions={members} />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量 value 必须失败");
    // 诊断必须明确状态类型。
    assert!(literal_value.message.contains("State<String>"));
    // Mentions 子节点不能被生成器丢弃。
    let child =
        generate(r#"<Mentions value={message} suggestions={members}><Text>lost</Text></Mentions>"#)
            // 子树形状必须失败。
            .expect_err("Mentions 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
