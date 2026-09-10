//! 桌面光标公开入口；直接复用 UIX 现有光标后端。
use crate::core::{Point, error::Result};
/// 查询桌面光标位置；坐标遵循宿主当前线程的 DPI 上下文。
pub fn cursor_position() -> Result<Point> {
    crate::platform::adapters::desktop_cursor::position()
}
/// 设置桌面光标位置；坐标遵循宿主当前线程的 DPI 上下文。
pub fn set_cursor_position(x: i32, y: i32) -> Result<()> {
    crate::platform::adapters::desktop_cursor::set_position(x, y)
}
