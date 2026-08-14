// data/settings 键值设置专项测试。
// 覆盖扁平 JSON 解析/序列化往返、重复 key 拒绝、尾逗号拒绝、转义与
// Unicode 校验，以及配置路径解析（绝对/相对/空路径）。

// 引入被测的 JSON 读写与路径解析函数。
use super::{parse_json_flat, resolve_configured_settings_path, serialize_json_flat};

/// 断言解析成功且与期望映射一致。
fn assert_parses(input: &str, expected: &[(&str, &str)]) {
    let map = parse_json_flat(input).expect("解析必须成功");
    assert_eq!(
        map.len(),
        expected.len(),
        "键数量不符: {input}"
    );
    for (key, value) in expected {
        assert_eq!(
            map.get(*key).map(String::as_str),
            Some(*value),
            "键 {key} 的值不符"
        );
    }
}

/// 断言解析失败。
fn assert_rejects(input: &str) {
    assert!(
        parse_json_flat(input).is_err(),
        "输入必须被拒绝: {input}"
    );
}

// ── parse_json_flat 基本语法 ───────────────────────────────────────────────

// 空对象必须解析为空映射。
#[test]
fn empty_object_parses_to_empty_map() {
    // 空对象。
    assert_parses("{}", &[]);
    // 带空白包围的空对象。
    assert_parses("  { }  ", &[]);
}

