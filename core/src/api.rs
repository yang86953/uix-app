// ============================================================================
// base/api.rs — Base 层的公共 API 出口
//
// 本文件定义 base 层对外暴露的公共接口。其他层只能通过本文件
// 使用 base 层的功能，禁止直接引用内部模块。
// ============================================================================

pub use crate::geometry::{EdgeInsets, Point, Rect, Size};
pub use crate::types::*;

// ── 布局 ──
pub use crate::layout::{AlignItems, FlexDirection, JustifyContent};

// ── 视觉 ──
pub use crate::visual::{BlendMode, GradientDirection, HAlign, Radius, VAlign};

// ── 状态 ──
pub use crate::status::{ControlSize, ScrollDirection, StatusLevel};

// ── 事件 ──
pub use crate::event::EventResult;

// ── 错误 ──
pub use crate::error::{Errc, ErrorSeverity};
