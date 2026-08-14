// Agent 线协议解析与帧往返专项测试。
// 复用父模块的 wire 解析器与协议会话，全部为纯函数/内存内会话，不启动线程与 IO。

// 引入被测的 wire 解析函数（parse_action 等，同为 agent_protocol 子模块）。
use super::wire::*;
// 引入协议会话、桥与运行时类型。
use super::*;
// 构造协议请求 JSON 载荷。
use serde_json::json;

/// 把测试载荷 Value 断言为对象后借用其字段映射（wire 解析函数要求 &Map）。
fn as_map(value: &serde_json::Value) -> &serde_json::Map<String, serde_json::Value> {
    value.as_object().expect("测试载荷必须是 JSON 对象")
}

/// 构造一个持有指定会话 token 的协议会话（桥使用空运行时，不触碰窗口）。
fn session_with_token(token: [u8; 32]) -> AgentProtocolSession {
    AgentProtocolSession::new(
        AgentProcessBridge::new(crate::app::session_runtime::AppRuntime::new()),
        Arc::new(token),
    )
}

/// 把协议回复帧字节解析为 JSON 供断言。
fn reply_json(reply: &AgentProtocolReply) -> serde_json::Value {
    serde_json::from_slice(reply.bytes()).expect("回复帧必须是合法 JSON")
}

/// 请求帧往返：构造 JSON Lines 请求，经会话处理后断言回复帧结构与状态码。
fn round_trip(
    session: &mut AgentProtocolSession,
    request: serde_json::Value,
) -> (serde_json::Value, &'static str, bool) {
    let line = serde_json::to_vec(&request).expect("请求必须可序列化");
    let reply = session.handle_line(&line);
    (reply_json(&reply), reply.result_code(), reply.close_connection())
}

/// 认证辅助：用给定 token 完成 hello，返回回复 JSON。
fn authenticate(session: &mut AgentProtocolSession, token: [u8; 32]) -> serde_json::Value {
    let (json, code, close) = round_trip(
        session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r1",
            "type": "hello",
            "token": encode_session_token(&token),
        }),
    );
    assert_eq!(code, "ok", "hello 必须认证成功");
    assert!(!close, "认证成功后不得关闭连接");
    json
}

// ── parse_action：语义动作 ────────────────────────────────────────────────

// 无参语义动作必须映射到对应变体。
#[test]
fn parse_action_maps_parameterless_semantic_kinds() {
    // 逐个验证无参数动作的映射。
    for (kind, expected) in [
        ("invoke", SemanticAction::Invoke),
        ("focus", SemanticAction::Focus),
        ("toggle", SemanticAction::Toggle),
        ("increment", SemanticAction::Increment),
        ("decrement", SemanticAction::Decrement),
    ] {
        // 构造只带动作类型的对象。
        let action = parse_action(&json!({ "kind": kind }));
        // 断言映射到预期语义动作。
        assert_eq!(
            action,
            Ok(ParsedAgentAction::Semantic(expected)),
            "kind={kind} 映射错误"
        );
    }
}

// 带文本参数的语义动作必须读取对应字段。
#[test]
fn parse_action_maps_text_semantic_kinds() {
    // set_value 读取 value 字段。
    let action = parse_action(&json!({ "kind": "set_value", "value": "你好" }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Semantic(SemanticAction::SetValue(
            "你好".to_owned()
        )))
    );

    // insert_text 读取 text 字段。
    let action = parse_action(&json!({ "kind": "insert_text", "text": "abc" }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Semantic(SemanticAction::InsertText(
            "abc".to_owned()
        )))
    );

    // select 读取 value 字段。
    let action = parse_action(&json!({ "kind": "select", "value": "b" }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Semantic(SemanticAction::Select(
            "b".to_owned()
        )))
    );
}

// 缺失文本字段的动作必须报 invalid_request。
#[test]
fn parse_action_rejects_missing_text_fields() {
    // set_value 缺 value 属于缺失必填字符串。
    let action = parse_action(&json!({ "kind": "set_value" }));
    assert!(action.is_err(), "缺 value 的 set_value 必须被拒绝");
    // insert_text 缺 text 属于缺失必填字符串。
    let action = parse_action(&json!({ "kind": "insert_text" }));
    assert!(action.is_err(), "缺 text 的 insert_text 必须被拒绝");
}

