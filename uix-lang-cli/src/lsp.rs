//! 最小 LSP stdio Adapter；会话状态与文档缓存只属于 CLI。

use serde_json::{Value, json};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use uix_lang_compiler::{CompileTarget, CompilerDiagnostic, CompilerSystem};

#[derive(Default)]
struct Session {
    documents: std::collections::BTreeMap<String, String>,
    shutdown: bool,
}

pub fn run_stdio() -> Result<u8, String> {
    let mut input = Vec::new();
    io::stdin()
        .read_to_end(&mut input)
        .map_err(|e| e.to_string())?;
    let mut offset = 0;
    let mut session = Session::default();
    let mut stdout = io::BufWriter::new(io::stdout());
    while let Some(body) = next_message(&input, &mut offset)? {
        if let Some(response) = handle(&mut session, &body) {
            write_message(&mut stdout, &response)?;
        }
        if session.shutdown {
            break;
        }
    }
    stdout.flush().map_err(|e| e.to_string())?;
    Ok(0)
}

fn next_message(input: &[u8], offset: &mut usize) -> Result<Option<Value>, String> {
    if *offset >= input.len() {
        return Ok(None);
    }
    let rest = std::str::from_utf8(&input[*offset..]).map_err(|e| e.to_string())?;
    let Some(end) = rest.find("\r\n\r\n").or_else(|| rest.find("\n\n")) else {
        return Err("LSP header 不完整".into());
    };
    let header = &rest[..end];
    let mut length = None;
    for line in header.lines() {
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = Some(value.trim().parse::<usize>().map_err(|e| e.to_string())?);
        }
    }
    let length = length.ok_or("缺少 Content-Length")?;
    let separator = if rest.as_bytes()[end..].starts_with(b"\r\n\r\n") {
        4
    } else {
        2
    };
    let start = *offset + end + separator;
    if input.len() < start + length {
        return Err("LSP body 不完整".into());
    }
    *offset = start + length;
    serde_json::from_slice(&input[start..start + length])
        .map(Some)
        .map_err(|e| e.to_string())
}

fn write_message(out: &mut impl Write, value: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    write!(out, "Content-Length: {}\r\n\r\n", body.len())
        .and_then(|_| out.write_all(&body))
        .map_err(|e| e.to_string())
}

fn handle(session: &mut Session, request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let result = match method {
        "initialize" => json!({"capabilities":{"textDocumentSync":1,"documentFormattingProvider":true,"completionProvider":{"triggerCharacters":["<","@"]},"hoverProvider":true,"definitionProvider":true,"referencesProvider":true}}),
        "initialized" | "$/cancelRequest" => return None,
        "shutdown" => { session.shutdown = true; Value::Null },
        "exit" => return None,
        "textDocument/didOpen" => { update_document(session, request); return Some(diagnostic_notification(session, request)); }
        "textDocument/didChange" => { update_document(session, request); return Some(diagnostic_notification(session, request)); }
        "textDocument/formatting" => formatting(session, request),
        "textDocument/completion" => completion(request),
        "textDocument/hover" => hover(session, request),
        "textDocument/definition" => locations(session, request, false),
        "textDocument/references" => locations(session, request, true),
        _ => return id.map(|id| json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}})),
    };
    id.map(|id| json!({"jsonrpc":"2.0","id":id,"result":result}))
}

fn update_document(session: &mut Session, request: &Value) {
    let params = &request["params"];
    let text = params
        .pointer("/textDocument/text")
        .and_then(Value::as_str)
        .map(str::to_string);
    let uri = params
        .pointer("/textDocument/uri")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if let Some(text) = text {
        session.documents.insert(uri, text);
        return;
    }
    if let Some(changes) = params.get("contentChanges").and_then(Value::as_array) {
        if let Some(text) = changes
            .last()
            .and_then(|v| v.get("text"))
            .and_then(Value::as_str)
        {
            session.documents.insert(uri, text.to_string());
        }
    }
}

fn diagnostic_notification(session: &Session, request: &Value) -> Value {
    let uri = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (text, name) = source(session, uri);
    let path = uri_to_path(uri);
    let result = path
        .as_ref()
        .filter(|p| std::fs::read_to_string(p).ok().as_deref() == Some(text.as_str()))
        .and_then(|p| {
            CompilerSystem::new()
                .check_file(p, CompileTarget::View)
                .err()
        })
        .or_else(|| {
            CompilerSystem::new()
                .check_inline(&text, &name, CompileTarget::View)
                .err()
        });
    let diagnostics = result.into_iter().map(lsp_diagnostic).collect::<Vec<_>>();
    json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":diagnostics}})
}

