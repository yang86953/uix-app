//! Diagnostics System 拥有的私有 crash Module。
//!
//! 该 Module 拥有三项窄职责：
//! - 有界 [`CrashReport`] 模型，字段经过净化和大小限制；
//! - 原子文件写入（临时文件 + fsync + rename），绝不留下部分报告；
//! - 框架 panic hook：配置了目录时记录崩溃报告，并且总是转发给先前的
//!   hook，保证默认输出与终止语义被保留（panic 永不吞没）。

use std::any::Any;
use std::cell::Cell;
use std::fmt::Write as _;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::{Errc, Error};

use super::Diagnostics;

/// 磁盘崩溃报告布局变更时递增。
pub(crate) const CRASH_SCHEMA_VERSION: u32 = 1;

const MAX_PANIC_MESSAGE_BYTES: usize = 2048;
const MAX_THREAD_BYTES: usize = 256;
const MAX_LOCATION_BYTES: usize = 512;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// 一次进程 panic 的有界、净化快照。
///
/// 每个字段在渲染前都设上限；渲染器永远不会收到无界值。不写入任何
/// 含秘密的数据（环境、参数、报告载荷）。
pub(crate) struct CrashReport {
    pub(crate) schema_version: u32,
    pub(crate) runtime_id: u64,
    pub(crate) occurred_at: SystemTime,
    pub(crate) thread: String,
    pub(crate) panic_message: String,
    pub(crate) panic_location: Option<String>,
    pub(crate) retained_reports: usize,
}

/// 使用 Diagnostics 运行时上下文（runtime id 与留存报告数）把一次
/// panic 捕获进有界模型。
pub(crate) fn capture(diagnostics: &Diagnostics, info: &PanicHookInfo<'_>) -> CrashReport {
    CrashReport {
        schema_version: CRASH_SCHEMA_VERSION,
        runtime_id: diagnostics.runtime_id(),
        occurred_at: SystemTime::now(),
        thread: truncate_utf8(thread_summary().as_str(), MAX_THREAD_BYTES),
        panic_message: truncate_utf8(&payload_summary(info.payload()), MAX_PANIC_MESSAGE_BYTES),
        panic_location: info.location().map(|location| {
            truncate_utf8(
                &format!(
                    "{}:{}:{}",
                    file_basename(location.file()),
                    location.line(),
                    location.column()
                ),
                MAX_LOCATION_BYTES,
            )
        }),
        retained_reports: diagnostics.snapshot().reports().len(),
    }
}

/// 渲染带净化值的行式报告。
///
/// 控制字符（含换行）被替换为空格，保证输出保持一行一字段，
/// 不会与另一份报告混淆。
pub(crate) fn render(report: &CrashReport) -> String {
    let mut out = String::with_capacity(1024);
    let _ = writeln!(out, "uix-crash-report");
    push_field(
        &mut out,
        "schema_version",
        &report.schema_version.to_string(),
    );
    push_field(&mut out, "runtime_id", &report.runtime_id.to_string());
    let (secs, millis) = unix_components(report.occurred_at);
    push_field(&mut out, "occurred_at_unix_secs", &secs.to_string());
    push_field(&mut out, "occurred_at_unix_millis", &millis.to_string());
    push_field(&mut out, "thread", &report.thread);
    if let Some(location) = &report.panic_location {
        push_field(&mut out, "panic_location", location);
    }
    push_field(&mut out, "panic_message", &report.panic_message);
    push_field(
        &mut out,
        "retained_reports",
        &report.retained_reports.to_string(),
    );
    out
}

fn push_field(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push('=');
    for character in value.chars() {
        if character.is_control() {
            out.push(' ');
        } else {
            out.push(character);
        }
    }
    out.push('\n');
}

/// 把报告原子写入 `directory` 并返回最终路径。
///
/// 内容先写入唯一临时文件、fsync，再 rename 到最终位置。写入失败会删除
/// 临时文件，绝不留下部分报告。最终文件名冲突时替换先前报告。
pub(crate) fn write_atomic(directory: &Path, report: &CrashReport) -> Result<PathBuf, Error> {
    write_text_atomic(
        directory,
        "crash",
        report.occurred_at,
        report.runtime_id,
        &render(report),
    )
}

