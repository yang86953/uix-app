//! 最小 LSP stdio Adapter；会话状态与文档缓存只属于 CLI。

use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use uix_lang_compiler::{CompilerDiagnostic, CompilerSession, CompilerSystem};

#[derive(Debug)]
enum OpenDocument {
    // 文件文档的源码只由 overlays 拥有，文档索引不保存第二份 String。
    File(PathBuf),
    // 非 file URI 没有可传给 CompilerSession 的稳定路径，继续由 LSP 会话拥有源码。
    Virtual(String),
}

#[derive(Default)]
struct Session {
    documents: BTreeMap<String, OpenDocument>,
    // LSP Adapter 持久拥有唯一文件源码快照，诊断请求直接借用而不重建整表。
    overlays: BTreeMap<PathBuf, String>,
    diagnostics_by_root: BTreeMap<String, BTreeMap<String, Vec<Value>>>,
    // LSP Adapter 独占编译会话；进程退出或文档关闭时释放对应阶段缓存。
    compiler: CompilerSession,
    shutdown: bool,
}

impl Session {
    fn store_document(&mut self, uri: String, source: String) {
        // 同一 URI 更新前先释放旧路径快照，避免 URI 改类时残留不可达源码。
        self.remove_document(&uri);
        if let Some(path) = uri_to_path(&uri) {
            self.overlays.insert(path.clone(), source);
            self.documents.insert(uri, OpenDocument::File(path));
        } else {
            self.documents.insert(uri, OpenDocument::Virtual(source));
        }
    }

    fn remove_document(&mut self, uri: &str) -> Option<PathBuf> {
        match self.documents.remove(uri) {
            Some(OpenDocument::File(path)) => {
                // URI 别名仍指向同一路径时保留共享快照，直到最后一个文档所有者关闭。
                let still_open = self.documents.values().any(
                    |document| matches!(document, OpenDocument::File(candidate) if candidate == &path),
                );
                if !still_open {
                    self.overlays.remove(&path);
                }
                Some(path)
            }
            Some(OpenDocument::Virtual(_)) | None => None,
        }
    }

    fn document_source(&self, uri: &str) -> Option<&str> {
        match self.documents.get(uri)? {
            OpenDocument::File(path) => self.overlays.get(path).map(String::as_str),
            OpenDocument::Virtual(source) => Some(source.as_str()),
        }
    }
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
        for response in handle(&mut session, &body) {
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

fn handle(session: &mut Session, request: &Value) -> Vec<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let result = match method {
        "initialize" => json!({"capabilities":{"textDocumentSync":{"openClose":true,"change":1},"documentFormattingProvider":true,"completionProvider":{"triggerCharacters":["<","@"]},"hoverProvider":true,"definitionProvider":true,"referencesProvider":true}}),
        "initialized" | "$/cancelRequest" => return Vec::new(),
        "shutdown" => { session.shutdown = true; Value::Null },
        "exit" => return Vec::new(),
        "textDocument/didOpen" => { update_document(session, request); return diagnostic_notifications(session, request); }
        "textDocument/didChange" => { update_document(session, request); return diagnostic_notifications(session, request); }
        "textDocument/didClose" => return close_document(session, request),
        "textDocument/formatting" => formatting(session, request),
        "textDocument/completion" => completion(request),
        "textDocument/hover" => hover(session, request),
        "textDocument/definition" => locations(session, request, false),
        "textDocument/references" => locations(session, request, true),
        _ => return id.into_iter().map(|id| json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}})).collect(),
    };
    id.into_iter()
        .map(|id| json!({"jsonrpc":"2.0","id":id,"result":result}))
        .collect()
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
        session.store_document(uri, text);
        return;
    }
    if let Some(changes) = params.get("contentChanges").and_then(Value::as_array) {
        if let Some(text) = changes
            .last()
            .and_then(|v| v.get("text"))
            .and_then(Value::as_str)
        {
            session.store_document(uri, text.to_string());
        }
    }
}

