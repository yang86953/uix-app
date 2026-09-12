//! 协议报文解析辅助。

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WireError {
    code: AgentErrorCode,
    message: &'static str,
}

impl WireError {
    pub(super) const fn invalid(message: &'static str) -> Self {
        Self {
            code: AgentErrorCode::InvalidRequest,
            message,
        }
    }

    pub(super) fn into_reply(self, request_id: Option<String>, close: bool) -> AgentProtocolReply {
        error_reply(request_id, self.code, self.message, close)
    }
}

pub(super) fn request_id(object: &Map<String, Value>) -> Result<String, WireError> {
    let request_id = required_string(object, "request_id", MAX_REQUEST_ID_BYTES)?;
    if request_id.is_empty() {
        return Err(WireError::invalid("request_id must not be empty"));
    }
    Ok(request_id.to_owned())
}

pub(super) fn required_string<'a>(
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

pub(super) fn required_u64(object: &Map<String, Value>, field: &str) -> Result<u64, WireError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| WireError::invalid("required unsigned integer field is missing or invalid"))
}

pub(super) fn optional_u64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<u64>, WireError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| WireError::invalid("optional revision field must be unsigned")),
    }
}

pub(super) fn parse_window_id(object: &Map<String, Value>) -> Result<WindowId, WireError> {
    Ok(WindowId::new(required_u64(object, "window_id")?))
}

pub(super) fn parse_target(value: &Value) -> Result<SemanticTarget, WireError> {
    let object = value
        .as_object()
        .ok_or_else(|| WireError::invalid("target must be an object"))?;
    let automation_id = object.get("automation_id").and_then(Value::as_str);
    let node_id = object.get("node_id").and_then(Value::as_str);
    match (automation_id, node_id) {
        (Some(id), None) if !id.is_empty() && id.len() <= MAX_AUTOMATION_ID_BYTES => {
            Ok(SemanticTarget::AutomationId(id.to_owned()))
        }
        (None, Some(id)) => parse_widget_id(id).map(SemanticTarget::NodeId),
        _ => Err(WireError::invalid(
            "target requires exactly one valid automation_id or node_id",
        )),
    }
}

pub(super) fn parse_widget_id(value: &str) -> Result<WidgetId, WireError> {
    let parts = value.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [slot, generation] => Ok(WidgetId::from_parts(
            slot.parse()
                .map_err(|_| WireError::invalid("node_id slot is invalid"))?,
            generation
                .parse()
                .map_err(|_| WireError::invalid("node_id generation is invalid"))?,
        )),
        [tree_scope, slot, generation] => Ok(WidgetId::from_scoped_parts(
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
pub(super) enum ParsedAgentAction {
    Semantic(SemanticAction),
    Window(AgentWindowAction),
}

pub(super) fn parse_action(value: &Value) -> Result<ParsedAgentAction, WireError> {
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
        // 连续值调整声明（E-05）：值域 min/max 由语义快照 value_min/value_max
        // 提供，协议字段为可选副本供对端回显，缺省 0.0 不影响执行。
        "adjust" => {
            let min = optional_f32(object, "min")?.unwrap_or(0.0) as f64;
            let max = optional_f32(object, "max")?.unwrap_or(0.0) as f64;
            SemanticAction::Adjust { min, max }
        }
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
            }));
        }
        "click_at" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::ClickAt {
                position: Point::new(required_f32(object, "x")?, required_f32(object, "y")?),
            }));
        }
        "pointer_move" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::PointerMove {
                position: Point::new(required_f32(object, "x")?, required_f32(object, "y")?),
            }));
        }
        "pointer_down" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::PointerDown {
                position: Point::new(required_f32(object, "x")?, required_f32(object, "y")?),
            }));
        }
        "pointer_up" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::PointerUp {
                position: Point::new(required_f32(object, "x")?, required_f32(object, "y")?),
            }));
        }
        "resize_window" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::Resize {
                width: required_i32(object, "width")?,
                height: required_i32(object, "height")?,
            }));
        }
        "move_window" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::Move {
                x: required_i32(object, "x")?,
                y: required_i32(object, "y")?,
            }));
        }
        "maximize_window" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::Maximize));
        }
        "minimize_window" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::Minimize));
        }
        "restore_window" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::Restore));
        }
        "activate_window" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::Activate));
        }
        "close_window" => {
            return Ok(ParsedAgentAction::Window(AgentWindowAction::Close));
        }
        _ => return Err(WireError::invalid("unknown action kind")),
    };
    Ok(ParsedAgentAction::Semantic(semantic))
}

