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
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

// ── 极简 JSON 读写（仅支持扁平 HashMap<String, String>） ────────────────

/// 解析 JSON 对象为 HashMap<String, String>。
/// 仅支持 `{"key": "value", ...}` 格式，值必须是双引号字符串。
pub(crate) fn parse_json_flat(input: &str) -> Result<HashMap<String, String>> {
    let input = input.trim();
    let bytes = input.as_bytes();
    if bytes.first() != Some(&b'{') {
        return Err(Error::new(
            Errc::FormatError,
            "settings: expected JSON object",
        ));
    }

    let mut map = HashMap::new();
    let mut pos = 1usize;
    skip_json_whitespace(bytes, &mut pos);
    if bytes.get(pos) == Some(&b'}') {
        pos += 1;
        skip_json_whitespace(bytes, &mut pos);
        return if pos == bytes.len() {
            Ok(map)
        } else {
            Err(Error::new(
                Errc::FormatError,
                "settings: trailing content after JSON object",
            ))
        };
    }

    loop {
        let key = parse_json_string(input, &mut pos, "key")?;

        // 跳过空白和 ':'
        skip_json_whitespace(bytes, &mut pos);
        if bytes.get(pos) != Some(&b':') {
            return Err(Error::new(Errc::FormatError, "settings: expected ':'"));
        }
        pos += 1;
        skip_json_whitespace(bytes, &mut pos);

        // 解析 value（必须是双引号字符串）
        let value = parse_json_string(input, &mut pos, "value")?;

        map.insert(key, value);

        skip_json_whitespace(bytes, &mut pos);
        match bytes.get(pos) {
            Some(b',') => {
                pos += 1;
                skip_json_whitespace(bytes, &mut pos);
                if bytes.get(pos) == Some(&b'}') {
                    return Err(Error::new(
                        Errc::FormatError,
                        "settings: trailing comma is not valid JSON",
                    ));
                }
            }
            Some(b'}') => {
                pos += 1;
                skip_json_whitespace(bytes, &mut pos);
                return if pos == bytes.len() {
                    Ok(map)
                } else {
                    Err(Error::new(
                        Errc::FormatError,
                        "settings: trailing content after JSON object",
                    ))
                };
            }
            _ => {
                return Err(Error::new(
                    Errc::FormatError,
                    "settings: expected ',' or '}'",
                ));
            }
        }
    }
}