fn close_document(session: &mut Session, request: &Value) -> Vec<Value> {
    let uri = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let closed_path = session.remove_document(&uri).or_else(|| uri_to_path(&uri));
    let previous = session.diagnostics_by_root.remove(&uri).unwrap_or_default();
    let affected_roots = closed_path
        .map(|path| session.compiler.evict_file(&path))
        .unwrap_or_default();
    // 关闭依赖 overlay 后立即按落盘内容复核仍打开的根，不能等待下一次编辑才失效。
    let affected_uris = session
        .documents
        .keys()
        .filter(|candidate| {
            uri_to_path(candidate).is_some_and(|candidate_path| {
                affected_roots
                    .iter()
                    .any(|root| same_document_path(root, &candidate_path))
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut notifications = BTreeMap::<String, Value>::new();
    for root_uri in affected_uris {
        for notification in diagnostic_notifications(
            session,
            &json!({"params":{"textDocument":{"uri":root_uri}}}),
        ) {
            if let Some(target_uri) = notification.pointer("/params/uri").and_then(Value::as_str) {
                notifications.insert(target_uri.to_string(), notification);
            }
        }
    }
    // 关闭根后重新汇聚仍由其他根持有的诊断；至少发布一次结果清理客户端状态。
    let mut publish_uris = previous.keys().cloned().collect::<BTreeSet<_>>();
    publish_uris.insert(uri);
    for publish_uri in publish_uris {
        let diagnostics = session
            .diagnostics_by_root
            .values()
            .filter_map(|by_uri| by_uri.get(&publish_uri))
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        notifications.insert(
            publish_uri.clone(),
            json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":publish_uri,"diagnostics":diagnostics}}),
        );
    }
    notifications.into_values().collect()
}

fn same_document_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn diagnostic_notifications(session: &mut Session, request: &Value) -> Vec<Value> {
    let request_uri = request
        .pointer("/params/textDocument/uri")
        .and_then(Value::as_str)
        .unwrap_or("");
    let path = uri_to_path(request_uri);
    let result = if let Some(path) = path.as_ref() {
        session
            .compiler
            .check_file_with_overlays_auto(path, &session.overlays)
            .err()
    } else {
        let (text, name) = source(session, request_uri);
        CompilerSystem::new().check_inline_auto(&text, &name).err()
    };
    let (text, _) = source(session, request_uri);
    // 每轮只保留当前失败来源；先前发布到其他依赖文件的诊断必须显式清空。
    let mut active = BTreeMap::<String, Vec<Value>>::new();
    if let Some(error) = result {
        let target_uri = diagnostic_uri(&error.source_name, request_uri);
        let diagnostic_source = diagnostic_source(session, &target_uri, &error.source_name, &text);
        active.insert(target_uri, vec![lsp_diagnostic(error, &diagnostic_source)]);
    }
    // 请求文件始终发布一次，确保从根文件错误切换到依赖错误时不会残留。
    let mut publish_uris = session
        .diagnostics_by_root
        .get(request_uri)
        .into_iter()
        .flat_map(|diagnostics| diagnostics.keys().cloned())
        .collect::<BTreeSet<_>>();
    publish_uris.insert(request_uri.to_string());
    publish_uris.extend(active.keys().cloned());
    if active.is_empty() {
        session.diagnostics_by_root.remove(request_uri);
    } else {
        session
            .diagnostics_by_root
            .insert(request_uri.to_string(), active);
    }
    publish_uris
        .into_iter()
        .map(|uri| {
            let diagnostics = session
                .diagnostics_by_root
                .values()
                .filter_map(|by_uri| by_uri.get(&uri))
                .flatten()
                .cloned()
                .collect::<Vec<_>>();
            json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":diagnostics}})
        })
        .collect()
}

fn diagnostic_uri(source_name: &str, fallback: &str) -> String {
    let path = Path::new(source_name);
    if path.is_absolute() {
        #[cfg(windows)]
        return format!("file:///{}", source_name.replace('\\', "/"));
        #[cfg(not(windows))]
        return format!("file://{}", path.display());
    }
    fallback.to_string()
}

fn diagnostic_source<'a>(
    session: &'a Session,
    target_uri: &str,
    source_name: &str,
    fallback: &'a str,
) -> Cow<'a, str> {
    session
        .document_source(target_uri)
        .map(Cow::Borrowed)
        .or_else(|| std::fs::read_to_string(source_name).ok().map(Cow::Owned))
        .unwrap_or(Cow::Borrowed(fallback))
}

fn lsp_diagnostic(error: CompilerDiagnostic, source: &str) -> Value {
    let fallback_start = (error.line.saturating_sub(1), error.column.saturating_sub(1));
    let start = byte_offset_position(source, error.start).unwrap_or(fallback_start);
    let mut end = byte_offset_position(source, error.end).unwrap_or((start.0, start.1 + 1));
    if end <= start {
        end = (start.0, start.1 + 1);
    }
    json!({
        "range":{
            "start":{"line":start.0,"character":start.1},
            "end":{"line":end.0,"character":end.1}
        },
        "severity":1,
        "code":error.code,
        "source":"uix",
        "message":format!("[{}] {}\n建议: {}", error.phase.as_str(), error.message, error.suggestion),
        "data":{
            "phase":error.phase.as_str(),
            "sourceId":error.source_id.value(),
            "sourceName":error.source_name,
            "byteRange":{"start":error.start,"end":error.end}
        }
    })
}

fn byte_offset_position(source: &str, offset: usize) -> Option<(usize, usize)> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let character = source[line_start..offset].encode_utf16().count();
    Some((line, character))
}

fn source<'a>(session: &'a Session, uri: &str) -> (Cow<'a, str>, String) {
    let path = uri_to_path(uri);
    let text = session
        .document_source(uri)
        .map(Cow::Borrowed)
        .or_else(|| {
            path.as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(Cow::Owned)
        })
        .unwrap_or(Cow::Borrowed(""));
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
        let response = handle(&mut session, &json!({"id":1,"method":"initialize"}))
            .into_iter()
            .next()
            .expect("initialize 必须返回响应");
        assert_eq!(response["result"]["capabilities"]["hoverProvider"], true);
        assert_eq!(
            response["result"]["capabilities"]["textDocumentSync"]["openClose"],
            true
        );
        assert_eq!(
            response["result"]["capabilities"]["textDocumentSync"]["change"],
            1
        );
    }

    #[test]
    fn file_documents_keep_one_persistent_overlay_source() {
        let first_path =
            std::env::temp_dir().join(format!("uix-lsp-overlay-first-{}.uix", std::process::id()));
        let second_path =
            std::env::temp_dir().join(format!("uix-lsp-overlay-second-{}.uix", std::process::id()));
        let first_uri = format!("file://{}", first_path.display());
        let second_uri = format!("file://{}", second_path.display());
        let mut session = Session::default();
        session.store_document(first_uri.clone(), "<App><Text>一</Text></App>".to_string());
        session.store_document(second_uri, "<App><Text>二</Text></App>".to_string());
        let first_pointer = session
            .document_source(&first_uri)
            .expect("打开文档必须可读")
            .as_ptr();
        assert_eq!(
            first_pointer,
            session
                .overlays
                .get(&first_path)
                .expect("文件文档必须持有 overlay")
                .as_ptr(),
            "文档索引与编译 overlay 必须借用同一份源码",
        );

        let _ = diagnostic_notifications(
            &mut session,
            &json!({"params":{"textDocument":{"uri":first_uri.clone()}}}),
        );

        assert_eq!(
            session
                .document_source(&first_uri)
                .expect("诊断后打开文档必须仍可读")
                .as_ptr(),
            first_pointer,
            "诊断请求不得重建 LSP 持有的 overlay 源码",
        );
        assert_eq!(session.overlays.len(), 2);
    }

    #[test]
    fn closing_one_uri_alias_keeps_shared_overlay_until_last_owner() {
        let path =
            std::env::temp_dir().join(format!("uix-lsp-overlay-alias-{}.uix", std::process::id()));
        let mut session = Session::default();
        session.overlays.insert(path.clone(), "<App />".to_string());
        session.documents.insert(
            "file:///alias-a.uix".to_string(),
            OpenDocument::File(path.clone()),
        );
        session.documents.insert(
            "file:///alias-b.uix".to_string(),
            OpenDocument::File(path.clone()),
        );

        session.remove_document("file:///alias-a.uix");
        assert!(session.overlays.contains_key(&path));
        session.remove_document("file:///alias-b.uix");
        assert!(!session.overlays.contains_key(&path));
    }

    #[test]
    fn unsaved_app_document_uses_overlay_and_app_target() {
        let path = std::env::temp_dir().join(format!("uix-lsp-app-{}.uix", std::process::id()));
        let uri = format!("file://{}", path.display());
        let mut session = Session::default();
        session.store_document(
            uri.clone(),
            "<App title=\"Overlay\"><Text>Hello</Text></App>".to_string(),
        );
        let notifications = diagnostic_notifications(
            &mut session,
            &json!({"params":{"textDocument":{"uri":uri.clone()}}}),
        );
        let notification = notifications
            .iter()
            .find(|notification| notification["params"]["uri"] == uri)
            .expect("请求文档必须收到诊断清空通知");
        assert_eq!(
            notification["params"]["diagnostics"],
            serde_json::Value::Array(Vec::new())
        );
    }

    #[test]
    fn did_close_releases_overlay_and_clears_published_diagnostics() {
        let path = std::env::temp_dir().join(format!("uix-lsp-close-{}.uix", std::process::id()));
        let uri = format!("file://{}", path.display());
        let mut session = Session::default();
        session.store_document(uri.clone(), "<App><Unknown /></App>".to_string());
        let published = diagnostic_notifications(
            &mut session,
            &json!({"params":{"textDocument":{"uri":uri.clone()}}}),
        );
        assert!(published.iter().any(|notification| {
            !notification["params"]["diagnostics"]
                .as_array()
                .expect("诊断必须是数组")
                .is_empty()
        }));

        let cleared = handle(
            &mut session,
            &json!({"method":"textDocument/didClose","params":{"textDocument":{"uri":uri.clone()}}}),
        );

        assert!(!session.documents.contains_key(&uri));
        assert!(!session.overlays.contains_key(&path));
        assert!(!session.diagnostics_by_root.contains_key(&uri));
        assert!(cleared.iter().any(|notification| {
            notification["params"]["uri"] == uri
                && notification["params"]["diagnostics"] == serde_json::Value::Array(Vec::new())
        }));
    }

    #[test]
    fn closing_dependency_overlay_revalidates_open_root_from_disk() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("系统时钟必须可用")
            .as_nanos();
        let fixture = std::env::temp_dir().join(format!("uix-lsp-close-dependency-{unique}"));
        std::fs::create_dir(&fixture).expect("必须创建 LSP fixture");
        let helper = fixture.join("helper.uix");
        let root = fixture.join("main.uix");
        let helper_source =
            "@export('Helper')\n<Widget name=\"Helper\"><Text>共享</Text></Widget>\n<Helper />";
        let root_source = "@import('./helper.uix', 'Helper')\n<App><Helper /></App>";
        std::fs::write(&helper, helper_source).expect("必须写入有效依赖文件");
        std::fs::write(&root, root_source).expect("必须写入根文件");
        let root_uri = format!("file://{}", root.display());
        let helper_uri = format!("file://{}", helper.display());
        let mut session = Session::default();
        session.store_document(root_uri.clone(), root_source.to_string());
        session.store_document(
            helper_uri.clone(),
            "@export('Helper')\n<Widget name=\"Helper\"><Unknown /></Widget>\n<Helper />"
                .to_string(),
        );
        let published = diagnostic_notifications(
            &mut session,
            &json!({"params":{"textDocument":{"uri":root_uri.clone()}}}),
        );
        assert!(published.iter().any(|notification| {
            notification["params"]["uri"] == helper_uri
                && !notification["params"]["diagnostics"]
                    .as_array()
                    .expect("诊断必须是数组")
                    .is_empty()
        }));

        let cleared = handle(
            &mut session,
            &json!({"method":"textDocument/didClose","params":{"textDocument":{"uri":helper_uri.clone()}}}),
        );

        assert!(session.documents.contains_key(&root_uri));
        assert!(!session.documents.contains_key(&helper_uri));
        assert!(session.overlays.contains_key(&root));
        assert!(!session.overlays.contains_key(&helper));
        assert!(!session.diagnostics_by_root.contains_key(&root_uri));
        assert!(cleared.iter().any(|notification| {
            notification["params"]["uri"] == helper_uri
                && notification["params"]["diagnostics"] == serde_json::Value::Array(Vec::new())
        }));
        let _ = std::fs::remove_dir_all(fixture);
    }

    #[test]
    fn byte_offsets_are_converted_to_utf16_positions() {
        let source = "甲x\n<Text mystery=\"x\" />";
        assert_eq!(byte_offset_position(source, "甲".len()), Some((0, 1)));
        let start = source.find("mystery").expect("fixture 必须包含属性");
        assert_eq!(byte_offset_position(source, start), Some((1, 6)));
        assert_eq!(
            byte_offset_position(source, start + "mystery=\"x\"".len()),
            Some((1, 17))
        );
    }

    #[test]
    fn imported_error_is_published_on_dependency_uri_and_clears_root() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("系统时钟必须可用")
            .as_nanos();
        let fixture =
            std::env::temp_dir().join(format!("uix-lsp-import-{}-{unique}", std::process::id()));
        std::fs::create_dir(&fixture).expect("必须创建 LSP fixture");
        let helper = fixture.join("helper.uix");
        let root = fixture.join("main.uix");
        std::fs::write(
            &helper,
            "@export('Helper')\n<Widget name=\"Helper\">\n<Text mystery=\"x\">共享</Text>\n</Widget>\n<Helper />",
        )
        .expect("必须写入依赖文件");
        std::fs::write(
            &root,
            "@import('./helper.uix', 'Helper')\n<App><Helper /></App>",
        )
        .expect("必须写入根文件");
        let root_uri = format!("file://{}", root.display());
        let helper_uri = format!("file://{}", helper.display());
        let mut session = Session::default();
        let notifications = diagnostic_notifications(
            &mut session,
            &json!({"params":{"textDocument":{"uri":root_uri.clone()}}}),
        );
        let root_notification = notifications
            .iter()
            .find(|notification| notification["params"]["uri"] == root_uri)
            .expect("根文件必须收到清空通知");
        assert_eq!(
            root_notification["params"]["diagnostics"],
            serde_json::Value::Array(Vec::new())
        );
        let helper_notification = notifications
            .iter()
            .find(|notification| notification["params"]["uri"] == helper_uri)
            .expect("依赖文件必须收到真实诊断");
        let diagnostic = &helper_notification["params"]["diagnostics"][0];
        assert_eq!(diagnostic["code"], "UIX2000");
        assert_eq!(
            diagnostic["range"]["start"],
            json!({"line":2,"character":6})
        );
        assert_eq!(
            diagnostic["data"]["sourceName"],
            helper.to_string_lossy().as_ref()
        );
        // 依赖修复后必须主动清空先前发布到该文件的诊断。
        std::fs::write(
            &helper,
            "@export('Helper')\n<Widget name=\"Helper\"><Text>共享</Text></Widget>\n<Helper />",
        )
        .expect("必须修复依赖文件");
        let cleared = diagnostic_notifications(
            &mut session,
            &json!({"params":{"textDocument":{"uri":root_uri.clone()}}}),
        );
        let helper_clear = cleared
            .iter()
            .find(|notification| notification["params"]["uri"] == helper_uri)
            .expect("修复后必须清空依赖诊断");
        assert_eq!(
            helper_clear["params"]["diagnostics"],
            serde_json::Value::Array(Vec::new())
        );
        let _ = std::fs::remove_dir_all(&fixture);
    }
}
