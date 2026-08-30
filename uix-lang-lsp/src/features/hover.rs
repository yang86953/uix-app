//! 悬停特性：语义 IR 最窄节点优先，schema 词查询兜底。

use super::{analysis_for, document_snapshot, offset_at, request_position, request_uri};
use crate::scanner;
use crate::session::Session;
use serde_json::{Value, json};
use uix_lang_compiler::semantic_ir::{SemanticNodeInfo, SemanticNodeKind, TypedDeclarationKind};
use uix_lang_compiler::source_graph::SourceId;
use uix_lang_compiler::{CompilerSystem, projection_schema::UiProjectionSchema};

// 生成 textDocument/hover 响应。
pub(crate) fn hover(session: &mut Session, request: &Value) -> Value {
    let uri = request_uri(request);
    let snapshot = document_snapshot(session, &uri);
    let (line, character) = request_position(request);
    let offset = offset_at(&snapshot.text, line, character);
    // 优先使用语义 IR 的最窄节点，携带精确的元素与属性角色。
    if let Some(analysis) = analysis_for(session, &uri) {
        let source_id = SourceId::from_source_name(&snapshot.name);
        if let Some(node) = analysis.ir.semantic_node_at(source_id, offset) {
            let schema = CompilerSystem::new().schema();
            if let Some(markdown) = semantic_hover(&analysis.ir, source_id, &node, schema) {
                return json!({"contents": {"kind": "markdown", "value": markdown}});
            }
        }
    }
    // IR 不可用或未命中时退回 schema 词查询。
    schema_word_hover(&snapshot.text, offset)
}

// 按语义节点类别生成悬停内容；返回 None 表示该位置无悬停价值。
fn semantic_hover(
    ir: &uix_lang_compiler::semantic_ir::TypedUiIr,
    source_id: SourceId,
    node: &SemanticNodeInfo,
    schema: UiProjectionSchema,
) -> Option<String> {
    match node.kind {
        SemanticNodeKind::Attribute => {
            // 事件属性直接读 schema 载荷；其余按组件上下文解释。
            if let Some(event) = schema.event(&node.name) {
                return Some(event_markdown(event));
            }
            let component = enclosing_component(ir, source_id, node)?;
            Some(attribute_markdown(schema, &component, &node.name))
        }
        SemanticNodeKind::Element => {
            // 未命中任何登记的标签退回词查询，覆盖拼写进行中的场景。
            if let Some(component) = schema.component(&node.name) {
                return Some(component_markdown(component));
            }
            match declaration_summary(ir, &node.name) {
                Some(summary) => Some(summary),
                None => Some(control_element_markdown(&node.name)),
            }
        }
        SemanticNodeKind::Declaration => Some(declaration_markdown(ir, &node.name)),
        SemanticNodeKind::Text | SemanticNodeKind::Interpolation => None,
    }
}

// 生成组件登记的悬停文档。
fn component_markdown(component: &uix_lang_compiler::projection_schema::ComponentSpec) -> String {
    let capability = component
        .capability
        .map(|capability| format!("\n\n- 需要 capability `{capability}`"))
        .unwrap_or_default();
    let status = match component.status {
        uix_lang_compiler::projection_schema::RegistrationStatus::Available => "可用",
        uix_lang_compiler::projection_schema::RegistrationStatus::Planned => "已登记未开放",
    };
    format!(
        "**{}**（{}组件）\n\n- 状态：{status}\n- 生成入口：`{}`{capability}",
        component.name,
        component.category.label(),
        component.emitter.unwrap_or("内置")
    )
}

// 生成事件载荷的悬停文档。
fn event_markdown(event: &uix_lang_compiler::projection_schema::EventSpec) -> String {
    if event.fields.is_empty() {
        return format!("**{}** 事件\n\n- `$event` 无公开字段", event.name);
    }
    format!(
        "**{}** 事件\n\n- `$event` 公开字段：{}",
        event.name,
        event
            .fields
            .iter()
            .map(|field| format!("`{field}`"))
            .collect::<Vec<_>>()
            .join("、")
    )
}

