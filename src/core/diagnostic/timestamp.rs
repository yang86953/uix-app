// ============================================================================
// platform/src/diagnostic/timestamp.rs — 轻量时间戳
//
// 提供毫秒精度的时间戳生成与格式化，仅依赖 std::time。
// 替代 chrono crate，用于日志记录、诊断报告等场景。
// ============================================================================

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 自 Unix 纪元以来的毫秒数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(u64);

impl Timestamp {
    /// 创建当前时刻的时间戳。
    pub fn now() -> Self {
        let d = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        Self(d.as_millis() as u64)
    }

    /// 从 SystemTime 转换。
    pub fn from_system_time(t: SystemTime) -> Self {
        let d = t.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
        Self(d.as_millis() as u64)
    }

    /// 内部毫秒值。
    pub fn as_millis(self) -> u64 {
        self.0
    }

    /// 格式化为 `HH:MM:SS`（UTC）。
    pub fn format_time(self) -> String {
        let total_secs = self.0 / 1000;
        let h = (total_secs / 3600) % 24;
        let m = (total_secs / 60) % 60;
        let s = total_secs % 60;
        format!("{h:02}:{m:02}:{s:02}")
    }

    /// 格式化为 `HH:MM:SS.fff`（UTC，毫秒精度）。
    pub fn format_time_ms(self) -> String {
        let total_secs = self.0 / 1000;
        let ms = self.0 % 1000;
        let h = (total_secs / 3600) % 24;
        let m = (total_secs / 60) % 60;
        let s = total_secs % 60;
        format!("{h:02}:{m:02}:{s:02}.{ms:03}")
    }

    /// 格式化为 `YYYY-MM-DDTHH:MM:SS.fffZ`（ISO 8601，UTC）。
    pub fn format_iso_ms(self) -> String {
        let total_secs = self.0 / 1000;
        let ms = self.0 % 1000;
        let days = total_secs / 86400;
        let time_secs = total_secs % 86400;
        let h = (time_secs / 3600) as u32;
        let m = ((time_secs % 3600) / 60) as u32;
        let s = (time_secs % 60) as u32;

        let (y, mo, d) = days_to_date(days);
        format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}.{ms:03}Z")
    }

    /// 格式化为 `YYYY-MM-DD HH:MM:SS UTC`。
    pub fn format_datetime_utc(self) -> String {
        let total_secs = self.0 / 1000;
        let days = total_secs / 86400;
        let time_secs = total_secs % 86400;
        let h = (time_secs / 3600) as u32;
        let m = ((time_secs % 3600) / 60) as u32;
        let s = (time_secs % 60) as u32;

        let (y, mo, d) = days_to_date(days);
        format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02} UTC")
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_iso_ms())
    }
}

impl From<Timestamp> for SystemTime {
    fn from(ts: Timestamp) -> Self {
        UNIX_EPOCH + Duration::from_millis(ts.0)
    }
}

// ── 日期计算 ──────────────────────────────────────────────────────────────

/// 将自 Unix 纪元以来的天数转换为公历日期（年、月、日）。
fn days_to_date(days: u64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era as i64 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y as i32, mo as u32, d as u32)
}

// ── 测试 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "../../tests/core/diagnostic/timestamp.rs"]
mod tests;

