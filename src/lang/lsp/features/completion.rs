//! 补全特性：按光标语法上下文从共享 schema 与文档声明生成补全项。

use super::{document_snapshot, offset_at, request_position, request_uri};
use crate::lang::compiler::CompilerSystem;
use crate::lang::compiler::projection_schema::RegistrationStatus;
use crate::lang::lsp::scanner::{self, CompletionContext, ScanSymbolKind};
use crate::lang::lsp::session::Session;
use serde_json::{Value, json};
use std::collections::BTreeSet;

// 生成 textDocument/completion 响应。
pub(crate) fn complete(session: &Session, request: &Value) -> Value {
    if let Some(result) = super::modules::completion(session, request) {
        return result;
    }
    let uri = request_uri(request);
    let snapshot = document_snapshot(session, &uri);
    if !crate::lang::compiler::components::has_scope() {
        let path = snapshot
            .path
            .as_deref()
            .unwrap_or(std::path::Path::new("."));
        return match crate::lang::compiler::components::ComponentCatalog::for_project(path) {
            Ok(catalog) => std::sync::Arc::new(catalog).with(|| complete(session, request)),
            Err(_) => json!({"isIncomplete":true,"items":[]}),
        };
    }
    let catalog = crate::lang::compiler::components::current();
    let (line, character) = request_position(request);
    let offset = offset_at(&snapshot.text, line, character);
    // 上下文判定基于编辑器轻量扫描，允许语法未完成。
    let context = scanner::completion_context(&snapshot.text, offset);
    let schema = CompilerSystem::new().schema();
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();
    match context {
        CompletionContext::TagName => {
            for (name, component) in catalog.declarations() {
                push_item(
                    &mut items,
                    &mut seen,
                    item(
                        name,
                        7,
                        format!(
                            "组件 · {} · {}",
                            component.category.as_deref().unwrap_or("library"),
                            component.unit
                        ),
                    ),
                );
            }
            // 内置组件 + 控制元素 + 文档闭包内自定义组件。
            for entry in schema.components() {
                let availability = match entry.status {
                    RegistrationStatus::Available => "可用",
                    RegistrationStatus::Planned => "已登记未开放",
                };
                let capability = entry
                    .capability
                    .map(|capability| format!(" · 需 capability `{capability}`"))
                    .unwrap_or_default();
                push_item(
                    &mut items,
                    &mut seen,
                    with_markdown(
                        item(
                            entry.name,
                            7,
                            format!(
                                "组件 · {} · {availability}{capability}",
                                entry.category.label()
                            ),
                        ),
                        format!(
                            "**{}**（{}）\n\n{}组件；生成入口 `{}`。",
                            entry.name,
                            entry.category.label(),
                            availability,
                            entry.emitter.unwrap_or("内置")
                        ),
                    ),
                );
            }
            for &name in control_elements() {
                push_item(
                    &mut items,
                    &mut seen,
                    with_markdown(
                        item(name, 14, "控制流元素"),
                        format!("`<{name}>` 是语言控制元素，不进入组件 schema。"),
                    ),
                );
            }
            for name in custom_widget_names(session, &snapshot.text) {
                push_item(
                    &mut items,
                    &mut seen,
                    with_markdown(
                        item(&name, 7, "自定义组件"),
                        format!(
                            "`<{name}>` 来自 `<Widget name=\"{name}\">` 或 `@export('{name}')`。"
                        ),
                    ),
                );
            }
        }
        CompletionContext::AttributeName { component } => {
            if let Some(component) = catalog.declarations().get(&component) {
                for (name, property) in &component.properties {
                    push_item(
                        &mut items,
                        &mut seen,
                        item(
                            name,
                            10,
                            format!(
                                "属性 · {:?}{}",
                                property.kind,
                                if property.required { " · 必填" } else { "" }
                            ),
                        ),
                    );
                }
            }
            // 共用属性是唯一 schema 契约；结构属性按标签补充。
            for entry in schema.common_attributes() {
                push_item(
                    &mut items,
                    &mut seen,
                    item(
                        entry.name,
                        10,
                        format!("属性 · 值类别 {:?}", entry.value_kind),
                    ),
                );
            }
            for &name in style_bridge_attributes() {
                push_item(&mut items, &mut seen, item(name, 10, "样式入口属性"));
            }
            if component == "Widget" {
                for &(name, detail) in widget_structural_attributes() {
                    push_item(&mut items, &mut seen, item(name, 10, detail));
                }
            }
        }
        CompletionContext::EventName { component } => {
            if let Some(component) = catalog.declarations().get(&component) {
                for (name, event) in &component.events {
                    push_item(
                        &mut items,
                        &mut seen,
                        item(
                            name,
                            23,
                            format!("事件 · $event 字段: {}", event.fields.join(", ")),
                        ),
                    );
                }
            }
            for entry in schema.events() {
                let fields = if entry.fields.is_empty() {
                    "无载荷字段".to_string()
                } else {
                    format!("$event 字段: {}", entry.fields.join(", "))
                };
                push_item(&mut items, &mut seen, item(entry.name, 23, fields));
            }
        }
        CompletionContext::StyleProperty => {
            for entry in schema.style_properties() {
                push_item(
                    &mut items,
                    &mut seen,
                    item(
                        entry.name,
                        10,
                        format!("样式属性 · 值类别 {:?}", entry.value_kind),
                    ),
                );
            }
        }
        // 样式值位：主题 token 以 # 前缀插入，样式类用于 extends 等引用。
        CompletionContext::StyleValue => {
            for entry in schema.theme_tokens() {
                let mut completion = item(&entry.name, 21, token_detail(&entry));
                completion["insertText"] = json!(format!("#{}", entry.name));
                push_item(&mut items, &mut seen, completion);
            }
            for name in style_class_names(session, &snapshot.text) {
                push_item(&mut items, &mut seen, item(&name, 7, "样式类"));
            }
        }
        CompletionContext::ThemeToken => {
            // 光标已越过 #，插入纯 token 名。
            for entry in schema.theme_tokens() {
                push_item(
                    &mut items,
                    &mut seen,
                    item(&entry.name, 21, token_detail(&entry)),
                );
            }
        }
        CompletionContext::ClassName => {
            for name in style_class_names(session, &snapshot.text) {
                push_item(&mut items, &mut seen, item(&name, 7, "样式类"));
            }
        }
        CompletionContext::Directive => {
            for (label, detail, markdown) in directives() {
                push_item(
                    &mut items,
                    &mut seen,
                    with_markdown(item(label, 14, detail), markdown),
                );
            }
        }
        CompletionContext::Plain => {}
    }
    json!({"isIncomplete": false, "items": items})
}

