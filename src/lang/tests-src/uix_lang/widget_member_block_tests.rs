// 集中验证 Widget 成员声明块的解析、双写拒绝与等价展开。

use crate::lang::compiler::uix_lang::{Declaration, Node, WidgetMemberKind, parse_document};

// 解析文档并返回唯一 Widget 声明（已剥离空白文本与跨度细节）。
fn widget_of(source: &str) -> String {
    // 取原始声明后统一规范化。
    let mut widget = raw_widget_of(source);
    // 移除纯格式化空白文本。
    clean_whitespace(&mut widget.children);
    // 输出剥除跨度的调试串用于等价比较。
    strip_spans(&format!("{widget:?}"))
}

// 解析文档并克隆返回唯一 Widget 声明本体。
fn raw_widget_of(source: &str) -> crate::lang::compiler::uix_lang::WidgetDeclaration {
    // 先成功解析。
    let document = parse_document(source).expect("成员块文档必须可解析");
    // 取出唯一 Widget 声明。
    let Some(Declaration::Widget(widget)) = document
        .declarations
        .iter()
        .find(|declaration| matches!(declaration, Declaration::Widget(_)))
    else {
        panic!("应存在 Widget 声明");
    };
    // 克隆以获得所有权。
    widget.clone()
}

// 递归删除纯格式化空白的文本节点。
fn clean_whitespace(children: &mut Vec<Node>) {
    // 只保留非空白文本与其他节点。
    children.retain(|node| !matches!(node, Node::Text(text) if text.value.trim().is_empty()));
    // 递归处理嵌套元素。
    for node in children.iter_mut() {
        // 元素继续清理子树。
        if let Node::Element(element) = node {
            // 清理子级。
            clean_whitespace(&mut element.children);
        }
    }
}

// 属性形式与块级形式解析出完全一致的组件声明。
#[test]
fn block_form_parses_identically_to_attribute_form() {
    let attribute_form = "<Widget name=\"Counter\" state=\"count: 0\" computed=\"double: count * 2\"><Text>Count: {count}</Text></Widget><Counter />";
    let block_form = "<Widget name=\"Counter\">\n  @state {\n    count: 0,\n  }\n  @computed {\n    double: count * 2,\n  }\n  <Text>Count: {count}</Text>\n</Widget>\n<Counter />";
    // 两种形式的组件声明在剥除跨度与空白文本后必须完全一致。
    assert_eq!(widget_of(attribute_form), widget_of(block_form));
}

// props 与 actions 块同样与属性形式等价，包含 do 多语句主体。
#[test]
fn props_and_action_blocks_match_attribute_semantics() {
    let attribute_form = "<Widget name=\"Pager\" props=\"total: number, onPage: (number)\" actions=\"go: do { if total > 0 { onPage(total); } }\"><Slot /></Widget><Pager total={1} onPage={noop} />";
    let block_form = "<Widget name=\"Pager\">\n  @props {\n    total: number,\n    onPage: (number),\n  }\n  @actions {\n    go: do { if total > 0 { onPage(total); } },\n  }\n  <Slot />\n</Widget>\n<Pager total={1} onPage={noop} />";
    assert_eq!(widget_of(attribute_form), widget_of(block_form));
}

// 同一成员同时使用属性形式与块级形式时给出定向诊断。
#[test]
fn mixing_attribute_and_block_forms_is_rejected() {
    let source = "<Widget name=\"Counter\" state=\"count: 0\">\n  @state {\n    draft: '',\n  }\n  <Text>t</Text>\n</Widget><Counter />";
    let error = parse_document(source).expect_err("同类别双写必须失败");
    assert!(error.message.contains("state"), "诊断应点名冲突成员");
    assert!(error.message.contains("属性形式") && error.message.contains("块级形式"));
}

// 同类成员声明块重复出现时给出定向诊断。
#[test]
fn duplicate_member_blocks_are_rejected() {
    let source = "<Widget name=\"Counter\">\n  @state {\n    count: 0,\n  }\n  @state {\n    other: 1,\n  }\n</Widget><Counter />";
    let error = parse_document(source).expect_err("重复成员块必须失败");
    assert!(error.message.contains("重复"));
}

// 普通元素中的 @props 形状保持普通文本语义，不受成员块拦截影响。
#[test]
fn non_widget_elements_keep_at_sign_text_semantics() {
    let document =
        parse_document("<App><Text>@props 占位</Text></App>").expect("文本内 @ 不触发成员块解析");
    let Node::Element(text) = &document.root.children[0] else {
        panic!("根子节点应为元素");
    };
    // 根内容保持文本节点并保留 @props 字样。
    assert!(matches!(&text.children[0], Node::Text(node) if node.value.contains("@props")));
}

// 非法成员名（无法映射 Rust 标识符的字段）在成员解析层给出既有字段诊断。
#[test]
fn invalid_field_names_surface_through_block_body() {
    let source = "<Widget name=\"Odd\">\n  @state {\n    Count: 0,\n  }\n</Widget><Odd />";
    let error = parse_document(source).expect_err("非法字段名必须失败");
    assert!(error.message.contains("Rust 标识符"));
}

// 插槽扫描不会把成员块误当作插槽或内容；成员块不进入模板体。
#[test]
fn slot_scanning_ignores_member_blocks() {
    let widget = raw_widget_of(
        "<Widget name=\"Panel\">\n  @state {\n    open: false,\n  }\n  <Slot />\n</Widget><Panel>主体</Panel>",
    );
    // 成员块被剥离；模板只保留默认插槽。
    assert_eq!(widget.slots.len(), 1);
    assert!(widget.slots[0].name.is_none());
    // 子节点中不存在任何成员块节点。
    assert!(
        !widget
            .children
            .iter()
            .any(|node| matches!(node, Node::WidgetMember(_)))
    );
}

// 四类成员的类别标签与属性名一致。
#[test]
fn member_kind_labels_match_attribute_names() {
    assert_eq!(WidgetMemberKind::Props.as_str(), "props");
    assert_eq!(WidgetMemberKind::State.as_str(), "state");
    assert_eq!(WidgetMemberKind::Computed.as_str(), "computed");
    assert_eq!(WidgetMemberKind::Actions.as_str(), "actions");
}

// 移除调试串中的 SourceSpan 细节，使断言只比较结构与内容。
fn strip_spans(debug: &str) -> String {
    // 逐段替换带字段的跨度结构。
    let mut output = String::with_capacity(debug.len());
    // 保留扫描位置。
    let mut rest = debug;
    // 循环定位跨度前缀。
    while let Some(position) = rest.find("SourceSpan {") {
        // 写入前缀之前的内容。
        output.push_str(&rest[..position]);
        // 追加占位标记。
        output.push_str("SourceSpan");
        // 定位结构结束大括号。
        let tail_start = position + "SourceSpan {".len();
        // 计算结束偏移。
        let close = rest[tail_start..].find('}').expect("跨度结构必须闭合");
        // 前进到结构之后。
        rest = &rest[tail_start + close + 1..];
    }
    // 补齐尾段。
    output.push_str(rest);
    output
}
