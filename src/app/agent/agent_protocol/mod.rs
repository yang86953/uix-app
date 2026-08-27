//! 进程 Agent Bridge 的已认证、有界 JSON Lines 协议。
//!
//! 原生传输层提供私有字节流。本模块拥有帧划分语义与线上校验，但绝不直接
//! 触碰平台句柄或 `WidgetTree`。

use std::sync::Arc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::app::agent::agent_bridge::{
    AgentProcessBridge, AgentWaitCondition, AgentWaitError, AgentWaitOutcome, AgentWindowInfo,
    MAX_AGENT_WAIT_TIMEOUT,
};
use crate::app::agent::agent_screenshot::{
    MAX_SCREENSHOT_PNG_BYTES, ScreenshotEncodeError, base64_encode, encode_png_bounded,
};
use crate::app::queues::agent_command_queue::{
    AgentCommandError, AgentCommandResponse, AgentCommandTicket, AgentErrorCode, AgentSubmitError,
    AgentWindowAction, DEFAULT_AGENT_COMMAND_QUEUE_CAPACITY, MAX_AGENT_SETTLE_PASSES,
};
use crate::app::window_semantics::WindowSemanticSnapshot;
use crate::core::{Point, Rect, WidgetId, WindowId};
use crate::ui::accessibility::semantic_snapshot::{SemanticNode, SemanticTarget};
use crate::ui::semantic_action::SemanticAction;
use crate::ui::widget_snapshot::AccessibilityState;
use crate::ui::{KeyCode, KeyMod};

pub(crate) const AGENT_PROTOCOL_SCHEMA: &str = "uix.agent.v1";
pub(crate) const MAX_AGENT_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_AGENT_TEXT_BYTES: usize = 64 * 1024;
pub(crate) const MAX_AGENT_CONNECTIONS: usize = 8;
const MAX_REQUEST_ID_BYTES: usize = 128;
const MAX_AUTOMATION_ID_BYTES: usize = 512;
const AGENT_COMMAND_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
const AGENT_REQUEST_TYPES: &[&str] = &[
    "hello",
    "list_windows",
    "snapshot",
    "screenshot",
    "perform",
    "confirm",
    "wait",
];
/// `list_windows` 稳定公开的可选状态字段；旧客户端可忽略，能力客户端必须先协商。
const AGENT_WINDOW_STATE_FIELDS: &[&str] = &[
    "logical_width",
    "logical_height",
    "maximized",
    "minimized",
    "fullscreen",
    "focused",
];
const AGENT_SEMANTIC_ACTIONS: &[&str] = &[
    "invoke",
    "focus",
    "set_value",
    "insert_text",
    "select",
    "toggle",
    "increment",
    "decrement",
    "adjust",
    "scroll",
];
const AGENT_WINDOW_ACTIONS: &[&str] = &[
    "press_key",
    "click_at",
    "pointer_move",
    "pointer_down",
    "pointer_up",
    "activate_window",
    "resize_window",
    "move_window",
    "maximize_window",
    "minimize_window",
    "restore_window",
    "close_window",
];
const AGENT_KEY_MODIFIERS: &[&str] = &["shift", "ctrl", "alt", "super"];
const AGENT_KEY_CODES: &[(&str, KeyCode)] = &[
    ("a", KeyCode::A),
    ("b", KeyCode::B),
    ("c", KeyCode::C),
    ("d", KeyCode::D),
    ("e", KeyCode::E),
    ("f", KeyCode::F),
    ("g", KeyCode::G),
    ("h", KeyCode::H),
    ("i", KeyCode::I),
    ("j", KeyCode::J),
    ("k", KeyCode::K),
    ("l", KeyCode::L),
    ("m", KeyCode::M),
    ("n", KeyCode::N),
    ("o", KeyCode::O),
    ("p", KeyCode::P),
    ("q", KeyCode::Q),
    ("r", KeyCode::R),
    ("s", KeyCode::S),
    ("t", KeyCode::T),
    ("u", KeyCode::U),
    ("v", KeyCode::V),
    ("w", KeyCode::W),
    ("x", KeyCode::X),
    ("y", KeyCode::Y),
    ("z", KeyCode::Z),
    ("0", KeyCode::Num0),
    ("1", KeyCode::Num1),
    ("2", KeyCode::Num2),
    ("3", KeyCode::Num3),
    ("4", KeyCode::Num4),
    ("5", KeyCode::Num5),
    ("6", KeyCode::Num6),
    ("7", KeyCode::Num7),
    ("8", KeyCode::Num8),
    ("9", KeyCode::Num9),
    ("f1", KeyCode::F1),
    ("f2", KeyCode::F2),
    ("f3", KeyCode::F3),
    ("f4", KeyCode::F4),
    ("f5", KeyCode::F5),
    ("f6", KeyCode::F6),
    ("f7", KeyCode::F7),
    ("f8", KeyCode::F8),
    ("f9", KeyCode::F9),
    ("f10", KeyCode::F10),
    ("f11", KeyCode::F11),
    ("f12", KeyCode::F12),
    ("up", KeyCode::Up),
    ("down", KeyCode::Down),
    ("left", KeyCode::Left),
    ("right", KeyCode::Right),
    ("home", KeyCode::Home),
    ("end", KeyCode::End),
    ("page_up", KeyCode::PageUp),
    ("page_down", KeyCode::PageDown),
    ("enter", KeyCode::Enter),
    ("escape", KeyCode::Escape),
    ("backspace", KeyCode::Backspace),
    ("delete", KeyCode::Delete),
    ("tab", KeyCode::Tab),
    ("space", KeyCode::Space),
    ("insert", KeyCode::Insert),
    ("shift", KeyCode::Shift),
    ("ctrl", KeyCode::Ctrl),
    ("alt", KeyCode::Alt),
    ("super", KeyCode::Super),
];