// 构造一个基础补全项。
fn item(label: impl Into<String>, kind: i64, detail: impl Into<String>) -> Value {
    json!({"label": label.into(), "kind": kind, "detail": detail.into()})
}

// 为补全项附加 markdown 文档。
fn with_markdown(mut entry: Value, markdown: impl Into<String>) -> Value {
    entry["documentation"] = json!({"kind": "markdown", "value": markdown.into()});
    entry
}

// 推入去重后的补全项。
fn push_item(items: &mut Vec<Value>, seen: &mut BTreeSet<String>, entry: Value) {
    // 同名补全项只保留第一个来源。
    let label = entry
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if seen.insert(label) {
        items.push(entry);
    }
}

// 主题 token 的补全详情；兼容别名一并展示。
fn token_detail(entry: &crate::lang::compiler::projection_schema::ThemeTokenSpec) -> String {
    let alias = entry
        .alias_for
        .as_ref()
        .map(|alias| format!(" · 兼容别名 {alias}"))
        .unwrap_or_default();
    format!("主题 token · {:?}{alias}", entry.kind)
}

// 语言控制元素集合。
fn control_elements() -> &'static [&'static str] {
    &["If", "ElseIf", "Else", "For", "Slot"]
}

// 样式入口属性集合；语义角色由编译器判定，这里只用于补全提示。
fn style_bridge_attributes() -> &'static [&'static str] {
    &["style", "class", "animation", "transition"]
}

// Widget 结构属性与说明。
fn widget_structural_attributes() -> &'static [(&'static str, &'static str)] {
    &[
        ("name", "组件名称"),
        ("props", "输入参数声明（含类型与默认值）"),
        ("state", "私有响应式状态声明"),
    ]
}

// 顶层指令补全项。
fn directives() -> Vec<(&'static str, &'static str, String)> {
    vec![
        (
            "@import",
            "导入其他 .uix 文件",
            "```uix\n@import('./pages/start.uix')\n@import('./helper.uix', 'Helper')\n```"
                .to_string(),
        ),
        (
            "@export",
            "导出组件供其他文件导入",
            "```uix\n@export('Helper')\n<Widget name=\"Helper\">…</Widget>\n```".to_string(),
        ),
        (
            "@theme",
            "声明主题 token 覆盖",
            "```uix\n@theme light {\n  primaryColor: #1677FF;\n}\n```".to_string(),
        ),
        (
            "@keyframes",
            "声明关键帧动画",
            "```uix\n@keyframes fade {\n  from { opacity: 0; }\n  to { opacity: 1; }\n}\n```"
                .to_string(),
        ),
    ]
}