// adjust（E-05 连续值调整）必须读取可选 min/max，缺省为 0.0。
#[test]
fn parse_action_adjust_uses_optional_min_max_with_zero_default() {
    // 不提供 min/max 时按协议回退为 0.0。
    let action = parse_action(&json!({ "kind": "adjust" }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Semantic(SemanticAction::Adjust {
            min: 0.0,
            max: 0.0,
        }))
    );

    // 提供 min/max 时原样回显到动作载荷。
    let action = parse_action(&json!({ "kind": "adjust", "min": 1.5, "max": 88.25 }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Semantic(SemanticAction::Adjust {
            min: 1.5,
            max: 88.25,
        }))
    );

    // 只提供 max 时 min 保持缺省 0.0。
    let action = parse_action(&json!({ "kind": "adjust", "max": 10.0 }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Semantic(SemanticAction::Adjust {
            min: 0.0,
            max: 10.0,
        }))
    );

    // min/max 为 null 时同样按缺省 0.0 处理。
    let action = parse_action(&json!({ "kind": "adjust", "min": null, "max": null }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Semantic(SemanticAction::Adjust {
            min: 0.0,
            max: 0.0,
        }))
    );

    // min/max 超出 f32 范围必须被拒绝。
    let action = parse_action(&json!({ "kind": "adjust", "min": 1e300 }));
    assert!(action.is_err(), "超出 f32 范围的 min 必须被拒绝");
}

// scroll 必须读取 delta_x/delta_y 并组成 Point。
#[test]
fn parse_action_scroll_builds_point_delta() {
    // 正负混合的增量必须完整保留。
    let action = parse_action(&json!({ "kind": "scroll", "delta_x": 1.5, "delta_y": -2.0 }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Semantic(SemanticAction::Scroll {
            delta: crate::core::Point::new(1.5, -2.0),
        }))
    );
    // 缺 delta_x 属于缺失必填数字。
    let action = parse_action(&json!({ "kind": "scroll", "delta_y": 1.0 }));
    assert!(action.is_err(), "缺 delta_x 的 scroll 必须被拒绝");
}

// ── parse_action：窗口动作 ────────────────────────────────────────────────

// press_key 必须解析键名与修饰键并映射为窗口动作。
#[test]
fn parse_action_press_key_parses_key_and_modifiers() {
    // 组合修饰键与字母键。
    let action = parse_action(&json!({
        "kind": "press_key",
        "key": "a",
        "modifiers": ["ctrl", "shift"],
    }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Window(AgentWindowAction::PressKey {
            key: KeyCode::A,
            modifiers: KeyMod::CTRL | KeyMod::SHIFT,
        }))
    );

    // 不提供 modifiers 时按无修饰键处理。
    let action = parse_action(&json!({ "kind": "press_key", "key": "enter" }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Window(AgentWindowAction::PressKey {
            key: KeyCode::Enter,
            modifiers: KeyMod::NONE,
        }))
    );

    // 未知键名必须被拒绝。
    let action = parse_action(&json!({ "kind": "press_key", "key": "nope" }));
    assert!(action.is_err(), "未知键名必须被拒绝");
}

// 四个指针窗口动作必须读取 x/y 坐标。
#[test]
fn parse_action_pointer_actions_read_coordinates() {
    // click_at 读取 x/y。
    let action = parse_action(&json!({ "kind": "click_at", "x": 10.0, "y": 20.5 }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Window(AgentWindowAction::ClickAt {
            position: crate::core::Point::new(10.0, 20.5),
        }))
    );
    // pointer_move 读取 x/y。
    let action = parse_action(&json!({ "kind": "pointer_move", "x": 1.0, "y": 2.0 }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Window(AgentWindowAction::PointerMove {
            position: crate::core::Point::new(1.0, 2.0),
        }))
    );
    // pointer_down 读取 x/y。
    let action = parse_action(&json!({ "kind": "pointer_down", "x": 3.0, "y": 4.0 }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Window(AgentWindowAction::PointerDown {
            position: crate::core::Point::new(3.0, 4.0),
        }))
    );
    // pointer_up 读取 x/y。
    let action = parse_action(&json!({ "kind": "pointer_up", "x": 5.0, "y": 6.0 }));
    assert_eq!(
        action,
        Ok(ParsedAgentAction::Window(AgentWindowAction::PointerUp {
            position: crate::core::Point::new(5.0, 6.0),
        }))
    );
    // 缺 y 属于缺失必填数字。
    let action = parse_action(&json!({ "kind": "click_at", "x": 1.0 }));
    assert!(action.is_err(), "缺 y 的 click_at 必须被拒绝");
}

// 非法动作对象必须被拒绝。
#[test]
fn parse_action_rejects_invalid_objects() {
    // 非对象动作（字符串）必须被拒绝。
    let action = parse_action(&json!("invoke"));
    assert!(action.is_err(), "字符串动作必须被拒绝");
    // 未知动作类型必须被拒绝。
    let action = parse_action(&json!({ "kind": "explode" }));
    assert!(action.is_err(), "未知动作类型必须被拒绝");
    // 缺 kind 字段属于缺失必填字符串。
    let action = parse_action(&json!({ "value": "x" }));
    assert!(action.is_err(), "缺 kind 的动作必须被拒绝");
}