#[derive(Debug)]
pub(crate) struct AgentProtocolReply {
    bytes: Vec<u8>,
    close_connection: bool,
    result_code: &'static str,
}

impl AgentProtocolReply {
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) const fn close_connection(&self) -> bool {
        self.close_connection
    }

    pub(crate) const fn result_code(&self) -> &'static str {
        self.result_code
    }

    fn from_value(value: Value, close_connection: bool, result_code: &'static str) -> Self {
        let mut bytes = match serde_json::to_vec(&value) {
            Ok(bytes) if bytes.len() < MAX_AGENT_MESSAGE_BYTES => bytes,
            Ok(_) | Err(_) => br#"{"schema":"uix.agent.v1","request_id":null,"ok":false,"error":{"code":"internal","message":"response exceeds the protocol limit"}}"#.to_vec(),
        };
        bytes.push(b'\n');
        Self {
            bytes,
            close_connection,
            result_code,
        }
    }
}

pub(crate) fn framing_error_reply(message: &'static str) -> AgentProtocolReply {
    error_reply(None, AgentErrorCode::InvalidRequest, message, true)
}

pub(crate) struct AgentProtocolSession {
    bridge: AgentProcessBridge,
    session_token: Arc<[u8; 32]>,
    authenticated: bool,
}

impl AgentProtocolSession {
    pub(crate) fn new(bridge: AgentProcessBridge, session_token: Arc<[u8; 32]>) -> Self {
        Self {
            bridge,
            session_token,
            authenticated: false,
        }
    }

    pub(crate) fn handle_line(&mut self, line: &[u8]) -> AgentProtocolReply {
        let started = Instant::now();
        let reply = self.handle_line_inner(line);
        tracing::info!(
            "agent request result={} duration_ms={}",
            reply.result_code(),
            started.elapsed().as_millis()
        );
        reply
    }

