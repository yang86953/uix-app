//! Authenticated, bounded JSON Lines protocol for the process Agent Bridge.
//!
//! Native transports provide a private byte stream. This module owns framing
//! semantics and wire validation, but never touches a platform handle or a
//! `WidgetTree` directly.

use std::sync::mpsc::RecvTimeoutError;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use crate::app::agent_bridge::{
    AgentProcessBridge, AgentWaitCondition, AgentWaitError, AgentWaitOutcome, AgentWindowInfo,
    MAX_AGENT_WAIT_TIMEOUT,
};
use crate::app::agent_control::{
    AgentCommandError, AgentCommandResponse, AgentErrorCode, AgentSubmitError, AgentWindowAction,
    DEFAULT_AGENT_COMMAND_QUEUE_CAPACITY, MAX_AGENT_SETTLE_PASSES,
};
use crate::app::window_semantics::WindowSemanticSnapshot;
use crate::core::{ComponentId, Point, Rect, WindowId};
use crate::ui::component_snapshot::{AccessibilityRole, AccessibilityState};
use crate::ui::semantic_action::SemanticAction;
use crate::ui::semantic_snapshot::{SemanticNode, SemanticTarget};
use crate::ui::{KeyCode, KeyMod};

pub(crate) const AGENT_PROTOCOL_SCHEMA: &str = "uix.agent.v1";
pub(crate) const MAX_AGENT_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_AGENT_TEXT_BYTES: usize = 64 * 1024;
pub(crate) const MAX_AGENT_CONNECTIONS: usize = 8;
const MAX_REQUEST_ID_BYTES: usize = 128;
const MAX_AUTOMATION_ID_BYTES: usize = 512;
const AGENT_COMMAND_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
const AGENT_REQUEST_TYPES: &[&str] = &["hello", "list_windows", "snapshot", "perform", "wait"];
const AGENT_SEMANTIC_ACTIONS: &[&str] = &[
    "invoke",
    "focus",
    "set_value",
    "insert_text",
    "select",
    "toggle",
    "increment",
    "decrement",
    "scroll",
];
const AGENT_WINDOW_ACTIONS: &[&str] = &["press_key", "click_at"];
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
        crate::core::log::info_fn(format!(
            "agent request result={} duration_ms={}",
            reply.result_code(),
            started.elapsed().as_millis()
        ));
        reply
    }

    fn handle_line_inner(&mut self, line: &[u8]) -> AgentProtocolReply {
        if line.len() > MAX_AGENT_MESSAGE_BYTES {
            return error_reply(
                None,
                AgentErrorCode::InvalidRequest,
                "message exceeds the protocol limit",
                true,
            );
        }

        let value: Value = match serde_json::from_slice(line) {
            Ok(value) => value,
            Err(_) => {
                return error_reply(
                    None,
                    AgentErrorCode::InvalidRequest,
                    "message is not valid JSON",
                    !self.authenticated,
                )
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

        if !self.authenticated {
            return self.handle_hello(object, request_id, request_type);
        }

        match request_type {
            "hello" => error_reply(
                Some(request_id),
                AgentErrorCode::InvalidRequest,
                "connection is already authenticated",
                false,
            ),
            "list_windows" => self.handle_list_windows(request_id),
            "snapshot" => self.handle_snapshot(object, request_id),
            "perform" => self.handle_perform(object, request_id),
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
        if request_type != "hello" {
            return error_reply(
                Some(request_id),
                AgentErrorCode::Unauthorized,
                "hello must be the first request",
                true,
            );
        }
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
        success_reply(
            request_id,
            "hello",
            json!({
                "process_id": std::process::id(),
                "capabilities": {
                    "request_types": AGENT_REQUEST_TYPES,
                    "semantic_actions": AGENT_SEMANTIC_ACTIONS,
                    "window_actions": AGENT_WINDOW_ACTIONS,
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
                "UI command timed out",
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

    fn handle_perform(
        &self,
        object: &Map<String, Value>,
        request_id: String,
    ) -> AgentProtocolReply {
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
            Ok(Ok(AgentCommandResponse::Snapshot(_))) => error_reply(
                Some(request_id),
                AgentErrorCode::Internal,
                "unexpected command response",
                false,
            ),
            Ok(Err(error)) => command_error_reply(request_id, error),
            Err(RecvTimeoutError::Timeout) => error_reply(
                Some(request_id),
                AgentErrorCode::Timeout,
                "UI command timed out",
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

    fn handle_wait(&self, object: &Map<String, Value>, request_id: String) -> AgentProtocolReply {
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
            _ => {
                return error_reply(
                    Some(request_id),
                    AgentErrorCode::InvalidRequest,
                    "wait requires exactly one revision condition",
                    false,
                )
            }
        };

        match self.bridge.wait(
            window_id,
            generation,
            condition,
            Duration::from_millis(timeout_ms),
        ) {
            Ok(AgentWaitOutcome::Changed(window)) => wait_success(request_id, "changed", &window),
            Ok(AgentWaitOutcome::Presented(window)) => {
                wait_success(request_id, "presented", &window)
            }
            Ok(AgentWaitOutcome::Closed(window)) => wait_success(request_id, "closed", &window),
            Err(error) => wait_error_reply(request_id, error),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct WireError {
    code: AgentErrorCode,
    message: &'static str,
}

impl WireError {
    const fn invalid(message: &'static str) -> Self {
        Self {
            code: AgentErrorCode::InvalidRequest,
            message,
        }
    }

    fn into_reply(self, request_id: Option<String>, close: bool) -> AgentProtocolReply {
        error_reply(request_id, self.code, self.message, close)
    }
}

fn request_id(object: &Map<String, Value>) -> Result<String, WireError> {
    let request_id = required_string(object, "request_id", MAX_REQUEST_ID_BYTES)?;
    if request_id.is_empty() {
        return Err(WireError::invalid("request_id must not be empty"));
    }
    Ok(request_id.to_owned())
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    max_bytes: usize,
) -> Result<&'a str, WireError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| WireError::invalid("required string field is missing or invalid"))?;
    if value.len() > max_bytes {
        return Err(WireError::invalid("string field exceeds its limit"));
    }
    Ok(value)
}

fn required_u64(object: &Map<String, Value>, field: &str) -> Result<u64, WireError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| WireError::invalid("required unsigned integer field is missing or invalid"))
}

fn optional_u64(object: &Map<String, Value>, field: &str) -> Result<Option<u64>, WireError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| WireError::invalid("optional revision field must be unsigned")),
    }
}

fn parse_window_id(object: &Map<String, Value>) -> Result<WindowId, WireError> {
    Ok(WindowId::new(required_u64(object, "window_id")?))
}

fn parse_target(value: &Value) -> Result<SemanticTarget, WireError> {
    let object = value
        .as_object()
        .ok_or_else(|| WireError::invalid("target must be an object"))?;
    let automation_id = object.get("automation_id").and_then(Value::as_str);
    let node_id = object.get("node_id").and_then(Value::as_str);
    match (automation_id, node_id) {
        (Some(id), None) if !id.is_empty() && id.len() <= MAX_AUTOMATION_ID_BYTES => {
            Ok(SemanticTarget::AutomationId(id.to_owned()))
        }
        (None, Some(id)) => parse_component_id(id).map(SemanticTarget::NodeId),
        _ => Err(WireError::invalid(
            "target requires exactly one valid automation_id or node_id",
        )),
    }
}

fn parse_component_id(value: &str) -> Result<ComponentId, WireError> {
    let parts = value.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [slot, generation] => Ok(ComponentId::from_parts(
            slot.parse()
                .map_err(|_| WireError::invalid("node_id slot is invalid"))?,
            generation
                .parse()
                .map_err(|_| WireError::invalid("node_id generation is invalid"))?,
        )),
        [tree_scope, slot, generation] => Ok(ComponentId::from_scoped_parts(
            tree_scope
                .parse()
                .map_err(|_| WireError::invalid("node_id tree scope is invalid"))?,
            slot.parse()
                .map_err(|_| WireError::invalid("node_id slot is invalid"))?,
            generation
                .parse()
                .map_err(|_| WireError::invalid("node_id generation is invalid"))?,
        )),
        _ => Err(WireError::invalid("node_id has an invalid format")),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ParsedAgentAction {
    Semantic(SemanticAction),
    Window(AgentWindowAction),
}

fn parse_action(value: &Value) -> Result<ParsedAgentAction, WireError> {
    let object = value
        .as_object()
        .ok_or_else(|| WireError::invalid("action must be an object"))?;
    let kind = required_string(object, "kind", 32)?;
    let semantic = match kind {
        "invoke" => SemanticAction::Invoke,
        "focus" => SemanticAction::Focus,
        "set_value" => SemanticAction::SetValue(action_text(object, "value")?),
        "insert_text" => SemanticAction::InsertText(action_text(object, "text")?),
        "select" => SemanticAction::Select(action_text(object, "value")?),
        "toggle" => SemanticAction::Toggle,
        "increment" => SemanticAction::Increment,
        "decrement" => SemanticAction::Decrement,
        "scroll" => {
            let x = required_f32(object, "delta_x")?;
            let y = required_f32(object, "delta_y")?;
            SemanticAction::Scroll {
                delta: Point::new(x, y),
            }
        }
        "press_key" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::PressKey {
                key: parse_key_code(object)?,
                modifiers: parse_key_modifiers(object)?,
            }))
        }
        "click_at" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::ClickAt {
                position: Point::new(required_f32(object, "x")?, required_f32(object, "y")?),
            }))
        }
        _ => return Err(WireError::invalid("unknown action kind")),
    };
    Ok(ParsedAgentAction::Semantic(semantic))
}