// 按组件上下文解释属性：句柄位、通用属性、capability 与结构属性。
fn attribute_markdown(schema: UiProjectionSchema, component: &str, attribute: &str) -> String {
    // Widget 结构属性拥有语言级含义。
    if component == "Widget" {
        match attribute {
            "name" => return "**name**\n\n组件名称；被 `@export` 导出或以标签引用。".to_string(),
            "props" => return "**props**\n\n输入参数声明：`props=\"label: String = 'x', page: State<number>\"`。".to_string(),
            "state" => return "**state**\n\n私有响应式状态声明：`state=\"checked: bool = false\"`。".to_string(),
            _ => {}
        }
    }
    // State 句柄位必须接收 State<T>。
    if let Some(handle) = schema.handle_slot(component, attribute) {
        return format!(
            "**{attribute}**（{component} State 句柄位）\n\n- 值类型：`{}`",
            handle.value_type
        );
    }
    let mut markdown = match schema.common_attribute(attribute) {
        Some(entry) => format!(
            "**{attribute}**\n\n- 通用属性 · 值类别 `{:?}`",
            entry.value_kind
        ),
        None => format!("**{attribute}**（{component} 属性）"),
    };
    // 属性级 capability 门禁提示。
    if let Some(requirement) = schema.attribute_capability(component, attribute) {
        markdown.push_str(&format!(
            "\n\n- 需要 capability `{}`",
            requirement.capability
        ));
    }
    markdown
}

// 控制元素的语言级说明。
fn control_element_markdown(name: &str) -> String {
    match name {
        "If" => "**If**\n\n条件渲染：`<If {condition}>…</If>`。".to_string(),
        "ElseIf" => "**ElseIf**\n\n条件链分支：必须紧跟 `If` 或 `ElseIf`。".to_string(),
        "Else" => "**Else**\n\n条件链兜底分支：必须位于链尾。".to_string(),
        "For" => "**For**\n\n集合循环渲染，逐项绑定 `each`/`item`。".to_string(),
        "Slot" => "**Slot**\n\n组件模板插槽占位：`<Slot name=\"footer\" />`。".to_string(),
        _ => format!("`{name}` 是语言内部控制元素，不进入组件 schema。"),
    }
}

// 生成顶层声明的悬停文档。
fn declaration_markdown(ir: &uix_lang_compiler::semantic_ir::TypedUiIr, name: &str) -> String {
    match declaration_summary(ir, name) {
        Some(summary) => summary,
        None => format!("`{name}`"),
    }
}

// 汇总一个顶层声明的类别与来源；按名查找首个匹配声明。
fn declaration_summary(
    ir: &uix_lang_compiler::semantic_ir::TypedUiIr,
    name: &str,
) -> Option<String> {
    let declaration = ir
        .declarations()
        .iter()
        .find(|declaration| declaration.name == name)?;
    let kind = match declaration.kind {
        TypedDeclarationKind::Widget => "自定义组件",
        TypedDeclarationKind::Style => "样式类",
        TypedDeclarationKind::Theme => "主题",
        TypedDeclarationKind::Keyframes => "关键帧动画",
        TypedDeclarationKind::Record => "Record 数据模型",
        TypedDeclarationKind::Visual => "Visual 视觉常量",
    };
    Some(format!("**{name}**\n\n- {kind} 声明"))
}

// 找到包含指定属性节点的元素名，用于属性级 schema 查询。
fn enclosing_component(
    ir: &uix_lang_compiler::semantic_ir::TypedUiIr,
    source_id: SourceId,
    node: &SemanticNodeInfo,
) -> Option<String> {
    // 在声明体与根元素两棵树中查找跨度完全一致的属性。
    for declaration in ir.declarations() {
        if declaration.span.source_id != source_id {
            continue;
        }
        for body_node in &declaration.body {
            if let Some(found) = search_attribute_owner(body_node, source_id, node) {
                return Some(found);
            }
        }
    }
    search_attribute_owner(
        &uix_lang_compiler::semantic_ir::TypedNode::Element(ir.root().clone()),
        source_id,
        node,
    )
}