fn lsp_diagnostic(error: CompilerDiagnostic) -> Value {
    json!({"range":{"start":{"line":error.line.saturating_sub(1),"character":error.column.saturating_sub(1)},"end":{"line":error.line.saturating_sub(1),"character":error.column}},"severity":1,"code":error.code,"source":"uix","message":format!("{}\n建议: {}", error.message, error.suggestion)})
}

fn source(session: &Session, uri: &str) -> (String, String) {
    let path = uri_to_path(uri);
    let text = session
        .documents
        .get(uri)
        .cloned()
        .or_else(|| path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()))
        .unwrap_or_default();
    (
        text,
        path.map(|p| p.display().to_string())
            .unwrap_or_else(|| uri.to_string()),
    )
}
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    uri.strip_prefix("file://").map(|p| {
        PathBuf::from(if p.starts_with('/') {
            p.to_string()
        } else {
            format!("/{p}")
        })
    })
}

fn formatting(session: &Session, request: &Value) -> Value {
    let uri = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (text, name) = source(session, uri);
    match CompilerSystem::new().format_inline(&text, name) {
        Ok(out) => {
            json!([{"range":{"start":{"line":0,"character":0},"end":{"line":text.lines().count()+1,"character":0}},"newText":out.formatted}])
        }
        Err(_) => Value::Null,
    }
}

fn completion(request: &Value) -> Value {
    let schema = CompilerSystem::new().schema();
    let mut items = Vec::new();
    for e in schema.components() {
        items.push(json!({"label":e.name,"kind":7,"detail":"UIX component"}));
    }
    for e in schema.common_attributes() {
        items.push(json!({"label":e.name,"kind":10,"detail":"UIX attribute"}));
    }
    for e in schema.events() {
        items.push(json!({"label":e.name,"kind": EventKind::EVENT,"detail":e.fields.join(", ")}));
    }
    for e in schema.style_properties() {
        items.push(json!({"label":e.name,"kind":10,"detail":"style"}));
    }
    for e in schema.theme_tokens() {
        items.push(json!({"label":e.name,"kind":21,"detail":"theme token"}));
    }
    let _ = request;
    json!({"isIncomplete":false,"items":items})
}
struct EventKind;
impl EventKind {
    const EVENT: u8 = 14;
}

fn hover(session: &Session, request: &Value) -> Value {
    let uri = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
        .unwrap_or("");
    let position = &request["params"]["position"];
    let (text, _) = source(session, uri);
    let word = word_at(
        &text,
        position["line"].as_u64().unwrap_or(0) as usize,
        position["character"].as_u64().unwrap_or(0) as usize,
    );
    let schema = CompilerSystem::new().schema();
    let detail = schema
        .component(&word)
        .map(|e| format!("{} ({:?})", e.name, e.category))
        .or_else(|| {
            schema
                .style_property(&word)
                .map(|e| format!("style {} ({:?})", e.name, e.value_kind))
        })
        .or_else(|| {
            schema
                .theme_token(&word)
                .map(|e| format!("theme {} ({:?})", e.name, e.kind))
        });
    detail
        .map(|value| json!({"contents":{"kind":"markdown","value":value}}))
        .unwrap_or(Value::Null)
}

fn locations(session: &Session, request: &Value, references: bool) -> Value {
    let uri = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
        .unwrap_or("");
    let position = &request["params"]["position"];
    let (text, _) = source(session, uri);
    let word = word_at(
        &text,
        position["line"].as_u64().unwrap_or(0) as usize,
        position["character"].as_u64().unwrap_or(0) as usize,
    );
    let mut result = Vec::new();
    for (line, value) in text.lines().enumerate() {
        let mut start = 0;
        while let Some(index) = value[start..].find(&word) {
            let col = start + index;
            result.push(json!({"uri":uri,"range":{"start":{"line":line,"character":col},"end":{"line":line,"character":col+word.len()}}}));
            start = col + word.len();
            if start >= value.len() {
                break;
            }
        }
    }
    if references {
        json!(result)
    } else {
        result.into_iter().next().unwrap_or(Value::Null)
    }
}

fn word_at(text: &str, line: usize, character: usize) -> String {
    text.lines()
        .nth(line)
        .and_then(|v| v.get(..character.min(v.len())))
        .map(|prefix| {
            prefix
                .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_round_trip() {
        let body = serde_json::to_vec(&json!({"method": "shutdown"})).unwrap();
        let mut data = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        data.extend_from_slice(&body);
        let mut at = 0;
        assert_eq!(
            next_message(&data, &mut at).unwrap().unwrap()["method"],
            "shutdown"
        );
    }
    #[test]
    fn initialize_has_core_capabilities() {
        let mut session = Session::default();
        let response = handle(&mut session, &json!({"id":1,"method":"initialize"})).unwrap();
        assert_eq!(response["result"]["capabilities"]["hoverProvider"], true);
    }
}
