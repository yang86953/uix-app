//! UIX Animation — 动画模块。
//!
//! 提供泛型动画 `Animation<T>`、缓动曲线 `Easing`（含 antd 5 预设）、
//! 以及动画驱动引擎 `AnimationDriver`。
//!
//! ## 架构
//!
//! - `Easing` — 缓动函数，支持 cubic-bezier、弹性和经典预设。
//! - `Animatable` trait — 定义可插值类型（f32, f64, Color, Point, Rect）。
//! - `Animation<T>` — 泛型动画实例，独立运行完成回调。
//! - `AnimationDriver` — 驱动引擎，每帧推进所有活跃动画。
//!
//! ## 集成
//!
//! `AnimationManager`（managers/animation_manager.rs）是旧版管理器，
//! 新版 `AnimationDriver` 替代其驱动功能。`AnimationManager` 保持
//! 兼容接口用于 10-manager 系统。

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

pub mod core;
pub mod driver;
pub mod easing;

pub use core::*;
pub use driver::*;
pub use easing::*;

// Re-export Animatable for convenience
pub use easing::Animatable;