fn parse_key_code(object: &Map<String, Value>) -> Result<KeyCode, WireError> {
    let name = required_string(object, "key", 32)?;
    AGENT_KEY_CODES
        .iter()
        .find_map(|(candidate, key)| (*candidate == name).then_some(*key))
        .ok_or_else(|| WireError::invalid("unknown key name"))
}

fn parse_key_modifiers(object: &Map<String, Value>) -> Result<KeyMod, WireError> {
    let Some(value) = object.get("modifiers") else {
        return Ok(KeyMod::NONE);
    };
    let modifiers = value
        .as_array()
        .ok_or_else(|| WireError::invalid("modifiers must be an array"))?;
    if modifiers.len() > AGENT_KEY_MODIFIERS.len() {
        return Err(WireError::invalid("too many key modifiers"));
    }

    let mut result = KeyMod::NONE;
    for modifier in modifiers {
        let name = modifier
            .as_str()
            .ok_or_else(|| WireError::invalid("key modifier must be a string"))?;
        let flag = match name {
            "shift" => KeyMod::SHIFT,
            "ctrl" => KeyMod::CTRL,
            "alt" => KeyMod::ALT,
            "super" => KeyMod::SUPER,
            _ => return Err(WireError::invalid("unknown key modifier")),
        };
        if result.contains(flag) {
            return Err(WireError::invalid("key modifier must not be repeated"));
        }
        result |= flag;
    }
    Ok(result)
}