/// 为 Diagnostics 私有产物复用同一套原子落盘契约。
pub(super) fn write_text_atomic(
    directory: &Path,
    kind: &'static str,
    occurred_at: SystemTime,
    runtime_id: u64,
    contents: &str,
) -> Result<PathBuf, Error> {
    std::fs::create_dir_all(directory).map_err(|error| {
        Error::new(
            Errc::IoError,
            format!(
                "diagnostic artifact directory unavailable: {}",
                directory.display()
            ),
        )
        .with_source(Error::from(error))
    })?;

    let (secs, millis) = unix_components(occurred_at);
    let final_path = directory.join(format!("uix-{kind}-{secs}-{millis}-{runtime_id}.txt"));
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let tmp_path = directory.join(format!(
        ".uix-{kind}-{}-{sequence}-{secs}.tmp",
        std::process::id()
    ));

    let write_result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp_path)?;
        std::io::Write::write_all(&mut file, contents.as_bytes())?;
        file.sync_all()?;
        drop(file);
        match std::fs::rename(&tmp_path, &final_path) {
            Ok(()) => Ok(()),
            Err(_) => {
                // rename 到已存在目标在部分平台会失败；删除旧报告后重试，
                // 保证同一崩溃不残留两份伪完整文件。
                std::fs::remove_file(&final_path)?;
                std::fs::rename(&tmp_path, &final_path)
            }
        }
    })();

    match write_result {
        Ok(()) => Ok(final_path),
        Err(error) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(Error::new(
                Errc::IoError,
                format!("diagnostic artifact write failed: {}", final_path.display()),
            )
            .with_source(Error::from(error)))
        }
    }
}

thread_local! {
    static IN_PANIC_HOOK: Cell<bool> = const { Cell::new(false) };
}

struct PanicHookGuard;

impl PanicHookGuard {
    fn enter() -> Option<Self> {
        IN_PANIC_HOOK.with(|running| {
            if running.replace(true) {
                None
            } else {
                Some(Self)
            }
        })
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        IN_PANIC_HOOK.with(|running| running.set(false));
    }
}

/// 为一个 Diagnostics 运行时安装框架 panic hook。
///
/// 仅当配置了崩溃目录时 hook 才写崩溃报告；否则行为与先前 hook 完全一致。
/// 它永不吞没 panic：先前 hook 总是被调用，因此默认输出与 unwind/abort
/// 终止语义被保留。hook 自身内部发生 panic（包括写入中的二次失败）时
/// 有递归防护，且仍安全终止。
pub(crate) fn install_panic_hook(diagnostics: Diagnostics) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // 递归防护：hook 内再次 panic 时不再尝试写报告，直接打印并返回。
        let Some(_guard) = PanicHookGuard::enter() else {
            eprintln!("uix: panic occurred inside the panic hook; skipping crash report");
            return;
        };
        // 配置了目录才捕获：原子写入崩溃报告。
        if let Some(directory) = diagnostics.crash_report_directory() {
            match write_atomic(directory, &capture(&diagnostics, info)) {
                Ok(path) => eprintln!(
                    "uix: panic captured; crash report written to {}",
                    path.display()
                ),
                Err(error) => eprintln!(
                    "uix: panic hook could not write crash report: {}",
                    error.short_what()
                ),
            }
            match diagnostics.write_debug_repro_manifest_for_panic(directory) {
                Ok(path) => eprintln!(
                    "uix: debug reproduction manifest written to {}",
                    path.display()
                ),
                Err(error) => eprintln!(
                    "uix: panic hook could not write debug reproduction manifest: {}",
                    error.short_what()
                ),
            }
        }
        // 始终转发给先前 hook，保证默认输出与终止语义。
        previous(info);
    }));
}

fn payload_summary(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        // 非字符串 payload 无法安全还原为可读文本；以固定标记代替，
        // 不把未知内容当作 panic 信息写出。
        "non-string panic payload".to_string()
    }
}

fn thread_summary() -> String {
    let thread = std::thread::current();
    match thread.name() {
        Some(name) => format!("{name}:{:?}", thread.id()),
        None => format!("{:?}", thread.id()),
    }
}

fn file_basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn truncate_utf8(input: &str, limit: usize) -> String {
    if input.len() <= limit {
        return input.to_string();
    }
    let mut end = limit;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    input[..end].to_string()
}

fn unix_components(time: SystemTime) -> (u64, u64) {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs(), u64::from(duration.subsec_millis())),
        Err(_) => (0, 0),
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../tests/unit/diagnostics/crash__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