// ── parse_target / parse_component_id ─────────────────────────────────────

// target 必须恰好提供 automation_id 或 node_id 之一。
#[test]
fn parse_target_accepts_exactly_one_selector() {
    // automation_id 选择器必须解析为自动化标识。
    let target = parse_target(&json!({ "automation_id": "search-box" }));
    assert_eq!(
        target,
        Ok(SemanticTarget::AutomationId("search-box".to_owned()))
    );
    // node_id 选择器必须解析为组件标识。
    let target = parse_target(&json!({ "node_id": "5:2" }));
    assert_eq!(
        target,
        Ok(SemanticTarget::NodeId(ComponentId::from_parts(5, 2)))
    );
    // 空 automation_id 必须被拒绝。
    let target = parse_target(&json!({ "automation_id": "" }));
    assert!(target.is_err(), "空 automation_id 必须被拒绝");
    // 两个选择器同时出现属于歧义请求。
    let target = parse_target(&json!({ "automation_id": "a", "node_id": "1:2" }));
    assert!(target.is_err(), "同时提供两个选择器必须被拒绝");
    // 两个选择器都缺失属于非法请求。
    let target = parse_target(&json!({}));
    assert!(target.is_err(), "缺少选择器的 target 必须被拒绝");
    // 非对象 target 必须被拒绝。
    let target = parse_target(&json!("node_id"));
    assert!(target.is_err(), "非对象 target 必须被拒绝");
}

// node_id 支持 slot:generation 与 tree_scope:slot:generation 两种格式。
#[test]
fn parse_component_id_supports_two_and_three_part_formats() {
    // 两段格式映射到非作用域组件标识。
    assert_eq!(
        parse_component_id("7:3"),
        Ok(ComponentId::from_parts(7, 3))
    );
    // 三段格式映射到作用域组件标识。
    assert_eq!(
        parse_component_id("1:7:3"),
        Ok(ComponentId::from_scoped_parts(1, 7, 3))
    );
    // 非法 slot 必须被拒绝。
    assert!(parse_component_id("x:3").is_err(), "非数字 slot 必须被拒绝");
    // 非法 generation 必须被拒绝。
    assert!(parse_component_id("7:x").is_err(), "非数字 generation 必须被拒绝");
    // 段数不对必须被拒绝。
    assert!(parse_component_id("7").is_err(), "单段 node_id 必须被拒绝");
    assert!(
        parse_component_id("1:2:3:4").is_err(),
        "四段 node_id 必须被拒绝"
    );
}

// automation_id 超过协议上限必须被拒绝。
#[test]
fn parse_target_rejects_oversized_automation_id() {
    // 构造 513 字节的自动化标识。
    let oversized = "a".repeat(513);
    let target = parse_target(&json!({ "automation_id": oversized }));
    assert!(target.is_err(), "超长 automation_id 必须被拒绝");
}

// ── 键名与修饰键解析 ─────────────────────────────────────────────────────

// 键名表必须覆盖常见功能键。
#[test]
fn parse_key_code_recognizes_function_and_navigation_keys() {
    // 导航键必须可用。
    assert_eq!(parse_key_code(as_map(&json!({ "key": "up" })),), Ok(KeyCode::Up));
    // 功能键必须可用。
    assert_eq!(parse_key_code(as_map(&json!({ "key": "f12" })),), Ok(KeyCode::F12));
    // 数字键必须可用。
    assert_eq!(parse_key_code(as_map(&json!({ "key": "0" })),), Ok(KeyCode::Num0));
    // 未知键名必须被拒绝。
    assert!(parse_key_code(as_map(&json!({ "key": "home_run" })),).is_err());
    // 缺 key 字段属于缺失必填字符串。
    assert!(parse_key_code(as_map(&json!({})),).is_err());
}

// 修饰键解析必须支持组合、去重与未知拒绝。
#[test]
fn parse_key_modifiers_enforces_modifier_contract() {
    // 缺省时不返回任何修饰键。
    assert_eq!(parse_key_modifiers(as_map(&json!({})),), Ok(KeyMod::NONE));
    // 单个修饰键必须映射到对应位。
    assert_eq!(
        parse_key_modifiers(as_map(&json!({ "modifiers": ["alt"] })),),
        Ok(KeyMod::ALT)
    );
    // 多修饰键必须按位组合。
    assert_eq!(
        parse_key_modifiers(as_map(&json!({ "modifiers": ["ctrl", "super"] })),),
        Ok(KeyMod::CTRL | KeyMod::SUPER)
    );
    // 重复修饰键必须被拒绝。
    assert!(parse_key_modifiers(as_map(&json!({ "modifiers": ["shift", "shift"] })),).is_err());
    // 未知修饰键必须被拒绝。
    assert!(parse_key_modifiers(as_map(&json!({ "modifiers": ["meta"] })),).is_err());
    // 非数组 modifiers 必须被拒绝。
    assert!(parse_key_modifiers(as_map(&json!({ "modifiers": "ctrl" })),).is_err());
    // 超出修饰键总数的数组必须被拒绝。
    assert!(
        parse_key_modifiers(as_map(&json!({ "modifiers": ["shift", "ctrl", "alt", "super", "shift"] })),)
            .is_err()
    );
}

