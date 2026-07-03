// ============================================================================
// platform/settings.rs — 键值设置服务（JSON 持久化）
//
// 提供扁平 HashMap<String, String> 的 JSON 读写能力。
// 原位于 services crate，迁入 platform 层以消除服务层。
// ============================================================================

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::{Error, Errc, Result};

// ── 极简 JSON 读写（仅支持扁平 HashMap<String, String>） ────────────────

/// 解析 JSON 对象为 HashMap<String, String>。
/// 仅支持 `{"key": "value", ...}` 格式，值必须是双引号字符串。
fn parse_json_flat(input: &str) -> Result<HashMap<String, String>> {
    let input = input.trim();
    if !input.starts_with('{') || !input.ends_with('}') {
        return Err(Error::new(Errc::FormatError, "settings: expected JSON object"));
    }
    let inner = input[1..input.len() - 1].trim();
    if inner.is_empty() {
        return Ok(HashMap::new());
    }

    let mut map = HashMap::new();
    let mut pos = 0usize;
    let bytes = inner.as_bytes();

    while pos < bytes.len() {
        // 跳过空白
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() { pos += 1; }
        if pos >= bytes.len() { break; }

        // 解析 key（双引号字符串）
        if bytes[pos] != b'"' {
            return Err(Error::new(Errc::FormatError, "settings: expected key string"));
        }
        pos += 1;
        let key_start = pos;
        while pos < bytes.len() && bytes[pos] != b'"' {
            if bytes[pos] == b'\\' { pos += 2; } else { pos += 1; }
        }
        if pos >= bytes.len() {
            return Err(Error::new(Errc::FormatError, "settings: unterminated key"));
        }
        let key = unescape_json_str(&inner[key_start..pos]);
        pos += 1;

        // 跳过空白和 ':'
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() { pos += 1; }
        if pos >= bytes.len() || bytes[pos] != b':' {
            return Err(Error::new(Errc::FormatError, "settings: expected ':'"));
        }
        pos += 1;
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() { pos += 1; }

        // 解析 value（必须是双引号字符串）
        if pos >= bytes.len() || bytes[pos] != b'"' {
            return Err(Error::new(Errc::FormatError, "settings: expected value string"));
        }
        pos += 1;
        let val_start = pos;
        while pos < bytes.len() && bytes[pos] != b'"' {
            if bytes[pos] == b'\\' { pos += 2; } else { pos += 1; }
        }
        if pos >= bytes.len() {
            return Err(Error::new(Errc::FormatError, "settings: unterminated value"));
        }
        let val = unescape_json_str(&inner[val_start..pos]);
        pos += 1;

        map.insert(key, val);

        // 跳过空白和可选的 ','
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() { pos += 1; }
        if pos < bytes.len() && bytes[pos] == b',' { pos += 1; }
    }

    Ok(map)
}

