//! UIX Lang 语言服务器 System：stdio LSP 端口、编辑会话与语言特性的组合根。
//!
//! 语言事实（诊断、schema、格式化、源码图、语义 IR）全部来自共享
//! `uix-lang-compiler` 的公开命令；本 crate 只负责协议、会话与编辑器特性
//! 编排，不维护独立语言规则。`uix lsp` 与 `uix-lang-ls` 二进制都是这里的
//! 入口 Adapter，共享同一实现。

mod features;
mod position;
mod protocol;
mod scanner;
mod session;

use protocol::{error_response, read_message, response, write_message};
use serde_json::{Value, json};
use session::Session;
use std::io::{self, Write};

// 语言服务器标识，供客户端在 initialize 结果中展示。
pub const SERVER_NAME: &str = "uix-lang-ls";

/// 以 stdio JSON-RPC 运行语言服务器，返回进程退出码。
///
/// 退出码遵循 LSP 规范：先 `shutdown` 再 `exit` 返回 0，否则返回 1；
/// 输入流正常耗尽视为 0。协议帧错误以 `Err` 上报给进程入口。
pub fn run_stdio() -> Result<u8, String> {
    // 长寿命 LSP 只保留当前协议帧，不把整个会话历史累积到一个 Vec。
    let stdin = io::stdin();
    let mut input = io::BufReader::new(stdin.lock());
    let mut session = Session::default();
    let stdout = io::stdout();
    let mut output = io::BufWriter::new(stdout.lock());
    while let Some(message) = read_message(&mut input)? {
        for outgoing in handle(&mut session, &message) {
            write_message(&mut output, &outgoing)?;
        }
        // 每个请求后立即交付响应，不能等待进程退出或缓冲区自然填满。
        output.flush().map_err(|error| error.to_string())?;
        // exit 通知到达后按是否完成 shutdown 握手决定退出码。
        if session.exit_requested() {
            return Ok(session.exit_code());
        }
    }
    Ok(0)
}

// 分发单条 JSON-RPC 消息，返回响应与主动通知。
pub(crate) fn handle(session: &mut Session, message: &Value) -> Vec<Value> {
    // 通知没有 id；未知请求必须回 MethodNotFound，未知通知静默忽略。
    let id = message.get("id").cloned();
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        "initialize" => vec![response(id, capabilities())],
        "initialized" | "$/cancelRequest" | "$/setTrace" | "workspace/didChangeConfiguration" => {
            Vec::new()
        }
        "shutdown" => {
            session.request_shutdown();
            vec![response(id, Value::Null)]
        }
        "exit" => {
            session.request_exit();
            Vec::new()
        }
        "textDocument/didOpen" | "textDocument/didChange" => {
            features::diagnostics::did_change_document(session, params_of(message))
        }
        "textDocument/didClose" => {
            features::diagnostics::close(session, &features::request_uri(params_of(message)))
        }
        "workspace/didChangeWatchedFiles" => {
            features::diagnostics::watched_changed(session, params_of(message))
        }
        "textDocument/completion" => one(
            id,
            features::completion::complete(session, params_of(message)),
        ),
        "textDocument/hover" => one(id, features::hover::hover(session, params_of(message))),
        "textDocument/definition" => one(
            id,
            features::navigation::definition(session, params_of(message)),
        ),
        "textDocument/references" => one(
            id,
            features::navigation::references(session, params_of(message)),
        ),
        "textDocument/documentSymbol" => one(
            id,
            features::symbols::document_symbols(session, params_of(message)),
        ),
        "textDocument/formatting" => one(
            id,
            features::formatting::formatting(session, params_of(message)),
        ),
        _ => {
            if id.is_some() {
                vec![error_response(id, -32601, "Method not found")]
            } else {
                Vec::new()
            }
        }
    }
}

// 读取请求参数；缺省按 null 处理。
fn params_of(message: &Value) -> &Value {
    message.get("params").unwrap_or(&Value::Null)
}

// 把单一结果封装为响应帧列表。
fn one(id: Option<Value>, result: Value) -> Vec<Value> {
    vec![response(id, result)]
}

// 服务器能力声明：与实际实现的特性一一对应，不虚报。
fn capabilities() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": {"openClose": true, "change": 1},
            "completionProvider": {"triggerCharacters": ["<", "@", "#", "."]},
            "hoverProvider": true,
            "definitionProvider": true,
            "referencesProvider": true,
            "documentSymbolProvider": true,
            "documentFormattingProvider": true
        },
        "serverInfo": {"name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION")}
    })
}

#[cfg(test)]
mod tests {
    use super::{capabilities, handle};
    use crate::lang::lsp::session::Session;
    use serde_json::json;

    #[test]
    fn initialize_has_core_capabilities_and_server_info() {
        let mut session = Session::default();
        let responses = handle(&mut session, &json!({"id":1,"method":"initialize"}));
        let result = &responses[0]["result"];
        assert_eq!(result["capabilities"]["hoverProvider"], true);
        assert_eq!(
            result["capabilities"]["textDocumentSync"]["openClose"],
            true
        );
        assert_eq!(result["capabilities"]["textDocumentSync"]["change"], 1);
        assert_eq!(result["capabilities"]["documentSymbolProvider"], true);
        assert_eq!(result["serverInfo"]["name"], json!("uix-lang-ls"));
    }

    #[test]
    fn unknown_request_gets_method_not_found_and_notification_is_silent() {
        let mut session = Session::default();
        let request = handle(
            &mut session,
            &json!({"id":9,"method":"textDocument/typeCoverage"}),
        );
        assert_eq!(request[0]["error"]["code"], -32601);
        let notification = handle(
            &mut session,
            &json!({"method":"workspace/didChangeFolders"}),
        );
        assert!(notification.is_empty());
    }

    #[test]
    fn shutdown_then_exit_reports_clean_code() {
        let mut session = Session::default();
        let responses = handle(&mut session, &json!({"id":2,"method":"shutdown"}));
        assert!(responses[0]["result"].is_null());
        handle(&mut session, &json!({"method":"exit"}));
        assert!(session.exit_requested());
        assert_eq!(session.exit_code(), 0);
    }

    #[test]
    fn capabilities_value_is_stable() {
        let value = capabilities();
        assert_eq!(
            value["capabilities"]["completionProvider"]["triggerCharacters"][0],
            "<"
        );
        assert_eq!(value["capabilities"]["referencesProvider"], true);
    }
}