// 在类型化节点树中递归查找属性归属的元素名。
fn search_attribute_owner(
    node: &uix_lang_compiler::semantic_ir::TypedNode,
    source_id: SourceId,
    target: &SemanticNodeInfo,
) -> Option<String> {
    let element = match node {
        uix_lang_compiler::semantic_ir::TypedNode::Element(element) => element,
        _ => return None,
    };
    for attribute in &element.attributes {
        if attribute.span.source_id == source_id
            && attribute.span.start == target.span.start
            && attribute.span.end == target.span.end
        {
            return Some(element.name.clone());
        }
    }
    for child in &element.children {
        if let Some(found) = search_attribute_owner(child, source_id, target) {
            return Some(found);
        }
    }
    None
}

// schema 词查询兜底：组件、事件、样式属性与主题 token。
fn schema_word_hover(text: &str, offset: usize) -> Value {
    // 词提取失败或命中 @ 前缀事件时分别处理。
    let Some((start, end)) = scanner::word_at(text, offset) else {
        return Value::Null;
    };
    let word = &text[start..end];
    let schema = CompilerSystem::new().schema();
    let markdown = schema
        .component(word)
        .map(component_markdown)
        .or_else(|| schema.event(&format!("@{word}")).map(event_markdown))
        .or_else(|| schema.event(word).map(event_markdown))
        .or_else(|| {
            schema.style_property(word).map(|entry| {
                format!(
                    "**{}**\n\n- 样式属性 · 值类别 `{:?}`",
                    entry.name, entry.value_kind
                )
            })
        })
        .or_else(|| {
            schema.theme_token(word).map(|entry| {
                let alias = entry
                    .alias_for
                    .map(|alias| format!(" · 兼容别名 `{alias}`"))
                    .unwrap_or_default();
                format!(
                    "**{}**\n\n- 主题 token · `{:?}`{alias}",
                    entry.name, entry.kind
                )
            })
        });
    match markdown {
        Some(markdown) => json!({"contents": {"kind": "markdown", "value": markdown}}),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::hover;
    use crate::session::Session;
    use serde_json::json;

    #[test]
    fn component_hover_reports_category_and_status() {
        let mut session = Session::default();
        session.store_document("untitled:Demo".into(), "<App><Column /></App>".to_string());
        let response = hover(
            &mut session,
            &json!({"textDocument":{"uri":"untitled:Demo"},"position":{"line":0,"character":7}}),
        );
        let value = response["contents"]["value"]
            .as_str()
            .expect("必须返回悬停");
        assert!(value.contains("**Column**"), "必须命中组件名");
        assert!(
            value.contains("布局") || value.contains("组件"),
            "必须包含类别"
        );
    }

    #[test]
    fn event_hover_lists_payload_fields() {
        let mut session = Session::default();
        session.store_document(
            "untitled:Demo".into(),
            "<Button @click=\"save()\">go</Button>".to_string(),
        );
        let response = hover(
            &mut session,
            &json!({"textDocument":{"uri":"untitled:Demo"},"position":{"line":0,"character":10}}),
        );
        let value = response["contents"]["value"]
            .as_str()
            .expect("必须返回悬停");
        assert!(value.contains("**@click**"), "必须命中事件名");
    }

    #[test]
    fn unknown_words_yield_null_hover() {
        let mut session = Session::default();
        session.store_document("untitled:Demo".into(), "<App>文本</App>".to_string());
        let response = hover(
            &mut session,
            &json!({"textDocument":{"uri":"untitled:Demo"},"position":{"line":0,"character":6}}),
        );
        assert!(response.is_null() || response["contents"].is_null());
    }
}
