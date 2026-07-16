// ============================================================================
// data/settings/settings.rs — 键值设置服务（JSON 持久化）
//
// 提供扁平 HashMap<String, String> 的 JSON 读写能力。
// 原位于 services crate；现归 data 域，仅作为冷路径配置持久化能力。
// ============================================================================

use crate::core::{Errc, Error, Result};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Display;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[cfg(feature = "settings-serde")]
use serde::{de::DeserializeOwned, Serialize};

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

        match map.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(value);
            }
            Entry::Occupied(entry) => {
                return Err(Error::new(
                    Errc::FormatError,
                    format!("settings: duplicate key '{}'", entry.key()),
                ));
            }
        }

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
    entries.sort_unstable_by_key(|(key, _)| *key);
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

pub(crate) const SETTINGS_TEMP_SUFFIX: &str = ".uix-tmp";
pub(crate) const SETTINGS_BACKUP_SUFFIX: &str = ".uix-bak";

fn settings_sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut sidecar = path.as_os_str().to_os_string();
    sidecar.push(suffix);
    PathBuf::from(sidecar)
}

fn settings_io_error(action: &str, path: &Path, source: std::io::Error) -> Error {
    let source = Error::from(source);
    Error::new(
        source.code(),
        format!(
            "settings: {action} '{}': {}",
            path.display(),
            source.message()
        ),
    )
    .with_source(source)
}

fn existing_settings_file(path: &Path, role: &str, invalid_code: Errc) -> Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(Error::new(
            invalid_code,
            format!("settings: {role} '{}' is not a file", path.display()),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(settings_io_error("inspect", path, error)),
    }
}

/// 恢复上次在替换窗口中断的保存，并清理未提交的临时文件。
fn recover_interrupted_settings_save(path: &Path) -> Result<()> {
    let backup_path = settings_sidecar_path(path, SETTINGS_BACKUP_SUFFIX);
    let temp_path = settings_sidecar_path(path, SETTINGS_TEMP_SUFFIX);
    let target_exists = existing_settings_file(path, "target path", Errc::InvalidArgument)?;
    let backup_exists = existing_settings_file(&backup_path, "backup path", Errc::InvalidState)?;
    let temp_exists = existing_settings_file(&temp_path, "temp path", Errc::InvalidState)?;

    if backup_exists {
        if target_exists {
            fs::remove_file(&backup_path)
                .map_err(|error| settings_io_error("remove stale backup", &backup_path, error))?;
        } else {
            fs::rename(&backup_path, path)
                .map_err(|error| settings_io_error("restore backup", path, error))?;
        }
    }

    if temp_exists {
        fs::remove_file(&temp_path)
            .map_err(|error| settings_io_error("remove stale temp file", &temp_path, error))?;
    }

    Ok(())
}

fn cleanup_temp_after_error(temp_path: &Path, primary: Error) -> Error {
    match fs::remove_file(temp_path) {
        Ok(()) => primary,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => primary,
        Err(error) => Error::new(
            Errc::WriteFailure,
            "settings: save failed and its temp file could not be removed",
        )
        .with_source(settings_io_error("remove temp file", temp_path, error).with_source(primary)),
    }
}

fn write_settings_temp(temp_path: &Path, json: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)
        .map_err(|error| settings_io_error("create temp file", temp_path, error))?;
    let write_result = file
        .write_all(json.as_bytes())
        .and_then(|()| file.sync_all());
    drop(file);

    match write_result {
        Ok(()) => Ok(()),
        Err(error) => {
            let primary = settings_io_error("write and sync temp file", temp_path, error);
            Err(cleanup_temp_after_error(temp_path, primary))
        }
    }
}

