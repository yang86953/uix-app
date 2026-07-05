//! 呈现协议 — CPU 像素提交与 GPU 上下文生命周期。

use super::error::{Error, Result};

/// CPU/GPU 像素呈现时的屏幕损伤描述。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PresentDamage {
    /// 全屏重绘。
    #[default]
    Full,
    /// 一个或多个局部矩形 `(x, y, w, h)`，设备像素坐标。
    Partial(Vec<(i32, i32, i32, i32)>),
}

impl PresentDamage {
    /// 单矩形局部损伤。
    pub fn single(x: i32, y: i32, w: i32, h: i32) -> Self {
        if w <= 0 || h <= 0 {
            Self::Full
        } else {
            Self::Partial(vec![(x, y, w, h)])
        }
    }

    /// 兼容旧 API：`None` = 全屏，`Some` = 单矩形。
    pub fn from_legacy(dirty_rect: Option<(i32, i32, i32, i32)>) -> Self {
        match dirty_rect {
            None => Self::Full,
            Some((x, y, w, h)) => Self::single(x, y, w, h),
        }
    }

    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
}

/// CPU 像素呈现器接口。
pub trait IPresenter {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<(), Error>;
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
}

/// GPU 图形上下文接口（GL 上下文生命周期管理）。
pub trait IGraphicsContext {
    fn initialize(
        &mut self,
        native_window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> Result<(), Error>;
    fn resize(&mut self, width: i32, height: i32);
    fn make_current(&mut self);
    fn swap_buffers(&mut self);
    fn shutdown(&mut self);
    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Vec<u32>;
    fn width(&self) -> i32;
    fn height(&self) -> i32;

    fn get_proc_address(&self, name: &str) -> Option<*const std::ffi::c_void> {
        let _ = name;
        None
    }
}