// 汇集当前文档与全部已缓存源码闭包中的自定义组件名。
fn custom_widget_names(session: &Session, text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for symbol in scanner::scan_declaration_spans(text) {
        if symbol.kind == ScanSymbolKind::Widget {
            names.push(symbol.name);
        }
    }
    for (source, _) in session.graph_file_sources() {
        for symbol in scanner::scan_declaration_spans(&source) {
            if symbol.kind == ScanSymbolKind::Widget {
                names.push(symbol.name);
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

// 汇集当前文档与源码闭包中的样式类与主题名，供 class= 与 extends 引用。
fn style_class_names(session: &Session, text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut sources = vec![text.to_string()];
    sources.extend(
        session
            .graph_file_sources()
            .into_iter()
            .map(|(source, _)| source),
    );
    for source in sources {
        for symbol in scanner::scan_declaration_spans(&source) {
            if matches!(
                symbol.kind,
                ScanSymbolKind::StyleClass | ScanSymbolKind::Theme
            ) {
                names.push(symbol.name);
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::complete;
    use crate::lang::lsp::session::Session;
    use serde_json::json;

    // 从响应中提取全部 label。
    fn labels(response: &serde_json::Value) -> Vec<&str> {
        response["items"]
            .as_array()
            .expect("items 必须是数组")
            .iter()
            .filter_map(|item| item["label"].as_str())
            .collect()
    }

    #[test]
    fn tag_name_context_lists_components_controls_and_widgets() {
        let mut session = Session::default();
        session.store_document(
            "untitled:Demo".into(),
            "<Widget name=\"Helper\"><Text>hi</Text></Widget>\n<".to_string(),
        );
        let response = complete(
            &session,
            &json!({"textDocument":{"uri":"untitled:Demo"},"position":{"line":1,"character":1}}),
        );
        let labels = labels(&response);
        assert!(labels.contains(&"Column"), "内置组件必须出现");
        assert!(labels.contains(&"If"), "控制元素必须出现");
        assert!(labels.contains(&"Helper"), "文档内自定义组件必须出现");
    }

    #[test]
    fn event_context_lists_schema_events_only() {
        let mut session = Session::default();
        session.store_document("untitled:Demo".into(), "<Button @".to_string());
        let response = complete(
            &session,
            &json!({"textDocument":{"uri":"untitled:Demo"},"position":{"line":0,"character":9}}),
        );
        let labels = labels(&response);
        assert!(labels.contains(&"@click"));
        assert!(!labels.contains(&"gap"), "事件上下文不得混入属性");
    }

    #[test]
    fn style_contexts_split_properties_and_tokens() {
        let mut session = Session::default();
        let source = "<Text style=\"";
        session.store_document("untitled:Demo".into(), source.to_string());
        let properties = complete(
            &session,
            &json!({"textDocument":{"uri":"untitled:Demo"},"position":{"line":0,"character":13}}),
        );
        assert!(labels(&properties).contains(&"backgroundColor"));

        let source = "<Text style=\"color: #prim";
        session.store_document("untitled:Demo".into(), source.to_string());
        let tokens = complete(
            &session,
            &json!({"textDocument":{"uri":"untitled:Demo"},"position":{"line":0,"character":25}}),
        );
        assert!(labels(&tokens).contains(&"colorPrimary"));
        assert!(
            !labels(&properties).contains(&"colorPrimary"),
            "属性名位不得混入主题 token"
        );
    }

    #[test]
    fn directive_context_lists_language_directives() {
        let mut session = Session::default();
        session.store_document("untitled:Demo".into(), "@im".to_string());
        let response = complete(
            &session,
            &json!({"textDocument":{"uri":"untitled:Demo"},"position":{"line":0,"character":3}}),
        );
        assert!(labels(&response).contains(&"@import"));
    }

    #[test]
    fn plain_context_yields_empty_items() {
        let mut session = Session::default();
        session.store_document("untitled:Demo".into(), "<App>文本</App>".to_string());
        let response = complete(
            &session,
            &json!({"textDocument":{"uri":"untitled:Demo"},"position":{"line":0,"character":5}}),
        );
        assert!(labels(&response).is_empty());
    }
}