/// 使用同目录 temp + backup 提交文件；任一步失败都保留或恢复上一版本。
fn replace_settings_file(path: &Path, json: &str) -> Result<()> {
    recover_interrupted_settings_save(path)?;

    let temp_path = settings_sidecar_path(path, SETTINGS_TEMP_SUFFIX);
    let backup_path = settings_sidecar_path(path, SETTINGS_BACKUP_SUFFIX);
    write_settings_temp(&temp_path, json)?;

    let had_target = match existing_settings_file(path, "target path", Errc::InvalidArgument) {
        Ok(exists) => exists,
        Err(error) => return Err(cleanup_temp_after_error(&temp_path, error)),
    };
    if had_target {
        if let Err(error) = fs::rename(path, &backup_path) {
            let primary = settings_io_error("move current file to backup", path, error);
            return Err(cleanup_temp_after_error(&temp_path, primary));
        }
    }

    if let Err(error) = fs::rename(&temp_path, path) {
        let install_error = settings_io_error("install new file", path, error);
        if had_target {
            if let Err(error) = fs::rename(&backup_path, path) {
                let restore_error = settings_io_error("restore previous file", path, error)
                    .with_source(install_error);
                return Err(Error::new(
                    Errc::WriteFailure,
                    "settings: installing and restoring the settings file both failed",
                )
                .with_source(restore_error));
            }
        }
        return Err(cleanup_temp_after_error(&temp_path, install_error));
    }

    if had_target {
        fs::remove_file(&backup_path)
            .map_err(|error| settings_io_error("remove committed backup", &backup_path, error))?;
    }

    Ok(())
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
        let target_path = Path::new(path);
        recover_interrupted_settings_save(target_path)?;
        if !existing_settings_file(target_path, "target path", Errc::InvalidArgument)? {
            let mut state = self.write_state();
            state.path = Some(path.to_string());
            state.values.clear();
            state.dirty = false;
            return Ok(());
        }

        let content = fs::read_to_string(target_path)
            .map_err(|error| settings_io_error("read", target_path, error))?;
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

    /// 通过已同步的临时文件与可恢复备份提交设置。
    pub fn save(&self) -> Result<()> {
        let mut state = self.write_state();
        if !state.dirty {
            return Ok(());
        }
        let Some(path) = state.path.as_deref() else {
            return Err(Error::invalid_state(
                "settings: save requires a path configured by load",
            ));
        };
        let json = serialize_json_flat(&state.values);
        replace_settings_file(Path::new(path), &json)?;
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

    /// Stores a scalar using its canonical text representation.
    pub fn set_typed<T>(&self, key: &str, value: T)
    where
        T: Display,
    {
        self.set(key, &value.to_string());
    }

    /// Parses a scalar without performing any file I/O.
    pub fn get_typed<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.get(key)
            .map(|value| {
                value.parse::<T>().map_err(|error| {
                    settings_typed_parse_error(key, std::any::type_name::<T>(), error)
                })
            })
            .transpose()
    }

    /// Returns the parsed scalar or `default` when the key is absent.
    pub fn get_typed_or<T>(&self, key: &str, default: T) -> Result<T>
    where
        T: FromStr,
        T::Err: Display,
    {
        Ok(self.get_typed(key)?.unwrap_or(default))
    }

    /// Returns a parsed scalar and reports an absent key as `NotFound`.
    pub fn require_typed<T>(&self, key: &str) -> Result<T>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.get_typed(key)?.ok_or_else(|| {
            Error::new(
                Errc::NotFound,
                format!("settings: required key '{key}' was not found"),
            )
        })
    }

    /// Serializes one structured value into a string-valued settings entry.
    #[cfg(feature = "settings-serde")]
    pub fn set_struct<T>(&self, key: &str, value: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        let encoded = serde_json::to_string(value).map_err(|error| {
            Error::new(
                Errc::SerializationError,
                format!("settings: failed to serialize structured key '{key}': {error}"),
            )
        })?;
        self.set(key, &encoded);
        Ok(())
    }

    /// Deserializes one structured value without performing any file I/O.
    #[cfg(feature = "settings-serde")]
    pub fn get_struct<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        self.get(key)
            .map(|value| {
                serde_json::from_str(&value).map_err(|error| {
                    Error::new(
                        Errc::ParseError,
                        format!(
                            "settings: failed to parse structured key '{key}' as {}: {error}",
                            std::any::type_name::<T>()
                        ),
                    )
                })
            })
            .transpose()
    }

    /// Returns a structured value and reports an absent key as `NotFound`.
    #[cfg(feature = "settings-serde")]
    pub fn require_struct<T>(&self, key: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.get_struct(key)?.ok_or_else(|| {
            Error::new(
                Errc::NotFound,
                format!("settings: required structured key '{key}' was not found"),
            )
        })
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

fn settings_typed_parse_error(key: &str, target_type: &str, error: impl Display) -> Error {
    Error::new(
        Errc::ParseError,
        format!("settings: failed to parse key '{key}' as {target_type}: {error}"),
    )
}

// ════════════════════════════════════════════════════════════════════════════
// JSON 解析器单元测试
// ════════════════════════════════════════════════════════════════════════════