fn action_text(object: &Map<String, Value>, field: &str) -> Result<String, WireError> {
    let value = required_string(object, field, MAX_AGENT_TEXT_BYTES)?;
    Ok(value.to_owned())
}

fn required_f32(object: &Map<String, Value>, field: &str) -> Result<f32, WireError> {
    let value = object
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| WireError::invalid("required numeric field is missing or invalid"))?;
    if !value.is_finite() || value < f32::MIN as f64 || value > f32::MAX as f64 {
        return Err(WireError::invalid(
            "numeric field is outside the supported range",
        ));
    }
    Ok(value as f32)
}

fn success_reply(
    request_id: String,
    request_type: &'static str,
    payload: Value,
) -> AgentProtocolReply {
    let mut object = Map::new();
    object.insert(
        "schema".to_owned(),
        Value::String(AGENT_PROTOCOL_SCHEMA.to_owned()),
    );
    object.insert("request_id".to_owned(), Value::String(request_id));
    object.insert("ok".to_owned(), Value::Bool(true));
    object.insert("type".to_owned(), Value::String(request_type.to_owned()));
    if let Value::Object(payload) = payload {
        object.extend(payload);
    }
    AgentProtocolReply::from_value(Value::Object(object), false, "ok")
}

fn error_reply(
    request_id: Option<String>,
    code: AgentErrorCode,
    message: &'static str,
    close_connection: bool,
) -> AgentProtocolReply {
    AgentProtocolReply::from_value(
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": request_id,
            "ok": false,
            "error": {
                "code": code.as_str(),
                "message": message,
            }
        }),
        close_connection,
        code.as_str(),
    )
}

