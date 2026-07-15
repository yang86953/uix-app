// ============================================================================
// core/log/sink.rs — 日志输出目标（Sink）及相关类型
//
// 提供结构化日志记录、多目标输出支持。
// ============================================================================

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Mutex, RwLock};

use super::Level;
use crate::core::diagnostic::Timestamp;

// ════════════════════════════════════════════════════════════════════════════
// 日志记录
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Record {
    pub level: Level,
    pub timestamp: Timestamp,
    pub file: &'static str,
    pub line: u32,
    pub message: String,
    pub attributes: Vec<(String, String)>,
    pub sequence: u64,
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ts = self.timestamp.format_time_ms();
        let filename = Path::new(self.file)
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_else(|| std::borrow::Cow::Borrowed(self.file));

        write!(
            f,
            "{} [{}] {} ({}:{})",
            ts,
            self.level.name(),
            self.message,
            filename,
            self.line
        )?;
        if !self.attributes.is_empty() {
            write!(f, " {{")?;
            for (i, (k, v)) in self.attributes.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}={}", k, v)?;
            }
            write!(f, "}}")?;
        }
        Ok(())
    }
}

impl Record {
    pub fn to_json(&self) -> String {
        let ts = self.timestamp.format_iso_ms();
        let filename = Path::new(self.file)
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_else(|| std::borrow::Cow::Borrowed(self.file));

        let mut json = String::from(r#"{"ts":"#);
        push_json_string(&mut json, &ts);
        json.push_str(r#","level":"#);
        push_json_string(&mut json, self.level.name());
        json.push_str(r#","msg":"#);
        push_json_string(&mut json, &self.message);
        json.push_str(r#","file":"#);
        push_json_string(&mut json, &filename);
        json.push_str(r#","line":"#);
        json.push_str(&self.line.to_string());
        json.push_str(r#","seq":"#);
        json.push_str(&self.sequence.to_string());
        if !self.attributes.is_empty() {
            json.push_str(r#","attrs":{"#);
            for (i, (k, v)) in self.attributes.iter().enumerate() {
                if i > 0 {
                    json.push(',');
                }
                push_json_string(&mut json, k);
                json.push(':');
                push_json_string(&mut json, v);
            }
            json.push('}');
        }
        json.push('}');
        json
    }
}

fn push_json_string(output: &mut String, value: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            control if control <= '\u{1f}' => {
                let code = control as usize;
                output.push_str("\\u00");
                output.push(HEX[(code >> 4) & 0x0f] as char);
                output.push(HEX[code & 0x0f] as char);
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

// ════════════════════════════════════════════════════════════════════════════
// Sink trait
// ════════════════════════════════════════════════════════════════════════════

pub trait Sink: Send + Sync {
    fn write(&self, record: &Record);
    fn flush(&self) {}
    fn set_level(&self, level: Level);
    fn level(&self) -> Level;
    fn passes(&self, level: Level) -> bool {
        level as u8 >= self.level() as u8
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ConsoleSink
// ════════════════════════════════════════════════════════════════════════════

pub struct ConsoleSink {
    level: RwLock<Level>,
}

impl ConsoleSink {
    pub fn new(_use_color: bool) -> Self {
        Self {
            level: RwLock::new(Level::Warn),
        }
    }
    pub fn new_with_level(_use_color: bool, level: Level) -> Self {
        Self {
            level: RwLock::new(level),
        }
    }
    pub fn new_trace(_use_color: bool) -> Self {
        Self {
            level: RwLock::new(Level::Trace),
        }
    }
}

impl Sink for ConsoleSink {
    fn write(&self, record: &Record) {
        let line = format!("{}\n", record);
        if record.level >= Level::Warn {
            let _ = io::Write::write(&mut io::stderr(), line.as_bytes());
        } else {
            let _ = io::Write::write(&mut io::stdout(), line.as_bytes());
        }
    }

    fn flush(&self) {
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();
    }

    fn set_level(&self, level: Level) {
        *self.level.write().unwrap_or_else(|e| e.into_inner()) = level;
    }

    fn level(&self) -> Level {
        *self.level.read().unwrap_or_else(|e| e.into_inner())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// FileSink
// ════════════════════════════════════════════════════════════════════════════

pub struct FileSink {
    path: String,
    level: RwLock<Level>,
    max_size: u64,
    file: Mutex<File>,
}

impl FileSink {
    pub fn new(path: impl Into<String>) -> io::Result<Self> {
        let path = path.into();
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            path,
            level: RwLock::new(Level::Info),
            max_size: 0,
            file: Mutex::new(file),
        })
    }

    pub fn set_max_size(&mut self, bytes: u64) {
        self.max_size = bytes;
    }

    fn rotate(&self, file: &mut File) {
        if self.max_size == 0 {
            return;
        }
        if let Ok(metadata) = file.metadata() {
            if metadata.len() < self.max_size {
                return;
            }
        } else {
            return;
        }

        let oldest = format!("{}.9", self.path);
        if Path::new(&oldest).exists() {
            let _ = fs::remove_file(&oldest);
        }
        for i in (1..9).rev() {
            let old = format!("{}.{}", self.path, i);
            let new = format!("{}.{}", self.path, i + 1);
            if Path::new(&old).exists() {
                let _ = fs::rename(&old, &new);
            }
        }
        let first_backup = format!("{}.1", self.path);
        if fs::rename(&self.path, first_backup).is_err() {
            return;
        }

        let replacement = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!("failed to reopen log file after rotation: {}", e);
                return;
            }
        };
        *file = replacement;
    }
}

impl Sink for FileSink {
    fn write(&self, record: &Record) {
        let line = format!("{}\n", record.to_json());
        let mut file = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let _ = file.write_all(line.as_bytes());
        self.rotate(&mut file);
    }

    fn flush(&self) {
        let _ = self.file.lock().unwrap_or_else(|e| e.into_inner()).flush();
    }

    fn set_level(&self, level: Level) {
        *self.level.write().unwrap_or_else(|e| e.into_inner()) = level;
    }

    fn level(&self) -> Level {
        *self.level.read().unwrap_or_else(|e| e.into_inner())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// CallbackSink
// ════════════════════════════════════════════════════════════════════════════

pub struct CallbackSink {
    level: RwLock<Level>,
    callback: Box<dyn Fn(&Record) + Send + Sync>,
}

impl CallbackSink {
    pub fn new(callback: impl Fn(&Record) + Send + Sync + 'static) -> Self {
        Self {
            level: RwLock::new(Level::Trace),
            callback: Box::new(callback),
        }
    }
}

impl Sink for CallbackSink {
    fn write(&self, record: &Record) {
        (self.callback)(record);
    }

    fn set_level(&self, level: Level) {
        *self.level.write().unwrap_or_else(|e| e.into_inner()) = level;
    }

    fn level(&self) -> Level {
        *self.level.read().unwrap_or_else(|e| e.into_inner())
    }
}