// ── 必填/可选字段边界 ─────────────────────────────────────────────────────

// required_string 必须校验类型与长度上限。
#[test]
fn required_string_enforces_type_and_length() {
    // 合法字符串必须返回内容。
    assert_eq!(
        required_string(as_map(&json!({ "k": "v" })), "k", 8),
        Ok("v")
    );
    // 超长字符串必须被拒绝。
    assert!(required_string(as_map(&json!({ "k": "toolong" })), "k", 4).is_err());
    // 非字符串值必须被拒绝。
    assert!(required_string(as_map(&json!({ "k": 42 })), "k", 8).is_err());
    // 缺失字段必须被拒绝。
    assert!(required_string(as_map(&json!({})), "k", 8).is_err());
}

// 必填/可选 u64 字段必须校验类型。
#[test]
fn numeric_fields_enforce_unsigned_and_optional_semantics() {
    // 必填无符号整数读取成功。
    assert_eq!(required_u64(as_map(&json!({ "k": 7 })), "k"), Ok(7));
    // 必填无符号整数拒绝负数（as_u64 返回 None）。
    assert!(required_u64(as_map(&json!({ "k": -1 })), "k").is_err());
    // 必填无符号整数拒绝浮点。
    assert!(required_u64(as_map(&json!({ "k": 1.5 })), "k").is_err());
    // 可选字段缺省返回 None。
    assert_eq!(optional_u64(as_map(&json!({})), "k"), Ok(None));
    // 可选字段显式 null 返回 None。
    assert_eq!(optional_u64(as_map(&json!({ "k": null })), "k"), Ok(None));
    // 可选字段提供值时返回 Some。
    assert_eq!(optional_u64(as_map(&json!({ "k": 9 })), "k"), Ok(Some(9)));
    // 可选字段提供非法类型必须被拒绝。
    assert!(optional_u64(as_map(&json!({ "k": "x" })), "k").is_err());
}

// 必填/可选 f32 字段必须校验类型与 f32 范围（JSON 本身无法表示 NaN/Inf）。
#[test]
fn f32_fields_enforce_finite_and_range() {
    // 常规浮点必须可用。
    assert_eq!(required_f32(as_map(&json!({ "k": 1.25 })), "k"), Ok(1.25));
    // 超出 f32 上限的有限值必须被拒绝。
    assert!(required_f32(as_map(&json!({ "k": 1e300 })), "k").is_err());
    // 低于 f32 下限的有限值必须被拒绝。
    assert!(required_f32(as_map(&json!({ "k": -1e300 })), "k").is_err());
    // f32 边界内的值必须可用。
    assert_eq!(required_f32(as_map(&json!({ "k": 3.4028234e38 })), "k"), Ok(f32::MAX));
    // 非数字值必须被拒绝。
    assert!(required_f32(as_map(&json!({ "k": "1.5" })), "k").is_err());
    // 可选字段缺省返回 None。
    assert_eq!(optional_f32(as_map(&json!({})), "k"), Ok(None));
    // 可选字段 null 返回 None。
    assert_eq!(optional_f32(as_map(&json!({ "k": null })), "k"), Ok(None));
    // 可选字段提供值时返回 Some。
    assert_eq!(optional_f32(as_map(&json!({ "k": -3.5 })), "k"), Ok(Some(-3.5)));
    // 可选字段提供超范围值必须被拒绝。
    assert!(optional_f32(as_map(&json!({ "k": 1e300 })), "k").is_err());
}

// request_id 必须非空且来自必填字符串。
#[test]
fn request_id_rejects_empty_and_missing_ids() {
    // 合法 request_id 必须返回内容。
    assert_eq!(request_id(as_map(&json!({ "request_id": "abc" }))), Ok("abc".to_owned()));
    // 空 request_id 必须被拒绝。
    assert!(request_id(as_map(&json!({ "request_id": "" }))).is_err());
    // 缺失 request_id 属于缺失必填字符串。
    assert!(request_id(as_map(&json!({}))).is_err());
}

// ── token 编解码与匹配 ────────────────────────────────────────────────────

