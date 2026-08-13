// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Tree 数据、交互配置与公共属性的完整生成契约。
#[test]
// 声明完整 Tree 生成测试。
fn generates_tree_contract() {
    // 生成覆盖类型化数据、静态勾选、动态展开与公共属性的树。
    let snapshot = generate(r#"<Tree data={tree_nodes} checkable="true" defaultExpandAll={expand_tree} width="320px" automationId="navigation-tree" />"#)
        // 合法树必须成功生成。
        .expect("文档属性应映射到公开 Tree API");
    // 数据表达式必须通过拥有所有权的 IntoIterator 统一收集。
    assert!(snapshot.contains("IntoIterator :: into_iter ((tree_nodes) . clone ())"));
    // 收集目标必须锁定公开 TreeNode 类型。
    assert!(snapshot.contains("TreeNode"));
    // 运行时构造器必须接收类型化节点集合。
    assert!(snapshot.contains("Tree :: new"));
    // 静态勾选能力必须进入公开构建器。
    assert!(snapshot.contains("checkable (true)"));
    // 动态默认展开值必须保留给 Rust 核对 bool 类型。
    assert!(snapshot.contains("default_expand_all (expand_tree)"));
    // 组件必须物化为公开叶节点。
    assert!(snapshot.contains("ViewNode :: leaf"));
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (320.0)") && snapshot.contains("automation_id"));
}

// 验证 Tree 两个布尔属性的显式默认值。
#[test]
// 声明 Tree 默认值测试。
fn generates_tree_boolean_defaults() {
    // 生成只包含必需数据的最小树。
    let snapshot = generate(r#"<Tree data={tree_nodes} />"#)
        // 最小属性集合必须成功生成。
        .expect("缺省布尔属性应使用文档默认值");
    // checkable 缺省值必须显式固定为 false。
    assert!(snapshot.contains("checkable (false)"));
    // defaultExpandAll 缺省值必须显式固定为 false。
    assert!(snapshot.contains("default_expand_all (false)"));
}

// 验证 Tree 必需数据与数据表达式形状诊断。
#[test]
// 声明 Tree 数据错误测试。
fn rejects_invalid_tree_data() {
    // 缺失 data 时没有运行时节点来源。
    let missing = generate(r#"<Tree />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 data 必须被拒绝");
    // 诊断必须点名 data。
    assert!(missing.message.contains("data"));
    // 字符串不能伪装成类型化集合。
    let literal = generate(r#"<Tree data="nodes" />"#)
        // 非表达式 data 必须失败。
        .expect_err("字符串 data 必须被拒绝");
    // 诊断必须说明 TreeNode 集合要求。
    assert!(literal.message.contains("TreeNode"));
    // 修复建议必须给出花括号数据引用。
    assert!(literal.suggestion.contains("data={tree_nodes}"));
}

// 验证 Tree 叶节点、布尔值与专有属性边界。
#[test]
// 声明 Tree 形状错误测试。
fn rejects_invalid_tree_shape_and_attributes() {
    // Tree 自身绘制层级，不能接受 View 子树。
    let child = generate(r#"<Tree data={nodes}><Text>非法</Text></Tree>"#)
        // 嵌套元素必须失败。
        .expect_err("Tree 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // 静态 checkable 只接受布尔字面量。
    let checkable = generate(r#"<Tree data={nodes} checkable="yes" />"#)
        // 非布尔 checkable 必须失败。
        .expect_err("非法 checkable 必须被拒绝");
    // 诊断必须说明布尔值要求。
    assert!(checkable.message.contains("布尔"));
    // 静态 defaultExpandAll 只接受布尔字面量。
    let expand = generate(r#"<Tree data={nodes} defaultExpandAll="all" />"#)
        // 非布尔默认展开值必须失败。
        .expect_err("非法 defaultExpandAll 必须被拒绝");
    // 诊断必须说明布尔值要求。
    assert!(expand.message.contains("布尔"));
    // 未登记属性不能被公共映射静默忽略。
    let unknown = generate(r#"<Tree data={nodes} expandAll="true" />"#)
        // 拼写错误的属性必须失败。
        .expect_err("未知 Tree 属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(unknown.message.contains("expandAll"));
}
