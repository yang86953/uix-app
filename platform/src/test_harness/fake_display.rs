//! Fake 显示 — 完全可配置的显示信息。所有字段通过 Cell 支持 &self 访问。

use std::cell::Cell;
use crate::api::traits::IDisplay;
use crate::geometry::Rect;
use crate::types::DisplayInfo;

#[derive(Debug)]
pub struct FakeDisplay {
    pub dpi_scale: Cell<f32>,
    pub is_dark_mode: Cell<bool>,
    pub count: Cell<i32>,
    /// `info()` 调用记录 (每次调用的 index 参数)
    pub info_calls: std::cell::RefCell<Vec<i32>>,
}

impl FakeDisplay {
    pub fn new() -> Self {
        Self {
            dpi_scale: Cell::new(1.0),
            is_dark_mode: Cell::new(false),
            count: Cell::new(1),
            info_calls: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn set_dpi(&self, scale: f32) {
        self.dpi_scale.set(scale);
    }

    pub fn set_dark_mode(&self, dark: bool) {
        self.is_dark_mode.set(dark);
    }

    pub fn set_count(&self, count: i32) {
        self.count.set(count);
    }

    /// 清除调用记录
    pub fn clear_history(&self) {
        self.info_calls.borrow_mut().clear();
    }

    /// 生成 DisplayInfo（基于当前配置）
    pub fn make_info(&self, index: i32) -> DisplayInfo {
        DisplayInfo {
            bounds: Rect::new(
                index as f32 * 1920.0, 0.0, 1920.0, 1080.0,
            ),
            dpi_scale: self.dpi_scale.get(),
            is_primary: index == 0,
        }
    }
}

impl IDisplay for FakeDisplay {
    fn dpi_scale(&self) -> f32 {
        self.dpi_scale.get()
    }

    fn is_dark_mode(&self) -> bool {
        self.is_dark_mode.get()
    }

    fn count(&self) -> i32 {
        self.count.get()
    }

    fn info(&self, index: i32) -> DisplayInfo {
        self.info_calls.borrow_mut().push(index);
        self.make_info(index)
    }
}