// 会话 token 编码必须输出 64 位小写十六进制。
#[test]
fn encode_session_token_emits_lowercase_hex() {
    // 全零 token 必须输出 64 个零。
    let encoded = encode_session_token(&[0u8; 32]);
    assert_eq!(encoded, "0".repeat(64));
    // 0xAB 重复必须输出 ab 重复。
    let encoded = encode_session_token(&[0xAB; 32]);
    assert_eq!(encoded, "ab".repeat(32));
    // 输出长度必须固定为 64。
    assert_eq!(encoded.len(), 64);
}

// decode_token 必须严格校验长度、ASCII 与十六进制字符。
#[test]
fn decode_token_validates_hex_format() {
    // 合法十六进制必须解码回原始字节。
    let mut decoded = [0u8; 32];
    assert!(decode_token(&"ab".repeat(32), &mut decoded));
    assert_eq!(decoded, [0xAB; 32]);
    // 大写十六进制同样合法。
    let mut decoded = [0u8; 32];
    assert!(decode_token(&"AB".repeat(32), &mut decoded));
    assert_eq!(decoded, [0xAB; 32]);
    // 长度不足必须失败。
    let mut decoded = [0u8; 32];
    assert!(!decode_token(&"ab".repeat(31), &mut decoded));
    // 非 ASCII 字符必须失败。
    let mut decoded = [0u8; 32];
    assert!(!decode_token(&"你".repeat(21), &mut decoded));
    // 非法十六进制字符必须失败。
    let mut decoded = [0u8; 32];
    assert!(!decode_token(&"zz".repeat(32), &mut decoded));
}

// token_matches 必须常量时间比较且只接受合法编码。
#[test]
fn token_matches_compares_decoded_bytes() {
    // 编码往返后必须匹配。
    let token = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    let encoded = encode_session_token(&token);
    // 相同 token 必须匹配。
    assert!(token_matches(&token, &encoded));
    // 大小写混写编码仍必须匹配（hex_nibble 接受 A-F）。
    let mixed = encoded.replace('a', "A");
    assert!(token_matches(&token, &mixed));
    // 单字节差异必须不匹配。
    let mut other = token;
    other[0] ^= 0xFF;
    assert!(!token_matches(&other, &encoded));
    // 非法编码（长度错误）必须不匹配。
    assert!(!token_matches(&token, &encoded[..62]));
}

// ── 协议会话：认证前拒绝路径 ──────────────────────────────────────────────

// 非 JSON 帧必须返回 invalid_request 并在未认证时关闭连接。
#[test]
fn session_rejects_non_json_line() {
    let mut session = session_with_token([0u8; 32]);
    // 用原始非法文本走一次真实帧解析（非 JSON 分支）。
    let reply = session.handle_line(b"this is not json\n");
    let value = reply_json(&reply);
    // 必须返回 invalid_request 错误码。
    assert_eq!(value["error"]["code"], "invalid_request");
    assert_eq!(value["error"]["message"], "message is not valid JSON");
    // 未认证连接的错误回复必须关闭连接。
    assert!(reply.close_connection());
    // 请求 ID 缺失时回复中为 null。
    assert_eq!(value["request_id"], serde_json::Value::Null);
}

// 顶层非对象帧必须被拒绝。
#[test]
fn session_rejects_non_object_request() {
    let mut session = session_with_token([0u8; 32]);
    // 数组请求不属于合法对象。
    let (json, code, close) = round_trip(&mut session, json!([1, 2, 3]));
    // 必须报对象格式错误。
    assert_eq!(json["error"]["message"], "request must be a JSON object");
    // 未认证时关闭连接。
    assert_eq!(code, "invalid_request");
    assert!(close);
}

// 缺失或空 request_id 必须被拒绝。
#[test]
fn session_rejects_missing_or_empty_request_id() {
    let mut session = session_with_token([0u8; 32]);
    // 缺失 request_id 的 hello 请求。
    let (json, code, _) = round_trip(
        &mut session,
        json!({ "schema": AGENT_PROTOCOL_SCHEMA, "type": "hello", "token": "00".repeat(64) }),
    );
    assert_eq!(code, "invalid_request");
    // 缺失必填字段使用通用缺字段消息（不含字段名）。
    assert_eq!(
        json["error"]["message"],
        "required string field is missing or invalid"
    );
}

// schema 不匹配必须返回 unsupported_schema。
#[test]
fn session_rejects_unsupported_schema() {
    let mut session = session_with_token([0u8; 32]);
    // 使用旧版本 schema 的 hello 请求。
    let (json, code, close) = round_trip(
        &mut session,
        json!({
            "schema": "uix.agent.v0",
            "request_id": "r1",
            "type": "hello",
            "token": "00".repeat(64),
        }),
    );
    // 必须报告协议版本不支持。
    assert_eq!(code, "unsupported_schema");
    assert_eq!(json["error"]["code"], "unsupported_schema");
    assert!(close);
}

