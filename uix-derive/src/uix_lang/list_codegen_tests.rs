// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 List 数据、字符串槽位与公共属性的完整生成契约。
#[test]
// 声明完整 List 生成测试。
fn generates_list_contract() {
    // 生成覆盖拥有型集合、动态字符串与公共属性的文本列表。
    let snapshot = generate(
        // 使用全部已登记 List 专有属性。
        r#"<List data={list_items} header={list_header} footer="共 2 项" loadMore={load_more_text} width="320px" automationId="list" />"#,
    )
    // 合法列表必须成功生成。
    .expect("文档属性应映射到公开 List API");
    // 文本集合必须从调用方可迭代表达式取得所有权。
    assert!(snapshot.contains("IntoIterator :: into_iter (list_items)"));
    // 集合元素必须统一收集为运行时拥有的 Vec<String>。
    assert!(snapshot.contains("Vec < :: std :: string :: String >"));
    // 列表必须从公开构造器和 items 构建器开始。
    assert!(snapshot.contains("List :: new () . items"));
    // 动态页首必须进入公开 header 构建器。
    assert!(snapshot.contains("header") && snapshot.contains("list_header"));
    // 静态页尾必须进入公开 footer 构建器。
    assert!(snapshot.contains("footer") && snapshot.contains("共 2 项"));
    // 动态加载更多文字必须进入公开 load_more 构建器。
    assert!(snapshot.contains("load_more") && snapshot.contains("load_more_text"));
    // 物化必须经过公开 View 契约保留空列表替代行为。
    assert!(snapshot.contains("View :: build"));
    // 公共尺寸与自动化标识仍由统一属性层消费。
    assert!(snapshot.contains("width") && snapshot.contains("automation_id"));
}

// 验证 List 最小声明不伪造可选字符串槽位。
#[test]
// 声明 List 默认值测试。
fn generates_list_runtime_defaults() {
    // 生成只包含必需数据表达式的最小列表。
    let snapshot = generate(r#"<List data={list_items} />"#)
        // 最小合法列表必须成功生成。
        .expect("缺省 List 应保留运行时字符串槽位默认值");
    // 文本集合必须进入公开 items 构建器。
    assert!(snapshot.contains("items"));
    // 省略页首时不得生成 header 构建器。
    assert!(!snapshot.contains("header"));
    // 省略页尾时不得生成 footer 构建器。
    assert!(!snapshot.contains("footer"));
    // 省略加载更多文字时不得生成 load_more 构建器。
    assert!(!snapshot.contains("load_more"));
}

// 验证 List 必需集合与叶节点边界。
#[test]
// 声明 List 基础拒绝测试。
fn rejects_missing_literal_and_children() {
    // 缺失 data 时没有列表内容来源。
    let missing = generate(r#"<List />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 data 必须被拒绝");
    // 诊断必须点名 data。
    assert!(missing.message.contains("data"));
    // 单个字符串字面量不能伪装为可迭代文本集合。
    let literal = generate(r#"<List data="单项" />"#)
        // 字面量集合必须失败。
        .expect_err("字面量 List data 必须被拒绝");
    // 诊断必须说明可迭代表达式要求。
    assert!(literal.message.contains("可迭代字符串表达式"));
    // List 运行时绘制文本行，不能接受任意 View 子树。
    let child = generate(r#"<List data={list_items}><Text>额外节点</Text></List>"#)
        // 嵌套元素必须失败。
        .expect_err("List 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}

// 验证 List 未登记属性不能穿过公共映射。
#[test]
// 声明 List 属性拒绝测试。
fn rejects_unknown_list_attribute() {
    // bordered 尚未进入当前 UIX 专有属性契约。
    let unknown = generate(r#"<List data={list_items} bordered />"#)
        // 未登记属性必须失败。
        .expect_err("未知 List 属性必须被拒绝");
    // 诊断必须包含具体属性名。
    assert!(unknown.message.contains("bordered"));
}
