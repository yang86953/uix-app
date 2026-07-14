//! UIX Animation — 动画模块。
//!
//! 提供泛型动画 `Animation<T>`、缓动曲线 `Easing`（含 antd 5 预设）、
//! 以及进出场过渡系统。
//!
//! ## 架构
//!
//! - `Easing` — 缓动函数，支持 cubic-bezier、弹性和经典预设。
//! - `Animatable` trait — 定义可插值类型（f32, f64, Color, Point, Rect）。
//! - `Animation<T>` — 泛型动画实例，驱动单值从 from→to 的插值。
//! - `AnimationConfig` — 进出场动画公开配置（淡入/滑入/缩放等）。
//! - `TransitionPlayer` — 过渡播放器，见控 widget 的单次进出场动画。
//!
//! ## 集成
//!
//! Widget 通过 `on_update(dt)` 生命周期钩子推进动画帧。
//! `Modal` / `Drawer` 使用 `TransitionPlayer` 实现进出场过渡。
//! 自定义 widget 可直接使用 `Animation<T>` + `on_update(dt)` 实现动画。

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

pub mod core;
pub mod easing;
pub mod transition;

pub use core::*;
pub use easing::*;
pub use transition::*;

// Animatable trait 已迁移至 ui::traits::animation，通过 ui::traits 统一导出。
