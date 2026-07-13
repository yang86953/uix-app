// ============================================================================
// data/settings/settings.rs — 键值设置服务（JSON 持久化）
//
// 提供扁平 HashMap<String, String> 的 JSON 读写能力。
// 原位于 services crate；现归 data 域，仅作为冷路径配置持久化能力。
// ============================================================================

use crate::core::{Errc, Error, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ── 极简 JSON 读写（仅支持扁平 HashMap<String, String>） ────────────────

/// 解析 JSON 对象为 HashMap<String, String>。
/// 仅支持 `{"key": "value", ...}` 格式，值必须是双引号字符串。
pub(crate) fn parse_json_flat(input: &str) -> Result<HashMap<String, String>> {
    let input = input.trim();
    if !input.starts_with('{') || !input.ends_with('}') {
        return Err(Error::new(
            Errc::FormatError,
            "settings: expected JSON object",
        ));
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
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }

        // 解析 key（双引号字符串）
        if bytes[pos] != b'"' {
            return Err(Error::new(
                Errc::FormatError,
                "settings: expected key string",
            ));
        }
        pos += 1;
        let key_start = pos;
        while pos < bytes.len() && bytes[pos] != b'"' {
            if bytes[pos] == b'\\' {
                pos += 2;
            } else {
                pos += 1;
            }
        }
        if pos >= bytes.len() {
            return Err(Error::new(Errc::FormatError, "settings: unterminated key"));
        }
        let key = unescape_json_str(&inner[key_start..pos]);
        pos += 1;

        // 跳过空白和 ':'
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() || bytes[pos] != b':' {
            return Err(Error::new(Errc::FormatError, "settings: expected ':'"));
        }
        pos += 1;
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }

        // 解析 value（必须是双引号字符串）
        if pos >= bytes.len() || bytes[pos] != b'"' {
            return Err(Error::new(
                Errc::FormatError,
                "settings: expected value string",
            ));
        }
        pos += 1;
        let val_start = pos;
        while pos < bytes.len() && bytes[pos] != b'"' {
            if bytes[pos] == b'\\' {
                pos += 2;
            } else {
                pos += 1;
            }
        }
        if pos >= bytes.len() {
            return Err(Error::new(
                Errc::FormatError,
                "settings: unterminated value",
            ));
        }
        let val = unescape_json_str(&inner[val_start..pos]);
        pos += 1;

        map.insert(key, val);

        // 跳过空白和可选的 ','
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos < bytes.len() && bytes[pos] == b',' {
            pos += 1;
        }
    }

    Ok(map)
}

/// JSON 字符串 unescape（含 \uXXXX 解码）。
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
                Some('/') => out.push('/'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if hex.len() == 4 {
                        if let Ok(code) = u32::from_str_radix(&hex, 16) {
                            if let Some(ch) = char::from_u32(code) {
                                out.push(ch);
                            } else {
                                out.push_str(&format!("\\u{hex}"));
                            }
                        } else {
                            out.push_str(&format!("\\u{hex}"));
                        }
                    } else {
                        out.push_str(&format!("\\u{hex}"));
                    }
                }
                Some(c) => {
                    out.push('\\');
                    out.push(c);
                }
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
pub(crate) fn serialize_json_flat(map: &HashMap<String, String>) -> String {
    if map.is_empty() {
        return "{}".to_string();
    }
    let mut out = String::with_capacity(128);
    out.push_str("{\n");
    let mut first = true;
    for (k, v) in map {
        if !first {
            out.push_str(",\n");
        }
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
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 JSON 文件加载设置。
    /// 先读入并解析，成功后一次提交 path / values / dirty；失败时保持原状态不变。
    pub fn load(&mut self, path: &str) -> Result<()> {
        if !Path::new(path).exists() {
            self.path = Some(path.to_string());
            self.values.clear();
            self.dirty = false;
            return Ok(());
        }

        let content = fs::read_to_string(path)?;
        let parsed = if content.trim().is_empty() {
            HashMap::new()
        } else {
            parse_json_flat(&content)?
        };
        self.path = Some(path.to_string());
        self.values = parsed;
        self.dirty = false;
        Ok(())
    }

    /// 保存设置到 JSON 文件（临时文件 + rename 保证原子性）。
    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };
        let json = serialize_json_flat(&self.values);
        let tmp_path = format!("{path}.tmp");
        fs::write(&tmp_path, &json)?;
        let _ = fs::remove_file(path);
        fs::rename(&tmp_path, path)?;
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

    pub fn all(&self) -> &HashMap<String, String> {
        &self.values
    }
    pub fn loaded_path(&self) -> Option<&str> {
        self.path.as_deref()
    }
    pub fn dirty(&self) -> bool {
        self.dirty
    }
    pub fn count(&self) -> usize {
        self.values.len()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// JSON 解析器单元测试
// ════════════════════════════════════════════════════════════════════════════