// 未认证连接的第一个请求必须是 hello。
#[test]
fn session_requires_hello_as_first_request() {
    let mut session = session_with_token([0u8; 32]);
    // 直接发 list_windows 必须被拒绝并关闭连接。
    let (json, code, close) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r1",
            "type": "list_windows",
        }),
    );
    // 必须报未授权错误。
    assert_eq!(code, "unauthorized");
    assert_eq!(json["error"]["message"], "hello must be the first request");
    assert!(close);
}

// 错误 token 必须被拒绝并关闭连接。
#[test]
fn session_rejects_wrong_hello_token() {
    let mut session = session_with_token([0x42; 32]);
    // 使用与会话不符的 token。
    let (json, code, close) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r1",
            "type": "hello",
            "token": "00".repeat(64),
        }),
    );
    // 必须报认证失败。
    assert_eq!(code, "unauthorized");
    assert_eq!(json["error"]["message"], "session authentication failed");
    assert!(close);
}

// 缺 token 的 hello 等价于错误 token。
#[test]
fn session_rejects_hello_without_token() {
    let mut session = session_with_token([0x42; 32]);
    // hello 请求不携带 token 字段。
    let (json, code, close) = round_trip(
        &mut session,
        json!({ "schema": AGENT_PROTOCOL_SCHEMA, "request_id": "r1", "type": "hello" }),
    );
    // 必须报认证失败并关闭连接。
    assert_eq!(code, "unauthorized");
    assert_eq!(json["error"]["message"], "session authentication failed");
    assert!(close);
}

// ── 协议会话：认证后路径 ──────────────────────────────────────────────────

// 正确 token 的 hello 必须返回能力清单与协议限制。
#[test]
fn session_hello_reports_capabilities_and_limits() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 完成认证并取回能力回复。
    let json = authenticate(&mut session, token);
    // 回复必须标记成功。
    assert_eq!(json["ok"], true);
    assert_eq!(json["type"], "hello");
    // 能力清单必须包含协议支持的请求类型。
    let request_types = json["capabilities"]["request_types"]
        .as_array()
        .expect("capabilities 必须携带 request_types");
    assert_eq!(request_types.len(), 5);
    // 五个请求类型必须逐项存在。
    for expected in ["hello", "list_windows", "snapshot", "perform", "wait"] {
        assert!(
            request_types.iter().any(|value| value == expected),
            "缺少请求类型 {expected}"
        );
    }
    // 语义动作清单必须包含 adjust。
    let semantic_actions = json["capabilities"]["semantic_actions"]
        .as_array()
        .expect("capabilities 必须携带 semantic_actions");
    assert!(semantic_actions.iter().any(|value| value == "adjust"));
    // 限制必须暴露消息上限。
    assert_eq!(json["limits"]["max_message_bytes"], MAX_AGENT_MESSAGE_BYTES as u64);
}

// 认证后重复 hello 属于协议错误但不关闭连接。
#[test]
fn session_rejects_repeated_hello_after_auth() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 先完成一次认证。
    authenticate(&mut session, token);
    // 再次发送 hello。
    let (json, code, close) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r2",
            "type": "hello",
            "token": encode_session_token(&token),
        }),
    );
    // 必须报已认证错误且连接保持。
    assert_eq!(code, "invalid_request");
    assert_eq!(
        json["error"]["message"],
        "connection is already authenticated"
    );
    assert!(!close);
}

// 认证后未知请求类型必须被拒绝。
#[test]
fn session_rejects_unknown_request_type_after_auth() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 先完成认证。
    authenticate(&mut session, token);
    // 发送未知请求类型。
    let (json, code, close) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r3",
            "type": "explode",
        }),
    );
    // 必须报未知请求类型。
    assert_eq!(code, "invalid_request");
    assert_eq!(json["error"]["message"], "unknown request type");
    assert!(!close);
}

// 超长帧必须在进入解析前被拒绝。
#[test]
fn session_rejects_oversized_frame() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 构造超过协议上限的帧。
    let oversized = vec![b'x'; MAX_AGENT_MESSAGE_BYTES + 1];
    let reply = session.handle_line(&oversized);
    // 必须报超限错误。
    let json = reply_json(&reply);
    assert_eq!(json["error"]["code"], "invalid_request");
    assert_eq!(json["error"]["message"], "message exceeds the protocol limit");
    // 超限帧必须关闭连接。
    assert!(reply.close_connection());
}

// perform 缺 action 必须在桥调用前被拒绝。
#[test]
fn session_rejects_perform_without_action() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 先完成认证。
    authenticate(&mut session, token);
    // perform 请求不携带 action 对象。
    let (json, code, close) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r4",
            "type": "perform",
            "window_id": 1,
            "generation": 1,
        }),
    );
    // 必须报缺 action 对象。
    assert_eq!(code, "invalid_request");
    assert_eq!(
        json["error"]["message"],
        "perform requires an action object"
    );
    assert!(!close);
}