/// JSON 字符串 unescape。
fn unescape_json_str(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(c) => { out.push('\\'); out.push(c); }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// JSON 字符串 escape。
fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_ascii_control() => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// 序列化 HashMap<String, String> 为美化 JSON（2 空格缩进）。
fn serialize_json_flat(map: &HashMap<String, String>) -> String {
    if map.is_empty() {
        return "{}".to_string();
    }
    let mut out = String::with_capacity(128);
    out.push_str("{\n");
    let mut first = true;
    for (k, v) in map {
        if !first { out.push_str(",\n"); }
        first = false;
        out.push_str("  \"");
        out.push_str(&escape_json_str(k));
        out.push_str("\": \"");
        out.push_str(&escape_json_str(v));
        out.push('"');
    }
    out.push_str("\n}");
    out
}

/// 键值设置服务，支持 JSON 文件持久化。
#[derive(Debug, Clone, Default)]
pub struct SettingsService {
    path: Option<String>,
    values: HashMap<String, String>,
    dirty: bool,
}

impl SettingsService {
    pub fn new() -> Self { Self::default() }

    /// 从 JSON 文件加载设置。
    pub fn load(&mut self, path: &str) -> Result<()> {
        self.path = Some(path.to_string());
        self.values.clear();

        if !Path::new(path).exists() {
            return Ok(());
        }

        let content = fs::read_to_string(path)?;
        if content.trim().is_empty() {
            return Ok(());
        }

        self.values = parse_json_flat(&content)?;
        self.dirty = false;
        Ok(())
    }

    /// 保存设置到 JSON 文件。
    pub fn save(&mut self) -> Result<()> {
        let path = self.path.as_deref()
            .ok_or_else(|| Error::new(Errc::InvalidState, "no settings path set"))?;
        let json = serialize_json_flat(&self.values);
        fs::write(path, &json)?;
        self.dirty = false;
        Ok(())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_string(), value.to_string());
        self.dirty = true;
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.values.get(key).map(|s| s.as_str()).unwrap_or(default)
    }

    pub fn has(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn remove(&mut self, key: &str) {
        self.values.remove(key);
        self.dirty = true;
    }

    pub fn clear(&mut self) {
        self.values.clear();
        self.dirty = true;
    }

    pub fn all(&self) -> &HashMap<String, String> { &self.values }
    pub fn dirty(&self) -> bool { self.dirty }
    pub fn count(&self) -> usize { self.values.len() }
}

// ════════════════════════════════════════════════════════════════════════════
// JSON 解析器单元测试
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod json_parser_tests {
    use super::*;

    // ── parse_json_flat：正常路径 ──────────────────────────────────────

    #[test]
    fn parse_empty_object() {
        let m = parse_json_flat("{}").unwrap();
        assert!(m.is_empty());
    }

    #[test]
    fn parse_empty_object_with_whitespace() {
        let m = parse_json_flat("  {  }  ").unwrap();
        assert!(m.is_empty());
    }

    #[test]
    fn parse_single_pair() {
        let m = parse_json_flat(r#"{"key": "value"}"#).unwrap();
        assert_eq!(m.get("key").unwrap(), "value");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn parse_multiple_pairs() {
        let m = parse_json_flat(r#"{"a": "1", "b": "2", "c": "3"}"#).unwrap();
        assert_eq!(m.len(), 3);
        assert_eq!(m.get("a").unwrap(), "1");
        assert_eq!(m.get("b").unwrap(), "2");
        assert_eq!(m.get("c").unwrap(), "3");
    }

    #[test]
    fn parse_extra_whitespace() {
        let m = parse_json_flat(r#"{  "key"  :  "val"  }"#).unwrap();
        assert_eq!(m.get("key").unwrap(), "val");
    }

    #[test]
    fn parse_newlines_and_tabs() {
        let input = "{\n  \"key\": \"val\"\n}";
        let m = parse_json_flat(input).unwrap();
        assert_eq!(m.get("key").unwrap(), "val");
    }

    // ── parse_json_flat：转义字符 ──────────────────────────────────────

    #[test]
    fn parse_escaped_quote_in_value() {
        let m = parse_json_flat(r#"{"key": "hello\"world"}"#).unwrap();
        assert_eq!(m.get("key").unwrap(), "hello\"world");
    }

    #[test]
    fn parse_escaped_backslash() {
        let m = parse_json_flat(r#"{"path": "C:\\Users"}"#).unwrap();
        assert_eq!(m.get("path").unwrap(), "C:\\Users");
    }

    #[test]
    fn parse_escaped_newline() {
        let m = parse_json_flat(r#"{"msg": "line1\nline2"}"#).unwrap();
        assert_eq!(m.get("msg").unwrap(), "line1\nline2");
    }

    #[test]
    fn parse_escaped_tab() {
        let m = parse_json_flat(r#"{"col": "a\tb"}"#).unwrap();
        assert_eq!(m.get("col").unwrap(), "a\tb");
    }

    #[test]
    fn parse_escaped_key() {
        let m = parse_json_flat(r#"{"he\"llo": "world"}"#).unwrap();
        assert_eq!(m.get("he\"llo").unwrap(), "world");
    }

    // ── parse_json_flat：错误路径 ──────────────────────────────────────

    #[test]
    fn parse_not_json_object_returns_error() {
        let r = parse_json_flat("null");
        assert!(r.is_err());
        assert_eq!(r.unwrap_err().code(), Errc::FormatError);
    }

    #[test]
    fn parse_array_returns_error() {
        let r = parse_json_flat("[\"a\"]");
        assert!(r.is_err());
    }

    #[test]
    fn parse_invalid_key_not_string() {
        let r = parse_json_flat("{123: \"val\"}");
        assert!(r.is_err());
    }

    #[test]
    fn parse_invalid_value_not_string() {
        let r = parse_json_flat(r#"{"key": 123}"#);
        assert!(r.is_err());
    }

    #[test]
    fn parse_unterminated_key() {
        let r = parse_json_flat(r#"{"key: "val"}"#);
        assert!(r.is_err());
    }

    #[test]
    fn parse_unterminated_value() {
        let r = parse_json_flat(r#"{"key": "val}"#);
        assert!(r.is_err());
    }

    #[test]
    fn parse_missing_colon() {
        let r = parse_json_flat(r#"{"key" "val"}"#);
        assert!(r.is_err());
    }

    #[test]
    fn parse_trailing_comma_is_tolerated() {
        // 多余逗号被当作分隔符跳过，解析器宽容处理
        let m = parse_json_flat(r#"{"a": "1",}"#).unwrap();
        assert_eq!(m.get("a").unwrap(), "1");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn parse_empty_input() {
        let r = parse_json_flat("");
        assert!(r.is_err());
    }

    #[test]
    fn parse_only_whitespace() {
        let r = parse_json_flat("   ");
        assert!(r.is_err());
    }

    // ── serialize_json_flat ────────────────────────────────────────────

    #[test]
    fn serialize_empty() {
        let map = HashMap::new();
        let json = serialize_json_flat(&map);
        assert_eq!(json, "{}");
    }

    #[test]
    fn serialize_one_entry() {
        let mut map = HashMap::new();
        map.insert("key".into(), "value".into());
        let json = serialize_json_flat(&map);
        assert!(json.contains(r#""key": "value""#));
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }

    #[test]
    fn serialize_multiple_entries() {
        let mut map = HashMap::new();
        map.insert("a".into(), "1".into());
        map.insert("b".into(), "2".into());
        let json = serialize_json_flat(&map);
        // 两个键值对都应出现
        assert!(json.contains(r#""a": "1""#));
        assert!(json.contains(r#""b": "2""#));
    }

    #[test]
    fn serialize_escapes_special_chars() {
        let mut map = HashMap::new();
        map.insert("k".into(), "hello\"world\nnext".into());
        let json = serialize_json_flat(&map);
        assert!(json.contains(r#"hello\"world\nnext"#));
    }

    // ── 序列化 ↔ 反序列化 一致性 ───────────────────────────────────────

    #[test]
    fn roundtrip_identity() {
        let cases = vec![
            r#"{"a": "1"}"#,
            r#"{"a": "1", "b": "2"}"#,
            r#"{"key": "value"}"#,
            r#"{"x": "y"}"#,
        ];
        for input in cases {
            let map = parse_json_flat(input).unwrap();
            let json = serialize_json_flat(&map);
            let map2 = parse_json_flat(&json).unwrap();
            assert_eq!(map, map2, "roundtrip failed for: {}", input);
        }
    }

    // ════════════════════════════════════════════════════════════════════
    // SettingsService 原有测试
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_set_get() {
        let mut s = SettingsService::new();
        s.set("theme", "dark");
        assert_eq!(s.get("theme"), Some("dark"));
        assert!(s.dirty());
    }

    #[test]
    fn test_get_or() {
        let s = SettingsService::new();
        assert_eq!(s.get_or("missing", "default"), "default");
    }

    #[test]
    fn test_has_remove() {
        let mut s = SettingsService::new();
        s.set("key", "val");
        assert!(s.has("key"));
        s.remove("key");
        assert!(!s.has("key"));
    }

    #[test]
    fn test_clear() {
        let mut s = SettingsService::new();
        s.set("a", "1");
        s.set("b", "2");
        s.clear();
        assert_eq!(s.count(), 0);
    }

    #[test]
    fn test_save_load_roundtrip() {
        let p = std::env::temp_dir().join("uix_settings_test.json");
        let ps = p.to_string_lossy().to_string();

        {
            let mut s = SettingsService::new();
            s.set("key1", "value1");
            s.set("key2", "value2");
            s.load(&ps).unwrap();
            s.set("key3", "value3");
            s.save().unwrap();
        }

        {
            let mut s2 = SettingsService::new();
            s2.load(&ps).unwrap();
            assert_eq!(s2.get("key3"), Some("value3"));
            assert!(!s2.dirty());
        }

        std::fs::remove_file(&ps).ok();
    }

    #[test]
    fn test_all() {
        let mut s = SettingsService::new();
        s.set("a", "1");
        s.set("b", "2");
        let all = s.all();
        assert_eq!(all.len(), 2);
        assert_eq!(all.get("a").unwrap(), "1");
    }
}
