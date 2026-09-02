//! Error 落地检测的全局兜底观测。
//!
//! 诊断处置标记之外的 [`Error`] 在 Drop 时进入这里：累计计数并按
//! `(code, 消息前缀)` 去重——每个键只发一条 debug 事件，杜绝动态消息
//! 造成的事件风暴；去重键数量设有硬上限，超出部分折叠为溢出键，保证
//! 「错误风暴不能无限增长内存」同样适用于兜底观测自身。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use super::codes::Errc;
use super::types::Error;

/// 去重键的消息前缀长度上限。
const MESSAGE_PREFIX_BYTES: usize = 64;
/// 去重键数量硬上限；超出后折叠为溢出键。
const MAX_DISTINCT_KEYS: usize = 256;
/// 溢出键消息标记。
const OVERFLOW_MARKER: &str = "__unhandled_overflow__";

static TOTAL: AtomicU64 = AtomicU64::new(0);
static DISTINCT: OnceLock<Mutex<HashMap<(Errc, String), u64>>> = OnceLock::new();

fn distinct() -> &'static Mutex<HashMap<(Errc, String), u64>> {
    DISTINCT.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 消息前缀截断：按字符边界截到字节上限，控制字符替换为空格。
fn message_prefix(message: &str) -> String {
    let sanitized: String = message
        .chars()
        .map(|character| if character.is_control() { ' ' } else { character })
        .collect();
    sanitized.char_indices().take_while(|(index, _)| *index < MESSAGE_PREFIX_BYTES).last().map_or(
        String::new(),
        |(index, character)| sanitized[..index + character.len_utf8()].to_string(),
    )
}

pub(crate) fn record_disposed_without_disposition(error: &Error) {
    TOTAL.fetch_add(1, Ordering::Relaxed);
    let mut distinct = distinct()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // 键数达到上限后统一折叠，保证内存上界不受动态消息影响。
    let key = if distinct.len() >= MAX_DISTINCT_KEYS {
        (Errc::Unknown, OVERFLOW_MARKER.to_string())
    } else {
        (error.code(), message_prefix(error.message()))
    };
    let occurrences = distinct.entry(key.clone()).or_insert(0);
    *occurrences += 1;
    if *occurrences == 1 {
        tracing::debug!(
            target: "uix::diagnostics",
            code = %key.0,
            message_prefix = %key.1,
            "error dropped without diagnostics disposition; this may indicate a swallowed failure"
        );
    }
}

/// 返回进程级未处置错误观测摘要：`(累计未处置 drop 次数, 去重键数量)`。
///
/// 供宿主与测试检视「被丢弃而未经任何诊断通道的错误」量级；非零值
/// 意味着调用方（框架或应用）存在绕过诊断系统的错误丢弃点。
pub fn unhandled_error_summary() -> (u64, usize) {
    let total = TOTAL.load(Ordering::Relaxed);
    let distinct = distinct()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    (total, distinct.len())
}