// 语义动作缺 target 必须在桥调用前被拒绝。
#[test]
fn session_rejects_semantic_perform_without_target() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 先完成认证。
    authenticate(&mut session, token);
    // 语义动作但未提供 target。
    let (json, code, _) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r5",
            "type": "perform",
            "window_id": 1,
            "generation": 1,
            "action": { "kind": "invoke" },
        }),
    );
    // 必须报缺 target 对象。
    assert_eq!(code, "invalid_request");
    assert_eq!(
        json["error"]["message"],
        "semantic action requires a target object"
    );
}

// 窗口动作携带 target 必须被拒绝。
#[test]
fn session_rejects_window_action_with_target() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 先完成认证。
    authenticate(&mut session, token);
    // 窗口动作同时携带 target 属于非法请求。
    let (json, code, _) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r6",
            "type": "perform",
            "window_id": 1,
            "generation": 1,
            "target": { "automation_id": "a" },
            "action": { "kind": "click_at", "x": 1.0, "y": 1.0 },
        }),
    );
    // 必须报窗口动作禁止携带 target。
    assert_eq!(code, "invalid_request");
    assert_eq!(
        json["error"]["message"],
        "window action must not include a target"
    );
}

// perform 指向不存在的窗口必须经桥返回 window_not_found。
#[test]
fn session_perform_on_missing_window_returns_not_found() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 先完成认证。
    authenticate(&mut session, token);
    // 指向从未注册的窗口（空运行时）。
    let (json, code, close) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r7",
            "type": "perform",
            "window_id": 999,
            "generation": 1,
            "target": { "node_id": "1:0" },
            "action": { "kind": "invoke" },
        }),
    );
    // 桥必须报告窗口不存在。
    assert_eq!(code, "window_not_found");
    assert_eq!(json["error"]["message"], "window was not found");
    assert!(!close);
}

// snapshot 缺 window_id 必须在桥调用前被拒绝。
#[test]
fn session_rejects_snapshot_without_window_id() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 先完成认证。
    authenticate(&mut session, token);
    // snapshot 请求缺少 window_id。
    let (json, code, _) = round_trip(
        &mut session,
        json!({ "schema": AGENT_PROTOCOL_SCHEMA, "request_id": "r8", "type": "snapshot" }),
    );
    // 必须报缺少必填字段的通用错误。
    assert_eq!(code, "invalid_request");
    assert_eq!(
        json["error"]["message"],
        "required unsigned integer field is missing or invalid"
    );
}

// wait 必须恰好提供一个修订条件。
#[test]
fn session_rejects_wait_with_wrong_revision_conditions() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 先完成认证。
    authenticate(&mut session, token);
    // 两个条件同时给出属于非法请求。
    let (json, code, _) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r9",
            "type": "wait",
            "window_id": 1,
            "generation": 1,
            "timeout_ms": 100,
            "after_revision": 1,
            "presented_revision": 1,
        }),
    );
    // 必须报条件二选一。
    assert_eq!(code, "invalid_request");
    assert_eq!(
        json["error"]["message"],
        "wait requires exactly one revision condition"
    );

    // 两个条件都缺失同样非法。
    let (_json, code, _) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r10",
            "type": "wait",
            "window_id": 1,
            "generation": 1,
            "timeout_ms": 100,
        }),
    );
    assert_eq!(code, "invalid_request");

    // 缺 timeout_ms 属于必填字段缺失。
    let (json, code, _) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r11",
            "type": "wait",
            "window_id": 1,
            "generation": 1,
            "after_revision": 1,
        }),
    );
    assert_eq!(code, "invalid_request");
    // 缺 timeout_ms 属于必填无符号整数字段缺失。
    assert_eq!(
        json["error"]["message"],
        "required unsigned integer field is missing or invalid"
    );
}

// wait 指向不存在窗口必须返回 window_not_found。
#[test]
fn session_wait_on_missing_window_returns_not_found() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 先完成认证。
    authenticate(&mut session, token);
    // 等待从未注册窗口的修订。
    let (_json, code, _) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r12",
            "type": "wait",
            "window_id": 999,
            "generation": 1,
            "timeout_ms": 100,
            "after_revision": 0,
        }),
    );
    // 桥必须报告窗口不存在。
    assert_eq!(code, "window_not_found");
}

// list_windows 在空运行时下必须返回空列表。
#[test]
fn session_list_windows_returns_empty_on_idle_runtime() {
    let token = [0x5A; 32];
    let mut session = session_with_token(token);
    // 先完成认证。
    authenticate(&mut session, token);
    // 查询窗口列表（空运行时）。
    let (json, code, close) = round_trip(
        &mut session,
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "r13",
            "type": "list_windows",
        }),
    );
    // 必须成功且窗口列表为空。
    assert_eq!(code, "ok");
    assert_eq!(json["windows"].as_array().expect("windows 必须是数组").len(), 0);
    assert!(!close);
}

