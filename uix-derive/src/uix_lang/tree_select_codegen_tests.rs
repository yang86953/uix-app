// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 TreeSelect 树结构、稳定 key 绑定与公共属性生成。
#[test]
fn generates_bound_tree_select_contract() {
    // 生成包含受控树形选择器的文档。
    let snapshot = generate(
        r#"<TreeSelect value={selected_key} options={tree_nodes} width="240px" automationId="tree" />"#,
    )
    // 合法属性必须成功映射公开 TreeSelect API。
    .expect("文档属性应映射到公开 TreeSelect API");
    // 构造器必须先接收 TreeNode 树表达式。
    let nodes = snapshot
        // 查找公开节点构建入口。
        .find("nodes ((tree_nodes) . clone ())")
        // 失败时输出完整令牌便于定位格式漂移。
        .unwrap_or_else(|| panic!("应生成 TreeSelect 节点映射：{snapshot}"));
    // 稳定 key 状态必须借用给运行时绑定入口。
    let value = snapshot
        // 查找公开受控绑定入口。
        .find("bind_value (& (selected_key))")
        // 失败表示状态句柄未保留。
        .expect("应生成 TreeSelect 稳定 key 绑定");
    // 运行时契约要求先设置节点再同步绑定值。
    assert!(nodes < value);
    // 公共宽度与自动化标识必须继续映射。
    assert!(snapshot.contains("width (240.0)") && snapshot.contains("automation_id (\"tree\")"));
}

// 验证 TreeSelect 拒绝缺失属性、字面量与子节点。
#[test]
fn rejects_invalid_tree_select_contracts() {
    // 缺少节点树时无法调用公开 nodes 构建器。
    let missing_options = generate(r#"<TreeSelect value={selected_key} />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺少 options 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_options.message.contains("options"));
    // 缺少状态时无法兑现双向 key 绑定。
    let missing_value = generate(r#"<TreeSelect options={tree_nodes} />"#)
        // 必需属性缺失必须失败。
        .expect_err("缺少 value 必须失败");
    // 诊断必须指出缺失属性。
    assert!(missing_value.message.contains("value"));
    // 字符串字面量不能提供 Vec<TreeNode> 所有权。
    let literal_options = generate(r#"<TreeSelect value={selected_key} options="tree" />"#)
        // 字面量选项必须失败。
        .expect_err("字面量 options 必须失败");
    // 诊断必须说明公开树节点类型。
    assert!(literal_options.message.contains("Vec<TreeNode>"));
    // 字符串字面量不能提供 State<String> 句柄。
    let literal_value = generate(r#"<TreeSelect value="member" options={tree_nodes} />"#)
        // 字面量绑定必须失败。
        .expect_err("字面量 value 必须失败");
    // 诊断必须明确状态类型。
    assert!(literal_value.message.contains("State<String>"));
    // TreeSelect 子节点不能被生成器丢弃。
    let child = generate(
        r#"<TreeSelect value={selected_key} options={tree_nodes}><Text>lost</Text></TreeSelect>"#,
    )
    // 子树形状必须失败。
    .expect_err("TreeSelect 子节点必须失败");
    // 诊断必须说明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}
