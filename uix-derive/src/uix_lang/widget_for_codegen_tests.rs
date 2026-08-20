// 引入组件感知生成入口与文档解析器。
use super::{generate_document_view, parse_document};

// 验证嵌套 keyed/位置 For 使用完整实际路径建立组件实例。
#[test]
fn nested_for_widget_uses_complete_instance_path() {
    // 解析外层业务 key 与内层位置共同拥有的状态组件。
    let document = parse_document(
        // Leaf 同时使用当前内层项 prop、computed 与私有 state。
        r#"
        <Widget name="Leaf" props="label: String" state="count: 0" computed="summary: label">
          <Text>{summary}: {count}</Text>
        </Widget>
        <Column><For {group} in {groups} key={group.id}><Column><For {item} in {group.items}><Leaf label={item.label} /></For></Column></For></Column>
        "#,
    )
    // 嵌套动态组件文档必须解析成功。
    .expect("嵌套 For Widget 应可解析");
    // 生成完整实例路径与逐迭代准备语句。
    let tokens = generate_document_view(&document)
        // keyed 与位置身份组合必须完成生成。
        .expect("嵌套 For Widget 应生成成功")
        // 规范化令牌便于检查结构。
        .to_string();
    // 输出必须保留两层真实 Rust 循环。
    assert_eq!(tokens.matches("for (__uix_for_ordinal").count(), 2);
    // 内层位置必须与父业务 key 路径组合而不是单独使用序号。
    assert!(tokens.contains("format ! (\"{}|{}\""));
    // 实际 Widget 必须从完整路径派生私有状态作用域。
    assert!(tokens.contains("uix_widget_child_scope"));
    // prop、state 与 computed 都必须落在逐迭代准备区。
    assert!(
        tokens.contains("let __uix_prop_")
            && tokens.contains("uix_widget_state")
            && tokens.contains("let __uix_computed_")
    );
}

// 验证 For 内有状态组件的 Slot 仍在调用方实例作用域求值。
#[test]
fn for_widget_slot_projection_keeps_caller_scope() {
    // 解析有状态 Panel 接收当前行内容投影的组合。
    let document = parse_document(
        // Slot 内 Leaf 的 label 来自调用方 For 项而不是 Panel 私有 state。
        r#"
        <Widget name="Leaf" props="label: String"><Text>{label}</Text></Widget>
        <Widget name="Panel" state="open: true"><Column><Slot /></Column></Widget>
        <Column><For {item} in {items} key={item.id}><Panel><Leaf label={item.label} /></Panel></For></Column>
        "#,
    )
    // 插槽与调用方循环作用域必须解析成功。
    .expect("For Widget Slot 应可解析");
    // 生成调用方投影与被调用方私有状态。
    let tokens = generate_document_view(&document)
        // 两种作用域必须在同一实际迭代中闭合。
        .expect("For Widget Slot 应生成成功")
        // 规范化令牌便于检查边界。
        .to_string();
    // 调用方 item 字段必须进入 Leaf prop 初始化。
    assert!(tokens.contains("(item) . label"));
    // Panel 私有 state 必须使用当前行派生作用域。
    assert!(tokens.contains("uix_widget_child_scope"));
    // Slot 只做编译期投影，不得残留运行时标签或解释入口。
    assert!(!tokens.contains("Slot"));
}
