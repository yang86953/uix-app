// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 List 数据、字符串槽位、外观配置与公共属性的完整生成契约。
#[test]
// 声明完整 List 生成测试。
fn generates_list_contract() {
    // 生成覆盖拥有型集合、动态字符串、动态边框、静态尺寸与公共属性的文本列表。
    let snapshot = generate(
        // 使用全部已登记 List 专有属性。
        r#"<List data={list_items} header={list_header} footer="共 2 项" loadMore={load_more_text} bordered={show_border} size="large" width="320px" automationId="list" />"#,
    )
    // 合法列表必须成功生成。
    .expect("文档属性应映射到公开 List API");
    // 文本集合必须从调用方可迭代表达式取得所有权。
    assert!(snapshot.contains("IntoIterator :: into_iter ((list_items) . clone ())"));
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
    // 动态边框状态必须进入公开 bordered 构建器。
    assert!(snapshot.contains("bordered (show_border)"));
    // 静态大尺寸必须映射到公开 ControlSize 枚举。
    assert!(snapshot.contains("ControlSize :: Large"));
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
    // 省略边框时必须保留运行时默认值。
    assert!(!snapshot.contains("bordered"));
    // 省略尺寸时必须保留 ConfigProvider 运行时默认值。
    assert!(!snapshot.contains("ControlSize"));
}

// 验证 List 布尔形状与三档尺寸关键字的完整映射。
#[test]
// 声明 List 外观值映射测试。
fn maps_list_appearance_values() {
    // 布尔简写必须生成 true。
    let shorthand = generate(r#"<List data={list_items} bordered />"#)
        // 合法简写必须成功生成。
        .expect("List bordered 简写应生成");
    // 简写必须进入公开构建器。
    assert!(shorthand.contains("bordered (true)"));
    // 显式 false 字面量必须原样生成。
    let literal = generate(r#"<List data={list_items} bordered="false" />"#)
        // 合法字面量必须成功生成。
        .expect("List bordered=false 应生成");
    // 字面量必须进入公开构建器。
    assert!(literal.contains("bordered (false)"));
    // 三档尺寸及其公开枚举变体必须一一对应。
    for (value, variant) in [("small", "Small"), ("middle", "Medium"), ("large", "Large")] {
        // 构造当前静态尺寸声明。
        let source = format!(r#"<List data={{list_items}} size="{value}" />"#);
        // 每个登记尺寸必须成功生成。
        let snapshot = generate(&source).expect("已登记 List size 应生成");
        // 生成物必须包含对应公开 ControlSize 变体。
        assert!(snapshot.contains(&format!("ControlSize :: {variant}")));
    }
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

// 验证 List 非法外观值与未登记属性不能穿过公共映射。
#[test]
// 声明 List 属性拒绝测试。
fn rejects_invalid_list_appearance_and_unknown_attribute() {
    // 非布尔边框字面量不能进入公开 bool 构建器。
    let bordered = generate(r#"<List data={list_items} bordered="yes" />"#)
        // 非法布尔属性必须失败。
        .expect_err("非法 List bordered 必须被拒绝");
    // 诊断必须点名布尔值要求。
    assert!(bordered.message.contains("布尔值"));
    // 动态尺寸无法在生成期验证关键字集合。
    let dynamic_size = generate(r#"<List data={list_items} size={list_size} />"#)
        // 动态尺寸必须失败。
        .expect_err("动态 List size 必须被拒绝");
    // 诊断必须要求字符串字面量。
    assert!(dynamic_size.message.contains("字符串字面量"));
    // 未登记尺寸不能静默回退到运行时默认值。
    let unknown_size = generate(r#"<List data={list_items} size="huge" />"#)
        // 未知尺寸必须失败。
        .expect_err("未知 List size 必须被拒绝");
    // 诊断必须给出完整合法尺寸集合。
    assert!(
        unknown_size.message.contains("不受支持") && unknown_size.suggestion.contains("middle")
    );
    // 任意未登记属性仍须进入公共拒绝路径。
    let unknown = generate(r#"<List data={list_items} striped />"#)
        // 未登记属性必须失败。
        .expect_err("未知 List 属性必须被拒绝");
    // 诊断必须包含具体属性名。
    assert!(unknown.message.contains("striped"));
}