pub(super) fn parse_key_code(object: &Map<String, Value>) -> Result<KeyCode, WireError> {
    let name = required_string(object, "key", 32)?;
    AGENT_KEY_CODES
        .iter()
        .find_map(|(candidate, key)| (*candidate == name).then_some(*key))
        .ok_or_else(|| WireError::invalid("unknown key name"))
}

pub(super) fn parse_key_modifiers(object: &Map<String, Value>) -> Result<KeyMod, WireError> {
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

pub(super) fn action_text(object: &Map<String, Value>, field: &str) -> Result<String, WireError> {
    let value = required_string(object, field, MAX_AGENT_TEXT_BYTES)?;
    Ok(value.to_owned())
}

pub(super) fn required_f32(object: &Map<String, Value>, field: &str) -> Result<f32, WireError> {
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

pub(super) fn required_i32(object: &Map<String, Value>, field: &str) -> Result<i32, WireError> {
    let value = object
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| WireError::invalid("required integer field is missing or invalid"))?;
    i32::try_from(value).map_err(|_| WireError::invalid("integer field is outside the i32 range"))
}

pub(super) fn optional_f32(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<f32>, WireError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let number = value.as_f64().ok_or_else(|| {
                WireError::invalid("optional numeric field is missing or invalid")
            })?;
            if !number.is_finite() || number < f32::MIN as f64 || number > f32::MAX as f64 {
                return Err(WireError::invalid(
                    "numeric field is outside the supported range",
                ));
            }
            Ok(Some(number as f32))
        }
    }
}

pub(super) fn success_reply(
    request_id: String,
    request_type: &'static str,
    payload: Value,
) -> AgentProtocolReply {
    success_reply_with_close(request_id, request_type, payload, false)
}

fn success_reply_with_close(
    request_id: String,
    request_type: &'static str,
    payload: Value,
    close_connection: bool,
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
    AgentProtocolReply::from_value(Value::Object(object), close_connection, "ok")
}