// 单键与多键对象必须正确解析。
#[test]
fn simple_objects_parse_key_value_pairs() {
    // 单键值。
    assert_parses(r#"{"name": "uix"}"#, &[("name", "uix")]);
    // 多键值（含空白）。
    assert_parses(
        "{\n  \"a\": \"1\",\n  \"b\": \"2\"\n}",
        &[("a", "1"), ("b", "2")],
    );
}

// 顶层非对象输入必须被拒绝。
#[test]
fn non_object_input_is_rejected() {
    // 数组。
    assert_rejects(r#"["a"]"#);
    // 字符串。
    assert_rejects(r#""hello""#);
    // 数字。
    assert_rejects("42");
    // 空输入。
    assert_rejects("");
}

// 尾随内容与缺失标点必须被拒绝。
#[test]
fn malformed_structure_is_rejected() {
    // 对象后尾随内容。
    assert_rejects(r#"{"a":"1"} extra"#);
    // 缺冒号。
    assert_rejects(r#"{"a" "1"}"#);
    // 缺逗号或右花括号。
    assert_rejects(r#"{"a":"1" "b":"2"}"#);
    // 缺少右花括号。
    assert_rejects(r#"{"a":"1"}"#.replace("}", "").as_str());
}

// ── 重复 key 与尾逗号拒绝 ─────────────────────────────────────────────────

// 重复 key 必须被拒绝（解析器不做后者覆盖）。
#[test]
fn duplicate_keys_are_rejected() {
    // 完全相同的键出现两次。
    assert_rejects(r#"{"a": "1", "a": "2"}"#);
    // 不同空白排版下的重复键同样拒绝。
    assert_rejects("{\"a\":\"1\",\n\"a\":\"2\"}");
}

// 尾逗号必须被拒绝（严格 JSON 语法）。
#[test]
fn trailing_comma_is_rejected() {
    // 单键尾逗号。
    assert_rejects(r#"{"a": "1",}"#);
    // 多键尾逗号。
    assert_rejects(r#"{"a": "1", "b": "2",}"#);
}

// ── 字符串转义与 Unicode ──────────────────────────────────────────────────

// 常见转义序列必须解码。
#[test]
fn escape_sequences_are_decoded() {
    // 引号、反斜杠与常用控制转义。
    assert_parses(
        r#"{"a": "say \"hi\"\\"}"#,
        &[("a", "say \"hi\"\\")],
    );
    // 换行与制表符转义。
    assert_parses(
        r#"{"a": "line1\nline2\tend"}"#,
        &[("a", "line1\nline2\tend")],
    );
    // 正斜杠转义。
    assert_parses(r#"{"a": "a\/b"}"#, &[("a", "a/b")]);
}

// Unicode 转义与代理对必须解码为标量。
#[test]
fn unicode_escapes_and_surrogate_pairs_are_decoded() {
    // 基本多文种平面转义。
    assert_parses(r#"{"a": "\u4f60\u597d"}"#, &[("a", "你好")]);
    // 代理对（😀 = U+1F600）。
    assert_parses(r#"{"a": "\ud83d\ude00"}"#, &[("a", "😀")]);
    // 原始非 ASCII 字符直接保留。
    assert_parses(r#"{"a": "中文"}"#, &[("a", "中文")]);
}

// 非法转义与控制字符必须被拒绝。
#[test]
fn invalid_escapes_and_control_chars_are_rejected() {
    // 未知转义。
    assert_rejects(r#"{"a": "\q"}"#);
    // 未终止转义。
    assert_rejects(r#"{"a": "x\"}"#);
    // 原始控制字符。
    assert_rejects("{\"a\": \"x\u{0007}y\"}");
    // 未终止字符串。
    assert_rejects(r#"{"a": "unterminated}"#);
    // 值不是字符串。
    assert_rejects(r#"{"a": 42}"#);
    // 值缺少引号。
    assert_rejects(r#"{"a": plain}"#);
}

// Unicode 转义语法错误必须被拒绝。
#[test]
fn invalid_unicode_escapes_are_rejected() {
    // 高代理后缺少低代理。
    assert_rejects(r#"{"a": "\ud83d"}"#);
    // 高代理后接非代理对。
    assert_rejects(r#"{"a": "\ud83d\u0041"}"#);
    // 孤立低代理。
    assert_rejects(r#"{"a": "\ude00"}"#);
    // 截断的十六进制。
    assert_rejects(r#"{"a": "\u12"}"#);
    // 非法十六进制字符。
    assert_rejects(r#"{"a": "\u12zz"}"#);
}

// ── serialize_json_flat 与往返 ────────────────────────────────────────────

// 空映射必须序列化为空对象。
#[test]
fn empty_map_serializes_to_empty_object() {
    let map = std::collections::HashMap::new();
    // 空映射输出空对象。
    assert_eq!(serialize_json_flat(&map), "{}");
}

// 序列化必须按键排序并转义特殊字符。
#[test]
fn serialization_sorts_keys_and_escapes() {
    let mut map = std::collections::HashMap::new();
    // 键顺序故意乱序，键与值都含特殊字符。
    map.insert("z".to_owned(), "尾".to_owned());
    map.insert("a".to_owned(), "say \"hi\"\nline2".to_owned());
    let json = serialize_json_flat(&map);
    // 键必须按字典序输出。
    assert!(json.find("\"a\"").unwrap() < json.find("\"z\"").unwrap());
    // 特殊字符必须被转义。
    assert!(json.contains("say \\\"hi\\\"\\nline2"));
    // 整体是合法 JSON。
    let round_trip = parse_json_flat(&json).expect("序列化结果必须可解析");
    assert_eq!(round_trip.get("a").map(String::as_str), Some("say \"hi\"\nline2"));
}

// 序列化 → 解析 必须无损往返。
#[test]
fn serialize_parse_round_trip_is_lossless() {
    // 构造含特殊字符的多种键值。
    let mut map = std::collections::HashMap::new();
    map.insert("k1".to_owned(), "普通值".to_owned());
    map.insert("k2".to_owned(), "带\"引号\"和\\反斜杠".to_owned());
    map.insert("k3".to_owned(), "换行\n制表\t回车\r".to_owned());
    map.insert("k4".to_owned(), "控制\u{0001}字符".to_owned());
    // 序列化后必须可解析且完全一致。
    let json = serialize_json_flat(&map);
    let parsed = parse_json_flat(&json).expect("往返解析必须成功");
    assert_eq!(parsed, map);
}

// 多组输入序列化后再次解析必须保持语义。
#[test]
fn multi_case_serialize_parse_round_trip() {
    // 逐组验证序列化往返。
    for input in [
        r#"{"a":"1"}"#,
        r#"{"alpha":"x","beta":"y"}"#,
        r#"{"unicode":"你好😀"}"#,
        r#"{"escaped":"\"quoted\" \\ and /"}"#,
    ] {
        // 原输入解析。
        let map = parse_json_flat(input).expect("输入必须可解析");
        // 序列化后再次解析必须一致。
        let reparsed = parse_json_flat(&serialize_json_flat(&map)).expect("往返必须可解析");
        assert_eq!(reparsed, map, "往返不一致: {input}");
    }
}

// ── resolve_configured_settings_path ───────────────────────────────────────

// 空路径必须被拒绝。
#[test]
fn empty_settings_path_is_rejected() {
    // 空字符串。
    assert!(resolve_configured_settings_path("").is_err());
    // 纯空白字符串。
    assert!(resolve_configured_settings_path("   ").is_err());
}

// 绝对路径必须原样返回。
#[test]
fn absolute_settings_path_is_returned_unchanged() {
    // 使用平台原生临时目录构造绝对路径，避免 Unix 根路径在 Windows 上被补入盘符。
    let configured_path = std::env::temp_dir().join("uix-settings.json");
    // 配置接口只接受 UTF-8 字符串，测试路径必须满足同一契约。
    let configured = configured_path.to_str().expect("测试路径必须是 UTF-8");
    // 绝对路径不经过可执行文件目录解析。
    let resolved = resolve_configured_settings_path(configured).expect("必须成功");
    // 返回值必须与传入的原生绝对路径完全一致。
    assert_eq!(resolved, configured);
}

// 相对路径必须基于可执行文件目录解析为绝对路径。
#[test]
fn relative_settings_path_resolves_against_executable_dir() {
    // 相对路径必须解析为绝对路径。
    let resolved = resolve_configured_settings_path("data/settings.json").expect("必须成功");
    // 解析结果必须以可执行文件目录开头（先持有 PathBuf 再取父目录）。
    let executable = std::env::current_exe().expect("测试进程必须可查询可执行文件");
    let executable_dir = executable
        .parent()
        .expect("可执行文件必须有父目录")
        .to_path_buf();
    let resolved_path = std::path::Path::new(&resolved);
    assert!(resolved_path.is_absolute());
    // 解析结果必须包含配置的相对路径后缀。
    assert!(
        resolved_path.ends_with("data/settings.json"),
        "解析结果必须拼接相对路径: {resolved}"
    );
    // 可执行文件目录本身是解析出的前缀。
    assert!(
        resolved_path.starts_with(executable_dir),
        "解析结果必须基于可执行文件目录: {resolved}"
    );
}
