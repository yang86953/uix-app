//! 格式化特性：复用共享 Compiler System 的无损格式化入口。

use super::{document_snapshot, request_uri};
use crate::lang::lsp::position::LineIndex;
use crate::lang::lsp::session::Session;
use serde_json::{Value, json};
use crate::lang::compiler::CompilerSystem;

// 生成 textDocument/formatting 响应：单一全文替换编辑或 null。
pub(crate) fn formatting(session: &Session, request: &Value) -> Value {
    let uri = request_uri(request);
    let snapshot = document_snapshot(session, &uri);
    // 格式化失败（语法错误）时返回 null，客户端保持原文。
    let result = if session.is_component_document(&snapshot.uri, &snapshot.text) {
        crate::lang::compiler::component_source::format(&snapshot.text, &snapshot.name)
    } else { CompilerSystem::new().format_inline(&snapshot.text, &snapshot.name) };
    let Ok(output) = result else {
        return Value::Null;
    };
    // 全文替换范围覆盖整份文档的 UTF-16 行列空间。
    let index = LineIndex::new(&snapshot.text);
    let (end_line, end_character) = index.position(&snapshot.text, snapshot.text.len());
    json!([{
        "range": {
            "start": {"line": 0, "character": 0},
            "end": {"line": end_line, "character": end_character}
        },
        "newText": output.formatted
    }])
}

#[cfg(test)]
mod tests {
    use super::formatting;
    use crate::lang::lsp::session::Session;
    use serde_json::json;

    #[test]
    fn valid_documents_receive_one_full_text_edit() {
        let mut session = Session::default();
        session.store_document(
            "untitled:Demo".into(),
            "<App><Text>Hello</Text></App>".to_string(),
        );
        let response = formatting(&session, &json!({"textDocument":{"uri":"untitled:Demo"}}));
        let edits = response.as_array().expect("必须返回编辑数组");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0]["range"]["start"], json!({"line":0,"character":0}));
        assert!(
            edits[0]["newText"]
                .as_str()
                .expect("必须携带新文本")
                .contains("<Text>")
        );
    }

    #[test]
    fn broken_documents_return_null() {
        let mut session = Session::default();
        session.store_document("untitled:Broken".into(), "<App><Text".to_string());
        let response = formatting(&session, &json!({"textDocument":{"uri":"untitled:Broken"}}));
        assert!(response.is_null());
    }
}
