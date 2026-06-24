//! UIX Core — Foundation types shared across all layers.
//! Point, Size, Rect, EdgeInsets, Color, Animation, and other foundational types.

pub mod animation;
pub mod base;
pub mod geometry;
pub mod types;

// ── 新增：框架级基础词汇 ──
pub mod layout;   // FlexDirection, AlignItems, JustifyContent
pub mod visual;   // Radius, HAlign, VAlign, BlendMode, GradientDirection
pub mod status;   // StatusLevel, ControlSize, ScrollDirection
pub mod event;    // EventResult
pub mod error;    // Errc, ErrorSeverity

mod api;
pub use api::*;