    fn handle_line_inner(&mut self, line: &[u8]) -> AgentProtocolReply {
        // 帧长度上限：超出即拒绝，防止内存被无界输入撑爆。
        if line.len() > MAX_AGENT_MESSAGE_BYTES {
            return error_reply(
                None,
                AgentErrorCode::InvalidRequest,
                "message exceeds the protocol limit",
                true,
            );
        }

        // 逐层解析：JSON 合法 → 顶层对象 → request_id → schema → 请求类型。
        let value: Value = match serde_json::from_slice(line) {
            Ok(value) => value,
            Err(_) => {
                return error_reply(
                    None,
                    AgentErrorCode::InvalidRequest,
                    "message is not valid JSON",
                    !self.authenticated,
                );
            }
        };
        let Some(object) = value.as_object() else {
            return error_reply(
                None,
                AgentErrorCode::InvalidRequest,
                "request must be a JSON object",
                !self.authenticated,
            );
        };
        let request_id = match request_id(object) {
            Ok(request_id) => request_id,
            Err(error) => return error.into_reply(None, !self.authenticated),
        };
        let schema = match required_string(object, "schema", MAX_REQUEST_ID_BYTES) {
            Ok(schema) => schema,
            Err(error) => return error.into_reply(Some(request_id), !self.authenticated),
        };
        // schema 不匹配说明对端协议版本不一致，直接拒绝。
        if schema != AGENT_PROTOCOL_SCHEMA {
            return error_reply(
                Some(request_id),
                AgentErrorCode::UnsupportedSchema,
                "unsupported protocol schema",
                !self.authenticated,
            );
        }
        let request_type = match required_string(object, "type", 64) {
            Ok(request_type) => request_type,
            Err(error) => return error.into_reply(Some(request_id), !self.authenticated),
        };

        // 未认证连接只允许 hello，其余请求一律先走认证。
        if !self.authenticated {
            return self.handle_hello(object, request_id, request_type);
        }

        // 已认证后按请求类型分派处理。
        match request_type {
            // 重复 hello 视为协议错误。
            "hello" => error_reply(
                Some(request_id),
                AgentErrorCode::InvalidRequest,
                "connection is already authenticated",
                false,
            ),
            "list_windows" => self.handle_list_windows(request_id),
            "snapshot" => self.handle_snapshot(object, request_id),
            "screenshot" => self.handle_screenshot(object, request_id),
            "perform" => self.handle_perform(object, request_id),
            "confirm" => self.handle_confirm(object, request_id),
            "wait" => self.handle_wait(object, request_id),
            _ => error_reply(
                Some(request_id),
                AgentErrorCode::InvalidRequest,
                "unknown request type",
                false,
            ),
        }
    }

    fn handle_hello(
        &mut self,
        object: &Map<String, Value>,
        request_id: String,
        request_type: &str,
    ) -> AgentProtocolReply {
        // 首个请求必须是 hello，否则直接拒绝并断开。
        if request_type != "hello" {
            return error_reply(
                Some(request_id),
                AgentErrorCode::Unauthorized,
                "hello must be the first request",
                true,
            );
        }
        // 校验会话 token；失败则拒绝并关闭连接。
        let token = object.get("token").and_then(Value::as_str).unwrap_or("");
        if !token_matches(self.session_token.as_ref(), token) {
            return error_reply(
                Some(request_id),
                AgentErrorCode::Unauthorized,
                "session authentication failed",
                true,
            );
        }
        self.authenticated = true;
        // 认证成功后回传进程信息、能力清单与协议限制，供对端自适应。
        success_reply(
            request_id,
            "hello",
            json!({
                "process_id": std::process::id(),
                "capabilities": {
                    "request_types": AGENT_REQUEST_TYPES,
                    "semantic_actions": AGENT_SEMANTIC_ACTIONS,
                    "window_actions": AGENT_WINDOW_ACTIONS,
                    "window_state_fields": AGENT_WINDOW_STATE_FIELDS,
                    "screenshot": true,
                    "key_names": AGENT_KEY_CODES
                        .iter()
                        .map(|(name, _)| *name)
                        .collect::<Vec<_>>(),
                    "key_modifiers": AGENT_KEY_MODIFIERS,
                },
                "limits": {
                    "max_message_bytes": MAX_AGENT_MESSAGE_BYTES,
                    "max_text_bytes": MAX_AGENT_TEXT_BYTES,
                    "max_connections": MAX_AGENT_CONNECTIONS,
                    "window_queue_capacity": DEFAULT_AGENT_COMMAND_QUEUE_CAPACITY,
                    "max_settle_passes": MAX_AGENT_SETTLE_PASSES,
                    "max_wait_ms": MAX_AGENT_WAIT_TIMEOUT.as_millis() as u64,
                    "max_screenshot_bytes": MAX_SCREENSHOT_PNG_BYTES,
                },
            }),
        )
    }