pub(super) fn error_reply(
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

pub(super) fn submit_error_reply(
    request_id: String,
    error: AgentSubmitError,
) -> AgentProtocolReply {
    let message = match &error {
        AgentSubmitError::WindowNotFound => "window was not found",
        AgentSubmitError::QueueFull => "window command queue is full",
        AgentSubmitError::AppClosed => "application is closed",
        AgentSubmitError::ReadbackUnavailable { .. } => {
            "surface readback could not be scheduled for this window"
        }
    };
    error_reply(Some(request_id), error.code(), message, false)
}

pub(super) fn command_error_reply(
    request_id: String,
    error: AgentCommandError,
) -> AgentProtocolReply {
    let message = match &error {
        AgentCommandError::StaleWindow { .. } => "window generation is stale",
        AgentCommandError::StaleRevision { .. } => "window revision is stale",
        AgentCommandError::NodeNotFound(_) => "target node was not found",
        AgentCommandError::AmbiguousTarget { .. } => "target matched multiple nodes",
        AgentCommandError::UnsupportedAction { .. } => "target does not support the action",
        AgentCommandError::Forbidden { .. } => "action is forbidden by the agent policy",
        AgentCommandError::RequiresConfirmation { .. } => "action requires user confirmation",
        AgentCommandError::ConfirmationRejected { .. } => "confirmation was rejected",
        AgentCommandError::ConfirmationNotFound { .. } => "confirmation is unknown or expired",
        AgentCommandError::InvalidValue { .. } => "action value is invalid for the target",
        AgentCommandError::NotInteractable(_) => "action target is not interactable",
        AgentCommandError::Blocked { .. } => "target is blocked",
        AgentCommandError::DidNotSettle { .. } => "UI did not settle within its pass limit",
        AgentCommandError::WindowOperationFailed { .. } => "window operation failed",
        AgentCommandError::NotPresentable => "window is not presentable",
        AgentCommandError::AppClosed => "application is closed",
        AgentCommandError::Internal => "internal command failure",
    };
    // 需要确认的错误携带一次性 confirm_id，供后续 confirm 请求使用。
    if let AgentCommandError::RequiresConfirmation {
        target,
        action,
        confirm_id,
    } = &error
    {
        return AgentProtocolReply::from_value(
            json!({
                "schema": AGENT_PROTOCOL_SCHEMA,
                "request_id": request_id,
                "ok": false,
                "error": {
                    "code": error.code().as_str(),
                    "message": message,
                    "confirm_id": confirm_id,
                    "target": target,
                    "action": action.as_str(),
                }
            }),
            false,
            error.code().as_str(),
        );
    }
    error_reply(Some(request_id), error.code(), message, false)
}

pub(super) fn wait_error_reply(request_id: String, error: AgentWaitError) -> AgentProtocolReply {
    let message = match &error {
        AgentWaitError::InvalidTimeout { .. } => "wait timeout exceeds its limit",
        AgentWaitError::WindowNotFound => "window was not found",
        AgentWaitError::StaleWindow { .. } => "window generation is stale",
        AgentWaitError::Timeout => "wait timed out",
        AgentWaitError::AppClosed => "application is closed",
    };
    error_reply(Some(request_id), error.code(), message, false)
}

pub(super) fn wait_success(
    request_id: String,
    outcome: &'static str,
    window: &AgentWindowInfo,
) -> AgentProtocolReply {
    success_reply_with_close(
        request_id,
        "wait",
        json!({
            "outcome": outcome,
            "window": window_info_value(window),
        }),
        // closed 是该 generation 的终态；写回后主动结束连接，让应用关闭可等待传输排空。
        outcome == "closed",
    )
}

pub(super) fn window_info_value(window: &AgentWindowInfo) -> Value {
    json!({
        "window_id": window.window_id.raw(),
        "generation": window.generation,
        "title": window.title,
        "visible": window.visible,
        "presentable": window.presentable,
        "focused": window.focused,
        "logical_width": window.logical_width,
        "logical_height": window.logical_height,
        "maximized": window.maximized,
        "minimized": window.minimized,
        "fullscreen": window.fullscreen,
        "revision": window.revision,
        "presented_revision": window.presented_revision,
        "closed": window.closed,
    })
}

pub(super) fn semantic_snapshot_value(snapshot: &WindowSemanticSnapshot) -> Value {
    json!({
        "window_id": snapshot.window_id.raw(),
        "generation": snapshot.generation,
        "revision": snapshot.revision,
        "presented_revision": snapshot.presented_revision,
        "closed": snapshot.closed,
        // bounds 坐标空间事实：frame/visible_bounds 是 logical 客户区坐标，
        // 截屏像素为 physical；跨空间对照必须除以 device_pixel_ratio。
        "device_pixel_ratio": finite_number(snapshot.device_pixel_ratio as f64),
        "nodes": snapshot.nodes.iter().map(semantic_node_value).collect::<Vec<_>>(),
    })
}

pub(super) fn semantic_node_value(node: &SemanticNode) -> Value {
    let state = &node.accessibility.state;
    json!({
        "node_id": node.id.to_string(),
        "automation_id": node.automation_id,
        "parent": node.parent.map(|parent| parent.to_string()),
        "frame": rect_value(node.frame),
        "visible_bounds": node.visible_bounds.map(rect_value),
        "focused": node.focused,
        "hovered": node.hovered,
        "role": node.accessibility.role.automation_name(),
        "name": node.accessibility.name,
        "state": accessibility_state_value(state),
        "selection": node.selection.as_ref().map(selection_value),
        "actions": node.actions.iter().map(|action| action.as_str()).collect::<Vec<_>>(),
    })
}

pub(super) fn selection_value(selection: &crate::ui::SelectionSnapshot) -> Value {
    json!({
        "options": selection.options,
        "selected_indices": selection.selected_indices,
        "disabled_indices": selection.disabled_indices,
        "multiple": selection.multiple,
        "expanded": selection.expanded,
    })
}

pub(super) fn rect_value(rect: Rect) -> Value {
    json!({
        "x": finite_number(rect.x as f64),
        "y": finite_number(rect.y as f64),
        "w": finite_number(rect.w as f64),
        "h": finite_number(rect.h as f64),
    })
}

pub(super) fn accessibility_state_value(state: &AccessibilityState) -> Value {
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

pub(super) fn finite_number(value: f64) -> Value {
    finite_number_option(value).map_or(Value::Null, Value::Number)
}

pub(super) fn finite_number_option(value: f64) -> Option<serde_json::Number> {
    serde_json::Number::from_f64(value)
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

pub(super) fn token_matches(expected: &[u8; 32], candidate: &str) -> bool {
    let mut decoded = [0u8; 32];
    let valid = decode_token(candidate, &mut decoded);
    let mut difference = u8::from(!valid);
    for (expected, actual) in expected.iter().zip(decoded.iter()) {
        difference |= expected ^ actual;
    }
    difference == 0
}

pub(super) fn decode_token(candidate: &str, output: &mut [u8; 32]) -> bool {
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

#[cfg(test)]
#[path = "../../../../tests-src/app/agent/agent_protocol/wire_tests.rs"]
mod tests;