fn skip_json_whitespace(bytes: &[u8], pos: &mut usize) {
    while bytes
        .get(*pos)
        .is_some_and(|byte| matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
    {
        *pos += 1;
    }
}

/// 解析一个 JSON 字符串，并严格验证转义、控制字符和 UTF-16 代理对。
fn parse_json_string(input: &str, pos: &mut usize, role: &str) -> Result<String> {
    let bytes = input.as_bytes();
    if bytes.get(*pos) != Some(&b'"') {
        return Err(Error::new(
            Errc::FormatError,
            format!("settings: expected {role} string"),
        ));
    }

    *pos += 1;
    let mut segment_start = *pos;
    let mut output = String::new();
    while let Some(&byte) = bytes.get(*pos) {
        match byte {
            b'"' => {
                output.push_str(&input[segment_start..*pos]);
                *pos += 1;
                return Ok(output);
            }
            b'\\' => {
                output.push_str(&input[segment_start..*pos]);
                *pos += 1;
                let escaped = bytes.get(*pos).copied().ok_or_else(|| {
                    Error::new(
                        Errc::FormatError,
                        format!("settings: unterminated {role} escape"),
                    )
                })?;
                match escaped {
                    b'"' => output.push('"'),
                    b'\\' => output.push('\\'),
                    b'/' => output.push('/'),
                    b'b' => output.push('\u{0008}'),
                    b'f' => output.push('\u{000c}'),
                    b'n' => output.push('\n'),
                    b'r' => output.push('\r'),
                    b't' => output.push('\t'),
                    b'u' => {
                        *pos += 1;
                        let high = parse_json_hex_quad(bytes, pos, role)?;
                        let scalar = if (0xd800..=0xdbff).contains(&high) {
                            if bytes.get(*pos) != Some(&b'\\') || bytes.get(*pos + 1) != Some(&b'u')
                            {
                                return Err(Error::new(
                                    Errc::FormatError,
                                    format!("settings: incomplete {role} surrogate pair"),
                                ));
                            }
                            *pos += 2;
                            let low = parse_json_hex_quad(bytes, pos, role)?;
                            if !(0xdc00..=0xdfff).contains(&low) {
                                return Err(Error::new(
                                    Errc::FormatError,
                                    format!("settings: invalid {role} surrogate pair"),
                                ));
                            }
                            0x1_0000 + (((high - 0xd800) as u32) << 10) + (low - 0xdc00) as u32
                        } else if (0xdc00..=0xdfff).contains(&high) {
                            return Err(Error::new(
                                Errc::FormatError,
                                format!("settings: unexpected low surrogate in {role}"),
                            ));
                        } else {
                            high as u32
                        };
                        let decoded = char::from_u32(scalar).ok_or_else(|| {
                            Error::new(
                                Errc::FormatError,
                                format!("settings: invalid Unicode scalar in {role}"),
                            )
                        })?;
                        output.push(decoded);
                        segment_start = *pos;
                        continue;
                    }
                    _ => {
                        return Err(Error::new(
                            Errc::FormatError,
                            format!("settings: invalid escape in {role}"),
                        ));
                    }
                }
                *pos += 1;
                segment_start = *pos;
            }
            0x00..=0x1f => {
                return Err(Error::new(
                    Errc::FormatError,
                    format!("settings: unescaped control character in {role}"),
                ));
            }
            _ => *pos += 1,
        }
    }

    Err(Error::new(
        Errc::FormatError,
        format!("settings: unterminated {role}"),
    ))
}

fn parse_json_hex_quad(bytes: &[u8], pos: &mut usize, role: &str) -> Result<u16> {
    let end = pos.saturating_add(4);
    let Some(hex) = bytes.get(*pos..end) else {
        return Err(Error::new(
            Errc::FormatError,
            format!("settings: incomplete Unicode escape in {role}"),
        ));
    };
    if !hex.iter().all(u8::is_ascii_hexdigit) {
        return Err(Error::new(
            Errc::FormatError,
            format!("settings: invalid Unicode escape in {role}"),
        ));
    }
    *pos = end;
    let text = std::str::from_utf8(hex).map_err(|_| {
        Error::new(
            Errc::FormatError,
            format!("settings: invalid Unicode escape in {role}"),
        )
    })?;
    u16::from_str_radix(text, 16).map_err(|_| {
        Error::new(
            Errc::FormatError,
            format!("settings: invalid Unicode escape in {role}"),
        )
    })
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
    let mut entries = map.iter().collect::<Vec<_>>();
    entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
    let mut first = true;
    for (k, v) in entries {
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
#[derive(Debug, Default)]
struct SettingsState {
    path: Option<String>,
    values: HashMap<String, String>,
    dirty: bool,
}

/// 可跨 `AppHandle` 克隆并共享同一份设置状态的键值服务。
#[derive(Debug, Clone, Default)]
pub struct SettingsService {
    state: Arc<RwLock<SettingsState>>,
}

impl SettingsService {
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 JSON 文件加载设置。
    /// 先读入并解析，成功后一次提交 path / values / dirty；失败时保持原状态不变。
    pub fn load(&self, path: &str) -> Result<()> {
        if path.trim().is_empty() {
            return Err(Error::invalid_arg("settings: path must not be empty"));
        }
        if !Path::new(path).exists() {
            let mut state = self.write_state();
            state.path = Some(path.to_string());
            state.values.clear();
            state.dirty = false;
            return Ok(());
        }

        let content = fs::read_to_string(path)?;
        let parsed = if content.trim().is_empty() {
            HashMap::new()
        } else {
            parse_json_flat(&content)?
        };
        let mut state = self.write_state();
        state.path = Some(path.to_string());
        state.values = parsed;
        state.dirty = false;
        Ok(())
    }

    /// 保存设置到 JSON 文件（临时文件 + rename 保证原子性）。
    pub fn save(&self) -> Result<()> {
        let mut state = self.write_state();
        if !state.dirty {
            return Ok(());
        }
        let Some(path) = state.path.as_deref() else {
            return Ok(());
        };
        let json = serialize_json_flat(&state.values);
        let tmp_path = format!("{path}.tmp");
        fs::write(&tmp_path, &json)?;
        let _ = fs::remove_file(path);
        fs::rename(&tmp_path, path)?;
        state.dirty = false;
        Ok(())
    }

    pub fn set(&self, key: &str, value: &str) {
        let mut state = self.write_state();
        if state
            .values
            .get(key)
            .is_some_and(|current| current == value)
        {
            return;
        }
        state.values.insert(key.to_string(), value.to_string());
        state.dirty = true;
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.read_state().values.get(key).cloned()
    }

    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).unwrap_or_else(|| default.to_string())
    }

    pub fn has(&self, key: &str) -> bool {
        self.read_state().values.contains_key(key)
    }

    pub fn remove(&self, key: &str) {
        let mut state = self.write_state();
        if state.values.remove(key).is_some() {
            state.dirty = true;
        }
    }

    pub fn clear(&self) {
        let mut state = self.write_state();
        if state.values.is_empty() {
            return;
        }
        state.values.clear();
        state.dirty = true;
    }

    pub fn all(&self) -> HashMap<String, String> {
        self.read_state().values.clone()
    }

    pub fn loaded_path(&self) -> Option<String> {
        self.read_state().path.clone()
    }

    pub fn dirty(&self) -> bool {
        self.read_state().dirty
    }

    pub fn count(&self) -> usize {
        self.read_state().values.len()
    }

    fn read_state(&self) -> RwLockReadGuard<'_, SettingsState> {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_state(&self) -> RwLockWriteGuard<'_, SettingsState> {
        self.state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// JSON 解析器单元测试
// ════════════════════════════════════════════════════════════════════════════
