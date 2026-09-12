//! 诊断特性：didOpen/didChange/didClose/didChangeWatchedFiles 的发布与清理。

use super::{document_snapshot, source_uri};
use crate::lang::lsp::position::LineIndex;
use crate::lang::lsp::protocol::{notification, uri_to_path};
use crate::lang::lsp::session::{Session, same_document_path};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use crate::lang::compiler::{CompilerDiagnostic, CompilerSystem, DocumentOutput};

// 处理 didOpen / didChange：更新文档后重新发布该根的完整诊断。
pub(crate) fn did_change_document(session: &mut Session, params: &Value) -> Vec<Value> {
    // didOpen 在 textDocument.text 携带全文；didChange 全量同步取最后一个变更全文。
    let uri = params
        .pointer("/textDocument/uri")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let text = params
        .pointer("/textDocument/text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            params
                .get("contentChanges")
                .and_then(Value::as_array)
                .and_then(|changes| changes.last())
                .and_then(|change| change.get("text"))
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    if let Some(text) = text {
        session.store_document(uri.clone(), text);
    }
    let mut roots = session.declaration_uris();
    roots.push(uri);
    publish_roots(session, roots)
}

// 关闭文档：释放 overlay 与缓存，并清理或重新汇聚已发布诊断。
pub(crate) fn close(session: &mut Session, uri: &str) -> Vec<Value> {
    let closed_path = session.remove_document(uri).or_else(|| uri_to_path(uri));
    let previous = session.take_diagnostics(uri);
    let affected_roots = closed_path
        .as_ref()
        .map(|path| session.evict_file(path))
        .unwrap_or_default();
    session.forget_module_root(uri);
    session.forget_component_root(uri);
    // 关闭依赖 overlay 后立即按落盘内容复核仍打开的根，不能等待下一次编辑才失效。
    let mut affected_uris = session
        .open_file_documents()
        .into_iter()
        .filter(|(_, candidate_path)| {
            affected_roots
                .iter()
                .any(|root| same_document_path(Path::new(root), candidate_path))
        })
        .map(|(candidate_uri, _)| candidate_uri)
        .collect::<Vec<_>>();
    affected_uris.extend(session.declaration_uris());
    let mut notifications = BTreeMap::<String, Value>::new();
    for root_uri in affected_uris {
        for message in publish(session, &root_uri) {
            if let Some(target_uri) = message.pointer("/params/uri").and_then(Value::as_str) {
                notifications.insert(target_uri.to_string(), message);
            }
        }
    }
    // 关闭根后重新汇聚仍由其他根持有的诊断；至少发布一次结果清理客户端状态。
    let mut publish_uris = previous.keys().cloned().collect::<BTreeSet<_>>();
    publish_uris.insert(uri.to_string());
    for publish_uri in publish_uris {
        let diagnostics = session
            .diagnostics_for_document(&publish_uri)
            .into_iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        notifications.insert(
            publish_uri.clone(),
            notification(
                "textDocument/publishDiagnostics",
                json!({"uri": publish_uri, "diagnostics": diagnostics}),
            ),
        );
    }
    notifications.into_values().collect()
}

// 处理 workspace/didChangeWatchedFiles：磁盘依赖变化后复核仍打开的根。
pub(crate) fn watched_changed(session: &mut Session, params: &Value) -> Vec<Value> {
    // 收集本次变化的本地路径。
    let changed_paths = params
        .pointer("/changes")
        .and_then(Value::as_array)
        .map(|changes| {
            changes
                .iter()
                .filter_map(|change| change.get("uri").and_then(Value::as_str))
                .filter_map(uri_to_path)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // 找出受影响且仍打开的根：根自身或其递归依赖命中变化路径。
    let mut roots_to_recheck = session
        .open_file_documents()
        .into_iter()
        .filter(|(_, root_path)| {
            changed_paths.iter().any(|changed| {
                same_document_path(changed, root_path)
                    || session
                        .analysis_covering(root_path)
                        .is_some_and(|analysis| {
                            analysis
                                .source_graph
                                .files()
                                .iter()
                                .any(|file| same_document_path(Path::new(&file.path), changed))
                        })
                    || dependency_matches_root(session, root_path, changed)
            })
        })
        .map(|(root_uri, _)| root_uri)
        .collect::<Vec<_>>();
    // 模块检查失败时仍可能缺少完整来源图；显式文件事件复核打开的模块根，不扫描磁盘。
    roots_to_recheck.extend(session.declaration_uris());
    let mut notifications = BTreeMap::<String, Value>::new();
    for root_uri in roots_to_recheck {
        // 变化路径的编译缓存先行失效。
        for changed in &changed_paths {
            session.evict_file(changed);
        }
        for message in publish(session, &root_uri) {
            if let Some(target_uri) = message.pointer("/params/uri").and_then(Value::as_str) {
                notifications.insert(target_uri.to_string(), message);
            }
        }
    }
    notifications.into_values().collect()
}

// 语义分析缺失时回退扫描：判断受影响路径是否出现在仍打开根的源码闭包内。
fn dependency_matches_root(
    session: &Session,
    root_path: &std::path::Path,
    changed: &std::path::Path,
) -> bool {
    // 仅当根的 overlay 源码显式 @import 了变化路径的文件名时才触发复核。
    session
        .overlays()
        .get(root_path)
        .map(|source| {
            let file_name = changed
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            !file_name.is_empty() && source.contains(&file_name)
        })
        .unwrap_or(false)
}

// 对一个根执行检查并发布该根视野内的全部诊断通知。
pub(crate) fn publish(session: &mut Session, request_uri_value: &str) -> Vec<Value> {
    let path = uri_to_path(request_uri_value);
    // 文档走会话缓存检查并登记成功分析；虚拟文档走内嵌检查入口。
    let outcome = if let Some(path) = path.as_ref() {
        session.check_document_with_overlays(path)
    } else {
        let snapshot = document_snapshot(session, request_uri_value);
        CompilerSystem::new().check_document_inline(&snapshot.text, &snapshot.name)
    };
    match &outcome {
        Ok(DocumentOutput::Ui(analysis)) => {
            session.set_analysis(request_uri_value, analysis.clone())
        }
        Ok(DocumentOutput::Module(analysis)) => {
            session.set_module_analysis(request_uri_value, analysis.clone())
        }
        Ok(DocumentOutput::Component(analysis)) => {
            session.set_component_analysis(request_uri_value, analysis.clone())
        }
        Err(_) => session.drop_analyses_covering_uri(request_uri_value),
    }
    let error = outcome.err();
    let snapshot = document_snapshot(session, request_uri_value);
    // 每轮只保留当前失败来源；先前发布到其他依赖文件的诊断必须显式清空。
    let mut active = BTreeMap::<String, Vec<Value>>::new();
    if let Some(error) = error {
        let target_uri = diagnostic_uri(&error, request_uri_value);
        let diagnostic_source = diagnostic_source(session, &target_uri, &snapshot.text);
        active.insert(target_uri, vec![lsp_diagnostic(error, &diagnostic_source)]);
    }
    // 请求文件始终发布一次，确保从根文件错误切换到依赖错误时不会残留。
    let mut publish_uris = session
        .take_diagnostics(request_uri_value)
        .into_keys()
        .collect::<BTreeSet<_>>();
    publish_uris.insert(request_uri_value.to_string());
    publish_uris.extend(active.keys().cloned());
    if active.is_empty() {
        session.remove_diagnostics(request_uri_value);
    } else {
        session.set_diagnostics(request_uri_value, active.clone());
    }
    publish_uris
        .into_iter()
        .map(|uri| {
            let diagnostics = session
                .diagnostics_for_document(&uri)
                .into_iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>();
            notification(
                "textDocument/publishDiagnostics",
                json!({"uri": uri, "diagnostics": diagnostics}),
            )
        })
        .collect()
}

// 一次显式编辑只发布每个目标文档的最终汇总结果，避免中间重复通知覆盖新诊断。
fn publish_roots(session: &mut Session, roots: Vec<String>) -> Vec<Value> {
    let mut messages = BTreeMap::new();
    for root in roots.into_iter().collect::<BTreeSet<_>>() {
        for message in publish(session, &root) {
            if let Some(uri) = message.pointer("/params/uri").and_then(Value::as_str) {
                messages.insert(uri.to_string(), message);
            }
        }
    }
    messages.into_values().collect()
}

// 把编译器诊断的目标来源映射为客户端 URI；相对来源回退请求根。
fn diagnostic_uri(error: &CompilerDiagnostic, request: &str) -> String {
    source_uri(&error.source_name, request)
}

// 读取诊断目标来源的当前文本：优先打开 overlay，其次磁盘，最后回退根文本。
fn diagnostic_source(session: &Session, target_uri: &str, fallback: &str) -> String {
    session
        .document_source(target_uri)
        .map(str::to_string)
        .or_else(|| uri_to_path(target_uri).and_then(|path| std::fs::read_to_string(path).ok()))
        .unwrap_or_else(|| fallback.to_string())
}

// 把编译器诊断适配为 LSP Diagnostic；范围使用 UTF-16 行列。
pub(crate) fn lsp_diagnostic(error: CompilerDiagnostic, source: &str) -> Value {
    // 位置换算对越界与非法边界钳制，保证总能给出合法范围。
    let index = LineIndex::new(source);
    let start = index.position(source, error.start);
    let mut end = index.position(source, error.end);
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

#[cfg(test)]
mod tests {
    use super::{did_change_document, lsp_diagnostic, publish};
    use crate::lang::lsp::protocol::notification;
    use crate::lang::lsp::session::Session;
    use serde_json::json;
    use crate::lang::compiler::CompilerSystem;

    #[test]
    fn unsaved_app_document_uses_overlay_and_publishes_empty_diagnostics() {
        let path = std::env::temp_dir().join(format!("uix-lsp-app-{}.uix", std::process::id()));
        let uri = format!("file://{}", path.display());
        let mut session = Session::default();
        session.store_document(
            uri.clone(),
            "<App title=\"Overlay\"><Text>Hello</Text></App>".to_string(),
        );
        let messages = publish(&mut session, &uri);
        let message = messages
            .iter()
            .find(|message| message["params"]["uri"] == uri)
            .expect("请求文档必须收到诊断通知");
        assert_eq!(
            message["params"]["diagnostics"],
            serde_json::Value::Array(Vec::new())
        );
        // 成功检查必须登记分析供导航复用。
        assert!(session.analysis(&uri).is_some());
    }

    #[test]
    fn did_open_and_did_change_share_one_entry() {
        let mut session = Session::default();
        let opened = did_change_document(
            &mut session,
            &json!({"textDocument":{"uri":"untitled:Note","text":"<App><Text>Hi</Text></App>"}}),
        );
        assert!(
            opened
                .iter()
                .any(|message| message["params"]["uri"] == "untitled:Note")
        );
        let changed = did_change_document(
            &mut session,
            &json!({"textDocument":{"uri":"untitled:Note"},"contentChanges":[{"text":"<App><Unknown /></App>"}]}),
        );
        let diagnostics = changed
            .iter()
            .find(|message| message["params"]["uri"] == "untitled:Note")
            .expect("变更后必须重新发布")["params"]["diagnostics"]
            .as_array()
            .expect("诊断必须是数组")
            .clone();
        assert_eq!(diagnostics.len(), 1);
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
        let messages = publish(&mut session, &root_uri);
        let root_notification = messages
            .iter()
            .find(|message| message["params"]["uri"] == root_uri)
            .expect("根文件必须收到清空通知");
        assert_eq!(
            root_notification["params"]["diagnostics"],
            serde_json::Value::Array(Vec::new())
        );
        let helper_notification = messages
            .iter()
            .find(|message| message["params"]["uri"] == helper_uri)
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
        let cleared = publish(&mut session, &root_uri);
        let helper_clear = cleared
            .iter()
            .find(|message| message["params"]["uri"] == helper_uri)
            .expect("修复后必须清空依赖诊断");
        assert_eq!(
            helper_clear["params"]["diagnostics"],
            serde_json::Value::Array(Vec::new())
        );
        let _ = std::fs::remove_dir_all(fixture);
    }

    #[test]
    fn diagnostics_carry_utf16_ranges_and_phase_data() {
        // 合法根元素 + 第 2 行的未知属性，验证 UTF-16 行列换算。
        let source = "<Column>\n<Text mystery=\"x\" /></Column>";
        let error = CompilerSystem::new()
            .check_inline_auto(source, "inline.uix")
            .expect_err("未知属性必须失败");
        let diagnostic = lsp_diagnostic(error, source);
        assert_eq!(
            diagnostic["range"]["start"],
            json!({"line":1,"character":6})
        );
        assert_eq!(diagnostic["severity"], 1);
        assert_eq!(diagnostic["source"], "uix");
        assert!(
            diagnostic["message"]
                .as_str()
                .expect("必须携带消息")
                .contains("建议:")
        );
    }

    #[test]
    fn notification_shape_is_valid_jsonrpc() {
        let message = notification(
            "textDocument/publishDiagnostics",
            json!({"uri":"file:///a.uix","diagnostics":[]}),
        );
        assert_eq!(message["jsonrpc"], "2.0");
        assert_eq!(message["method"], "textDocument/publishDiagnostics");
        assert!(message.get("id").is_none());
    }
}