fn submit_error_reply(request_id: String, error: AgentSubmitError) -> AgentProtocolReply {
    let message = match error {
        AgentSubmitError::WindowNotFound => "window was not found",
        AgentSubmitError::QueueFull => "window command queue is full",
        AgentSubmitError::AppClosed => "application is closed",
    };
    error_reply(Some(request_id), error.code(), message, false)
}

fn command_error_reply(request_id: String, error: AgentCommandError) -> AgentProtocolReply {
    let message = match &error {
        AgentCommandError::StaleWindow { .. } => "window generation is stale",
        AgentCommandError::StaleRevision { .. } => "window revision is stale",
        AgentCommandError::NodeNotFound(_) => "target node was not found",
        AgentCommandError::AmbiguousTarget { .. } => "target matched multiple nodes",
        AgentCommandError::UnsupportedAction { .. } => "target does not support the action",
        AgentCommandError::InvalidValue { .. } => "action value is invalid for the target",
        AgentCommandError::NotInteractable(_) => "action target is not interactable",
        AgentCommandError::Blocked { .. } => "target is blocked",
        AgentCommandError::DidNotSettle { .. } => "UI did not settle within its pass limit",
        AgentCommandError::NotPresentable => "window is not presentable",
        AgentCommandError::AppClosed => "application is closed",
        AgentCommandError::Internal => "internal command failure",
    };
    error_reply(Some(request_id), error.code(), message, false)
}

fn wait_error_reply(request_id: String, error: AgentWaitError) -> AgentProtocolReply {
    let message = match &error {
        AgentWaitError::InvalidTimeout { .. } => "wait timeout exceeds its limit",
        AgentWaitError::WindowNotFound => "window was not found",
        AgentWaitError::StaleWindow { .. } => "window generation is stale",
        AgentWaitError::Timeout => "wait timed out",
        AgentWaitError::AppClosed => "application is closed",
    };
    error_reply(Some(request_id), error.code(), message, false)
}

fn wait_success(
    request_id: String,
    outcome: &'static str,
    window: &AgentWindowInfo,
) -> AgentProtocolReply {
    success_reply(
        request_id,
        "wait",
        json!({
            "outcome": outcome,
            "window": window_info_value(window),
        }),
    )
}

fn window_info_value(window: &AgentWindowInfo) -> Value {
    json!({
        "window_id": window.window_id.raw(),
        "generation": window.generation,
        "title": window.title,
        "visible": window.visible,
        "presentable": window.presentable,
        "revision": window.revision,
        "presented_revision": window.presented_revision,
        "closed": window.closed,
    })
}

fn semantic_snapshot_value(snapshot: &WindowSemanticSnapshot) -> Value {
    json!({
        "window_id": snapshot.window_id.raw(),
        "generation": snapshot.generation,
        "revision": snapshot.revision,
        "presented_revision": snapshot.presented_revision,
        "closed": snapshot.closed,
        "nodes": snapshot.nodes.iter().map(semantic_node_value).collect::<Vec<_>>(),
    })
}

