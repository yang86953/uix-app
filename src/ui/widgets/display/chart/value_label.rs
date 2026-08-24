//! 图表刻度、数值与提示文本使用的无堆分配格式化缓冲。

use std::fmt::{self, Write as _};

// 最大有限浮点数的定点一位文本小于此容量。
pub(crate) struct ChartValueLabel {
    bytes: [u8; 48],
    len: u8,
}

impl ChartValueLabel {
    pub(crate) fn from_f32(value: f32) -> Self {
        let mut label = Self::empty();
        let result = if value == value.trunc() {
            write!(&mut label, "{value:.0}")
        } else {
            write!(&mut label, "{value:.1}")
        };
        debug_assert!(result.is_ok(), "f32 标签必须适配固定栈缓冲");
        label
    }

    #[allow(dead_code, reason = "PieChart 迁移后复用同一无堆分配格式化器")]
    pub(crate) fn from_f64(value: f64) -> Self {
        let mut label = Self::empty();
        let rounded = value.round();
        let result = if (value - rounded).abs() < 0.001 {
            write!(&mut label, "{rounded:.0}")
        } else {
            write!(&mut label, "{value:.1}")
        };
        debug_assert!(result.is_ok(), "f64 标签必须适配固定栈缓冲");
        label
    }

    const fn empty() -> Self {
        Self {
            bytes: [0; 48],
            len: 0,
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        // 写入来源是 Rust 格式化器，始终产生合法 UTF-8。
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap_or("")
    }
}

impl fmt::Write for ChartValueLabel {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let start = usize::from(self.len);
        let end = start.checked_add(value.len()).ok_or(fmt::Error)?;
        let target = self.bytes.get_mut(start..end).ok_or(fmt::Error)?;
        target.copy_from_slice(value.as_bytes());
        self.len = end as u8;
        Ok(())
    }
}
