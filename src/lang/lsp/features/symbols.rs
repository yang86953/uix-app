//! 大纲特性：按语义声明生成文档符号；无 IR 时退回声明扫描。

use super::{analysis_for, document_snapshot, request_uri};
use crate::lang::lsp::position::LineIndex;
use crate::lang::lsp::scanner::{self, ScanSymbolKind};
use crate::lang::lsp::session::Session;
use serde_json::{Value, json};
use crate::lang::compiler::semantic_ir::{TypedDeclarationKind, TypedNode};

// 生成 textDocument/documentSymbol 响应。
pub(crate) fn document_symbols(session: &mut Session, request: &Value) -> Value {
    if super::component_document(session, request) { return super::components::symbols(session, request); }
    if let Some(result) = super::modules::symbols(session, request) {
        return result;
    }
    let uri = request_uri(request);
    let snapshot = document_snapshot(session, &uri);
    // 有语义 IR 时按声明建层级大纲。
    if let Some(analysis) = analysis_for(session, &uri) {
        let index = LineIndex::new(&snapshot.text);
        let mut symbols = Vec::new();
        for declaration in analysis.ir.declarations() {
            // 只输出当前文档内的声明；导入闭包的声明属于各自文件。
            if declaration.span.source_id != analysis.source_graph.root() {
                continue;
            }
            symbols.push(declaration_symbol(declaration, &snapshot.text, &index));
        }
        if !symbols.is_empty() {
            return json!(symbols);
        }
    }
    // IR 不可用（语法未完成或存在错误）时退回扫描大纲。
    scan_symbols(&snapshot.text)
}

// 把类型化声明转换为 DocumentSymbol；Widget 声明带模板元素子树。
fn declaration_symbol(
    declaration: &crate::lang::compiler::semantic_ir::TypedDeclaration,
    text: &str,
    index: &LineIndex,
) -> Value {
    let children = declaration
        .body
        .iter()
        .filter_map(|node| element_symbol(node, text, index))
        .collect::<Vec<_>>();
    let mut symbol = base_symbol(
        &declaration.name,
        declaration_kind_code(declaration.kind),
        text,
        index,
        declaration.span.start,
        declaration.span.end,
    );
    if !children.is_empty() {
        symbol["children"] = json!(children);
    }
    symbol
}

// 把类型化节点转换为元素符号；仅元素节点产生大纲项。
fn element_symbol(node: &TypedNode, text: &str, index: &LineIndex) -> Option<Value> {
    let element = match node {
        TypedNode::Element(element) => element,
        TypedNode::Text(_) | TypedNode::Interpolation(_) => return None,
    };
    let children = element
        .children
        .iter()
        .filter_map(|child| element_symbol(child, text, index))
        .collect::<Vec<_>>();
    let mut symbol = base_symbol(
        &element.name,
        19,
        text,
        index,
        element.span.start,
        element.span.end,
    );
    if !children.is_empty() {
        symbol["children"] = json!(children);
    }
    Some(symbol)
}

// 构造一个 DocumentSymbol 基础值；选择范围与元素范围一致。
fn base_symbol(
    name: &str,
    kind: i64,
    text: &str,
    index: &LineIndex,
    start: usize,
    end: usize,
) -> Value {
    let (start_line, start_character) = index.position(text, start);
    let (end_line, end_character) = index.position(text, end);
    json!({
        "name": name,
        "kind": kind,
        "range": {
            "start": {"line": start_line, "character": start_character},
            "end": {"line": end_line, "character": end_character}
        },
        "selectionRange": {
            "start": {"line": start_line, "character": start_character},
            "end": {"line": start_line, "character": start_character}
        }
    })
}

// 声明类别到 LSP SymbolKind 的映射。
fn declaration_kind_code(kind: TypedDeclarationKind) -> i64 {
    match kind {
        // 自定义组件使用类符号。
        TypedDeclarationKind::Widget => 5,
        // Record 数据模型使用结构体符号。
        TypedDeclarationKind::Record => 23,
        // Visual 常量使用常量符号。
        TypedDeclarationKind::Visual => 14,
        // 样式类、主题、关键帧与媒体块使用对象符号。
        TypedDeclarationKind::Style
        | TypedDeclarationKind::Media
        | TypedDeclarationKind::Theme
        | TypedDeclarationKind::Keyframes => 19,
    }
}

// 扫描大纲：语法未完成时仍然给出顶层声明。
fn scan_symbols(text: &str) -> Value {
    let index = LineIndex::new(text);
    let symbols = scanner::scan_declaration_spans(text)
        .into_iter()
        .map(|symbol| {
            base_symbol(
                &symbol.name,
                scan_kind_code(symbol.kind),
                text,
                &index,
                symbol.start,
                symbol.end,
            )
        })
        .collect::<Vec<_>>();
    json!(symbols)
}

// 扫描类别到 LSP SymbolKind 的映射。
fn scan_kind_code(kind: ScanSymbolKind) -> i64 {
    match kind {
        ScanSymbolKind::Widget => 5,
        ScanSymbolKind::StyleClass | ScanSymbolKind::Theme | ScanSymbolKind::Keyframes => 19,
    }
}

#[cfg(test)]
mod tests {
    use super::document_symbols;
    use crate::lang::lsp::session::Session;
    use serde_json::json;

    #[test]
    fn ir_symbols_cover_widgets_with_element_tree() {
        let mut session = Session::default();
        session.store_document(
            "untitled:Demo".into(),
            "<Widget name=\"Greeting\"><Column><Text>hi</Text></Column></Widget>\n<App><Greeting /></App>"
                .to_string(),
        );
        let response = document_symbols(
            &mut session,
            &json!({"textDocument":{"uri":"untitled:Demo"}}),
        );
        let symbols = response.as_array().expect("必须返回符号数组");
        let widget = symbols
            .iter()
            .find(|symbol| symbol["name"] == "Greeting")
            .expect("必须包含 Widget 声明符号");
        assert_eq!(widget["kind"], 5);
        let children = widget["children"].as_array().expect("Widget 必须有子树");
        assert_eq!(children[0]["name"], "Column");
        assert_eq!(children[0]["children"][0]["name"], "Text");
    }

    #[test]
    fn broken_documents_fall_back_to_scan_symbols() {
        let mut session = Session::default();
        session.store_document(
            "untitled:Broken".into(),
            "pagePanel {\n  padding: 16px;\n}\n<Widget name=\"Half\"".to_string(),
        );
        let response = document_symbols(
            &mut session,
            &json!({"textDocument":{"uri":"untitled:Broken"}}),
        );
        let symbols = response.as_array().expect("必须返回符号数组");
        let names = symbols
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"pagePanel"));
        assert!(names.contains(&"Half"));
    }
}