    fn handle_list_windows(&self, request_id: String) -> AgentProtocolReply {
        match self.bridge.list_windows() {
            Ok(windows) => success_reply(
                request_id,
                "list_windows",
                json!({
                    "windows": windows.iter().map(window_info_value).collect::<Vec<_>>()
                }),
            ),
            Err(error) => submit_error_reply(request_id, error),
        }
    }

    fn handle_snapshot(
        &self,
        object: &Map<String, Value>,
        request_id: String,
    ) -> AgentProtocolReply {
        let window_id = match parse_window_id(object) {
            Ok(window_id) => window_id,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        let ticket = match self.bridge.snapshot(window_id) {
            Ok(ticket) => ticket,
            Err(error) => return submit_error_reply(request_id, error),
        };
        match ticket.recv_timeout(AGENT_COMMAND_RESPONSE_TIMEOUT) {
            Ok(Ok(AgentCommandResponse::Snapshot(snapshot))) => success_reply(
                request_id,
                "snapshot",
                json!({ "snapshot": semantic_snapshot_value(&snapshot) }),
            ),
            Ok(Ok(AgentCommandResponse::Performed { .. })) => error_reply(
                Some(request_id),
                AgentErrorCode::Internal,
                "unexpected command response",
                false,
            ),
            Ok(Err(error)) => command_error_reply(request_id, error),
            Err(RecvTimeoutError::Timeout) => error_reply(
                Some(request_id),
                AgentErrorCode::Timeout,
                if ticket.cancel_pending() {
                    "UI snapshot timed out before execution and was cancelled"
                } else {
                    "UI snapshot timed out"
                },
                false,
            ),
            Err(RecvTimeoutError::Disconnected) => error_reply(
                Some(request_id),
                AgentErrorCode::AppClosed,
                "application closed before responding",
                false,
            ),
        }
    }

    /// Agent 截屏：强制出帧命令结算后，等待同一次 settle 的回读像素并编码 PNG。
    ///
    /// 失败路径一律释放回读单槽，避免同一窗口的后续截屏被未完成请求阻塞。
    fn handle_screenshot(
        &self,
        object: &Map<String, Value>,
        request_id: String,
    ) -> AgentProtocolReply {
        let window_id = match parse_window_id(object) {
            Ok(window_id) => window_id,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        let session = match self.bridge.screenshot(window_id) {
            Ok(session) => session,
            Err(error) => return submit_error_reply(request_id, error),
        };
        // 先等强制出帧命令结算；超时语义与 perform 一致（未开始可取消重试）。
        match session.command.recv_timeout(AGENT_COMMAND_RESPONSE_TIMEOUT) {
            Ok(Ok(AgentCommandResponse::Performed { .. })) => {}
            Ok(Ok(AgentCommandResponse::Snapshot(_))) => {
                self.bridge.cancel_surface_readback(window_id);
                return error_reply(
                    Some(request_id),
                    AgentErrorCode::Internal,
                    "unexpected command response",
                    false,
                );
            }
            Ok(Err(error)) => {
                self.bridge.cancel_surface_readback(window_id);
                return command_error_reply(request_id, error);
            }
            Err(RecvTimeoutError::Timeout) => {
                let cancelled = session.command.cancel_pending();
                self.bridge.cancel_surface_readback(window_id);
                return error_reply(
                    Some(request_id),
                    AgentErrorCode::Timeout,
                    if cancelled {
                        "UI screenshot timed out before execution and was cancelled"
                    } else {
                        "UI screenshot timed out"
                    },
                    false,
                );
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.bridge.cancel_surface_readback(window_id);
                return error_reply(
                    Some(request_id),
                    AgentErrorCode::AppClosed,
                    "application closed before responding",
                    false,
                );
            }
        }
        // 命令已结算：等待同一次 settle 内真实 present 完成的规范像素。
        // 票据把超时与通道关闭映射为 typed Error，这里按错误类别转协议码。
        let readback = match session.readback.recv_timeout(AGENT_COMMAND_RESPONSE_TIMEOUT) {
            Ok(readback) => readback,
            Err(error) => {
                self.bridge.cancel_surface_readback(window_id);
                if error.code() == crate::core::Errc::Timeout {
                    return error_reply(
                        Some(request_id),
                        AgentErrorCode::Timeout,
                        "surface readback did not complete before the timeout",
                        false,
                    );
                }
                // 软件回退等不支持场景映射为 unsupported_action，其余按平台失败处理。
                let code = if error.code() == crate::core::Errc::NotImplemented {
                    AgentErrorCode::UnsupportedAction
                } else {
                    AgentErrorCode::WindowOperationFailed
                };
                return error_reply(
                    Some(request_id),
                    code,
                    "surface readback could not capture the window",
                    false,
                );
            }
        };
        match encode_png_bounded(&readback) {
            Ok(png) => success_reply(
                request_id,
                "screenshot",
                json!({
                    "window_id": window_id.raw(),
                    "width": readback.width,
                    "height": readback.height,
                    "format": "png",
                    "data_base64": base64_encode(&png),
                }),
            ),
            Err(ScreenshotEncodeError::PayloadTooLarge) => error_reply(
                Some(request_id),
                AgentErrorCode::PayloadTooLarge,
                "screenshot payload exceeds the protocol limit",
                false,
            ),
            Err(_) => error_reply(
                Some(request_id),
                AgentErrorCode::Internal,
                "screenshot encoding failed",
                false,
            ),
        }
    }

    fn handle_perform(
        &self,
        object: &Map<String, Value>,
        request_id: String,
    ) -> AgentProtocolReply {
        // 解析公共字段：窗口 id、代次、期望修订号与动作对象。
        let window_id = match parse_window_id(object) {
            Ok(window_id) => window_id,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        let generation = match required_u64(object, "generation") {
            Ok(generation) => generation,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        let expected_revision = match optional_u64(object, "expected_revision") {
            Ok(revision) => revision,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        let action = match object
            .get("action")
            .ok_or_else(|| WireError::invalid("perform requires an action object"))
            .and_then(parse_action)
        {
            Ok(action) => action,
            Err(error) => return error.into_reply(Some(request_id), false),
        };

        // 按动作类别组装命令：语义动作需要 target，窗口动作禁止携带 target。
        let ticket = match action {
            ParsedAgentAction::Semantic(action) => {
                let target = match object
                    .get("target")
                    .ok_or_else(|| WireError::invalid("semantic action requires a target object"))
                    .and_then(parse_target)
                {
                    Ok(target) => target,
                    Err(error) => return error.into_reply(Some(request_id), false),
                };
                match self
                    .bridge
                    .perform(window_id, generation, expected_revision, target, action)
                {
                    Ok(ticket) => ticket,
                    Err(error) => return submit_error_reply(request_id, error),
                }
            }
            ParsedAgentAction::Window(action) => {
                if object.contains_key("target") {
                    return WireError::invalid("window action must not include a target")
                        .into_reply(Some(request_id), false);
                }
                match self
                    .bridge
                    .perform_window(window_id, generation, expected_revision, action)
                {
                    Ok(ticket) => ticket,
                    Err(error) => return submit_error_reply(request_id, error),
                }
            }
        };
        // 阻塞等待 UI 侧完成命令：成功回传执行结果，超时/断开回传对应错误。
        match ticket.recv_timeout(AGENT_COMMAND_RESPONSE_TIMEOUT) {
            Ok(Ok(AgentCommandResponse::Performed {
                window_id,
                generation,
                revision,
                presented_revision,
                settled,
            })) => success_reply(
                request_id,
                "perform",
                json!({
                    "window_id": window_id.raw(),
                    "generation": generation,
                    "revision": revision,
                    "presented_revision": presented_revision,
                    "settled": settled,
                }),
            ),
            // 响应类型与请求不匹配属于内部错误。
            Ok(Ok(AgentCommandResponse::Snapshot(_))) => error_reply(
                Some(request_id),
                AgentErrorCode::Internal,
                "unexpected command response",
                false,
            ),
            Ok(Err(error)) => command_error_reply(request_id, error),
            Err(RecvTimeoutError::Timeout) => command_timeout_reply(
                request_id,
                &ticket,
                "UI command timed out before execution and was cancelled",
                "UI command timed out after execution started; read current state before retrying",
            ),
            Err(RecvTimeoutError::Disconnected) => error_reply(
                Some(request_id),
                AgentErrorCode::AppClosed,
                "application closed before responding",
                false,
            ),
        }
    }

    /// 用户确认流程：确认执行先前命中 `requires_confirmation` 的动作。
    /// 请求携带 confirm_id；命令在 UI turn 内弹起应用确认 UI，ticket 挂起
    /// 直到用户决定（resolve）或确认失效。
    fn handle_confirm(
        &self,
        object: &Map<String, Value>,
        request_id: String,
    ) -> AgentProtocolReply {
        let window_id = match parse_window_id(object) {
            Ok(window_id) => window_id,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        let confirm_id = match required_u64(object, "confirm_id") {
            Ok(confirm_id) => confirm_id,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        let ticket = match self.bridge.confirm(window_id, confirm_id) {
            Ok(ticket) => ticket,
            Err(error) => return submit_error_reply(request_id, error),
        };
        // 确认流程等待用户决定；超时上限与命令响应一致，AI 可重查确认状态。
        match ticket.recv_timeout(AGENT_COMMAND_RESPONSE_TIMEOUT) {
            Ok(Ok(AgentCommandResponse::Performed {
                window_id,
                generation,
                revision,
                presented_revision,
                settled,
            })) => success_reply(
                request_id,
                "confirm",
                json!({
                    "window_id": window_id.raw(),
                    "generation": generation,
                    "revision": revision,
                    "presented_revision": presented_revision,
                    "settled": settled,
                }),
            ),
            Ok(Ok(AgentCommandResponse::Snapshot(_))) => error_reply(
                Some(request_id),
                AgentErrorCode::Internal,
                "unexpected command response",
                false,
            ),
            Ok(Err(error)) => command_error_reply(request_id, error),
            Err(RecvTimeoutError::Timeout) => command_timeout_reply(
                request_id,
                &ticket,
                "confirmation timed out before reaching the UI and was cancelled",
                "confirmation timed out after reaching the UI; read current state before retrying",
            ),
            Err(RecvTimeoutError::Disconnected) => error_reply(
                Some(request_id),
                AgentErrorCode::AppClosed,
                "application closed before responding",
                false,
            ),
        }
    }

    fn handle_wait(&self, object: &Map<String, Value>, request_id: String) -> AgentProtocolReply {
        // 解析窗口身份与超时参数。
        let window_id = match parse_window_id(object) {
            Ok(window_id) => window_id,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        let generation = match required_u64(object, "generation") {
            Ok(generation) => generation,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        let timeout_ms = match required_u64(object, "timeout_ms") {
            Ok(timeout_ms) => timeout_ms,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        // 等待条件二选一：修订号越过阈值或呈现修订号达到阈值。
        let after_revision = match optional_u64(object, "after_revision") {
            Ok(revision) => revision,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        let presented_revision = match optional_u64(object, "presented_revision") {
            Ok(revision) => revision,
            Err(error) => return error.into_reply(Some(request_id), false),
        };
        let condition = match (after_revision, presented_revision) {
            (Some(revision), None) => AgentWaitCondition::RevisionAfter(revision),
            (None, Some(revision)) => AgentWaitCondition::PresentedAtLeast(revision),
            // 两个条件同时出现或都缺失都是非法请求。
            _ => {
                return error_reply(
                    Some(request_id),
                    AgentErrorCode::InvalidRequest,
                    "wait requires exactly one revision condition",
                    false,
                );
            }
        };

        match self.bridge.wait(
            window_id,
            generation,
            condition,
            Duration::from_millis(timeout_ms),
        ) {
            // 按实际满足的等待结果类型回传对应状态。
            Ok(AgentWaitOutcome::Changed(window)) => wait_success(request_id, "changed", &window),
            Ok(AgentWaitOutcome::Presented(window)) => {
                wait_success(request_id, "presented", &window)
            }
            Ok(AgentWaitOutcome::Closed(window)) => wait_success(request_id, "closed", &window),
            Err(error) => wait_error_reply(request_id, error),
        }
    }
}

/// 区分安全取消与已开始后的未知结果，避免 AI 把迟到动作当成普通可重试超时。
fn command_timeout_reply(
    request_id: String,
    ticket: &AgentCommandTicket,
    cancelled_message: &'static str,
    outcome_unknown_message: &'static str,
) -> AgentProtocolReply {
    if ticket.cancel_pending() {
        error_reply(
            Some(request_id),
            AgentErrorCode::Timeout,
            cancelled_message,
            false,
        )
    } else {
        error_reply(
            Some(request_id),
            AgentErrorCode::OutcomeUnknown,
            outcome_unknown_message,
            false,
        )
    }
}

mod wire;

pub(crate) use self::wire::encode_session_token;
use self::wire::*;

#[cfg(test)]
mod capability_tests {
    use super::*;

    #[test]
    fn hello_catalog_advertises_every_parsed_window_action() {
        let actions = [
            json!({ "kind": "resize_window", "width": 800, "height": 600 }),
            json!({ "kind": "move_window", "x": 20, "y": 30 }),
            json!({ "kind": "maximize_window" }),
            json!({ "kind": "minimize_window" }),
            json!({ "kind": "restore_window" }),
            json!({ "kind": "close_window" }),
        ];
        for action in actions {
            let kind = action["kind"]
                .as_str()
                .unwrap_or_else(|| unreachable!("fixture action kind must be a string"));
            assert!(AGENT_WINDOW_ACTIONS.contains(&kind));
            assert!(matches!(
                parse_action(&action),
                Ok(ParsedAgentAction::Window(_))
            ));
        }
    }

    #[test]
    fn hello_catalog_freezes_window_state_field_names() {
        assert_eq!(
            AGENT_WINDOW_STATE_FIELDS,
            [
                "logical_width",
                "logical_height",
                "maximized",
                "minimized",
                "fullscreen",
                "focused",
            ]
        );
    }

    #[test]
    fn hello_catalog_freezes_request_types_and_screenshot_capability() {
        assert_eq!(
            AGENT_REQUEST_TYPES,
            [
                "hello",
                "list_windows",
                "snapshot",
                "screenshot",
                "perform",
                "confirm",
                "wait",
            ]
        );
    }
}

// 协议线解析与帧往返专项测试（仅 agent-control 能力下编译）。