// ── 回复帧构造 ────────────────────────────────────────────────────────────

// success_reply 必须携带 schema/request_id/ok/type 与载荷字段。
#[test]
fn success_reply_carries_schema_and_payload() {
    // 构造带载荷的成功回复。
    let reply = success_reply(
        "req-1".to_owned(),
        "perform",
        json!({ "revision": 3, "settled": true }),
    );
    // 回复帧必须是单行 JSON。
    assert_eq!(reply.bytes().last(), Some(&b'\n'));
    let value = reply_json(&reply);
    // 协议元数据必须完整。
    assert_eq!(value["schema"], AGENT_PROTOCOL_SCHEMA);
    assert_eq!(value["request_id"], "req-1");
    assert_eq!(value["ok"], true);
    assert_eq!(value["type"], "perform");
    // 载荷字段必须平铺进顶层。
    assert_eq!(value["revision"], 3);
    assert_eq!(value["settled"], true);
    // 成功回复不关闭连接。
    assert!(!reply.close_connection());
    assert_eq!(reply.result_code(), "ok");
}

// error_reply 必须携带错误码与消息，close 标志随调用方决定。
#[test]
fn error_reply_carries_code_message_and_close_flag() {
    // 构造带请求 ID 的错误回复。
    let reply = error_reply(
        Some("req-2".to_owned()),
        AgentErrorCode::Timeout,
        "UI command timed out",
        false,
    );
    let value = reply_json(&reply);
    // 错误结构必须完整。
    assert_eq!(value["ok"], false);
    assert_eq!(value["request_id"], "req-2");
    assert_eq!(value["error"]["code"], "timeout");
    assert_eq!(value["error"]["message"], "UI command timed out");
    // close 标志必须透传。
    assert!(!reply.close_connection());
    assert_eq!(reply.result_code(), "timeout");

    // 框架错误回复必须关闭连接。
    let framing = framing_error_reply("message exceeds the protocol limit");
    assert!(framing.close_connection());
    assert_eq!(framing.result_code(), "invalid_request");
}

// ── AgentErrorCode 字符串映射 ─────────────────────────────────────────────

// 全部错误码必须映射到稳定线协议字符串。
#[test]
fn agent_error_code_as_str_maps_all_variants() {
    // 逐项断言错误码与线协议字符串的映射。
    let cases = [
        (AgentErrorCode::Unauthorized, "unauthorized"),
        (AgentErrorCode::UnsupportedSchema, "unsupported_schema"),
        (AgentErrorCode::InvalidRequest, "invalid_request"),
        (AgentErrorCode::WindowNotFound, "window_not_found"),
        (AgentErrorCode::StaleWindow, "stale_window"),
        (AgentErrorCode::StaleRevision, "stale_revision"),
        (AgentErrorCode::NodeNotFound, "node_not_found"),
        (AgentErrorCode::AmbiguousTarget, "ambiguous_target"),
        (AgentErrorCode::UnsupportedAction, "unsupported_action"),
        (AgentErrorCode::InvalidValue, "invalid_value"),
        (AgentErrorCode::NotInteractable, "not_interactable"),
        (AgentErrorCode::Blocked, "blocked"),
        (AgentErrorCode::DidNotSettle, "did_not_settle"),
        (AgentErrorCode::NotPresentable, "not_presentable"),
        (AgentErrorCode::Timeout, "timeout"),
        (AgentErrorCode::AppClosed, "app_closed"),
        (AgentErrorCode::Internal, "internal"),
    ];
    // 全部 17 个错误码必须逐一验证。
    assert_eq!(cases.len(), 17);
    for (code, expected) in cases {
        assert_eq!(code.as_str(), expected, "错误码映射错误");
    }
}

// ── 协议常量契约 ──────────────────────────────────────────────────────────

// 请求类型表必须恰好包含协议分派的五个类型。
#[test]
fn agent_request_types_cover_protocol_dispatch() {
    // 请求类型表长度必须为 5。
    assert_eq!(AGENT_REQUEST_TYPES.len(), 5);
    // 协议分派的五种类型必须与表一致。
    for expected in ["hello", "list_windows", "snapshot", "perform", "wait"] {
        assert!(AGENT_REQUEST_TYPES.contains(&expected), "缺少 {expected}");
    }
}

// 语义动作表必须包含 adjust（E-05）在内的全部动作。
#[test]
fn agent_semantic_actions_include_adjust() {
    // 语义动作表必须覆盖协议解析的全部动作。
    for expected in [
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
    ] {
        assert!(
            AGENT_SEMANTIC_ACTIONS.contains(&expected),
            "缺少语义动作 {expected}"
        );
    }
}
