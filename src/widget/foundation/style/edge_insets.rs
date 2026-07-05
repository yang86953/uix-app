//! 外边距/内边距快捷构造（不实现 From 以避免孤儿规则冲突）。

use crate::platform::EdgeInsets;

/// 四边均匀外边距。
pub fn all(v: f32) -> EdgeInsets {
    EdgeInsets::uniform(v)
}

/// 垂直/水平外边距：`[top_bottom, left_right]`。
pub fn symmetric(vertical: f32, horizontal: f32) -> EdgeInsets {
    EdgeInsets::new(horizontal, vertical, horizontal, vertical)
}

/// 四边独立外边距：`[top, right, bottom, left]`。
pub fn trbl(top: f32, right: f32, bottom: f32, left: f32) -> EdgeInsets {
    EdgeInsets::new(left, top, right, bottom)
}