fn semantic_node_value(node: &SemanticNode) -> Value {
    let state = &node.accessibility.state;
    json!({
        "node_id": node.id.to_string(),
        "automation_id": node.automation_id,
        "parent": node.parent.map(|parent| parent.to_string()),
        "frame": rect_value(node.frame),
        "visible_bounds": node.visible_bounds.map(rect_value),
        "focused": node.focused,
        "role": role_name(node.accessibility.role),
        "name": node.accessibility.name,
        "state": accessibility_state_value(state),
        "selection": node.selection.as_ref().map(selection_value),
        "actions": node.actions.iter().map(|action| action.as_str()).collect::<Vec<_>>(),
    })
}

fn selection_value(selection: &crate::ui::SelectionSnapshot) -> Value {
    json!({
        "options": selection.options,
        "selected_indices": selection.selected_indices,
        "disabled_indices": selection.disabled_indices,
        "multiple": selection.multiple,
        "expanded": selection.expanded,
    })
}

fn rect_value(rect: Rect) -> Value {
    json!({
        "x": finite_number(rect.x as f64),
        "y": finite_number(rect.y as f64),
        "w": finite_number(rect.w as f64),
        "h": finite_number(rect.h as f64),
    })
}

fn accessibility_state_value(state: &AccessibilityState) -> Value {
    json!({
        "disabled": state.disabled,
        "checked": state.checked,
        "expanded": state.expanded,
        "selected": state.selected,
        "value_text": if state.password { None } else { state.value_text.as_deref() },
        "value_now": state.value_now.and_then(finite_number_option),
        "value_min": state.value_min.and_then(finite_number_option),
        "value_max": state.value_max.and_then(finite_number_option),
        "multiline": state.multiline,
        "password": state.password,
        "required": state.required,
    })
}

fn finite_number(value: f64) -> Value {
    finite_number_option(value).map_or(Value::Null, Value::Number)
}

fn finite_number_option(value: f64) -> Option<serde_json::Number> {
    serde_json::Number::from_f64(value)
}

fn role_name(role: AccessibilityRole) -> &'static str {
    match role {
        AccessibilityRole::None => "none",
        AccessibilityRole::Generic => "generic",
        AccessibilityRole::Alert => "alert",
        AccessibilityRole::Button => "button",
        AccessibilityRole::Checkbox => "checkbox",
        AccessibilityRole::Combobox => "combobox",
        AccessibilityRole::Dialog => "dialog",
        AccessibilityRole::Group => "group",
        AccessibilityRole::Image => "image",
        AccessibilityRole::List => "list",
        AccessibilityRole::Menu => "menu",
        AccessibilityRole::Navigation => "navigation",
        AccessibilityRole::ProgressBar => "progress_bar",
        AccessibilityRole::RadioGroup => "radio_group",
        AccessibilityRole::Slider => "slider",
        AccessibilityRole::SpinButton => "spin_button",
        AccessibilityRole::Status => "status",
        AccessibilityRole::Switch => "switch",
        AccessibilityRole::Table => "table",
        AccessibilityRole::TabList => "tab_list",
        AccessibilityRole::Text => "text",
        AccessibilityRole::TextBox => "text_box",
        AccessibilityRole::Tree => "tree",
    }
}

pub(crate) fn encode_session_token(token: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in token {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn token_matches(expected: &[u8; 32], candidate: &str) -> bool {
    let mut decoded = [0u8; 32];
    let valid = decode_token(candidate, &mut decoded);
    let mut difference = u8::from(!valid);
    for (expected, actual) in expected.iter().zip(decoded.iter()) {
        difference |= expected ^ actual;
    }
    difference == 0
}

fn decode_token(candidate: &str, output: &mut [u8; 32]) -> bool {
    if candidate.len() != 64 || !candidate.is_ascii() {
        return false;
    }
    for (index, pair) in candidate.as_bytes().chunks_exact(2).enumerate() {
        let Some(high) = hex_nibble(pair[0]) else {
            return false;
        };
        let Some(low) = hex_nibble(pair[1]) else {
            return false;
        };
        output[index] = (high << 4) | low;
    }
    true
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
