//! UIX Animation — 动画模块。
//!
//! 提供声明式动画值 `Animated<T>`、泛型动画 `Animation<T>`、typed keyframe、Spring、缓动曲线
//! `Easing`（含标准 CSS 预设），以及进出场过渡系统。
//!
//! ## 架构
//!
//! - `Easing` — 缓动函数，支持 cubic-bezier、弹性和经典预设。
//! - `Animatable` trait — 定义可插值类型（f32, f64, Color, Point, Rect）。
//! - `Animated<T>` — 由所属窗口自动推进的固定时长 / keyframe / Spring typed 值过渡、deadline 延迟、循环与一次性完成回调。
//! - `Animation<T>` — 泛型动画实例，驱动单值从 from→to 的插值。
//! - `KeyframeAnimation<T>` — 固定时长的 typed 单值分段插值。
//! - `keyframe!` — 生成逐字段 `Animatable` 聚合类型，让多项属性共享一个 source 与时钟。
//! - `AnimationGroup` — 在不新增 frame registration 的前提下编排有限并行 / 串行 / stagger source。
//! - `SpringAnimation<T>` — 使用阻尼谐振子解析解收敛到目标的单次动画。
//! - `AnimationConfig` — 进出场动画公开配置（淡入/滑入/缩放等）。
//! - `TransitionPlayer` — 过渡播放器，见控 widget 的单次进出场动画。
//!
//! ## 集成
//!
//! Widget 通过 `update_animation(dt)` 生命周期钩子推进动画帧。
//! `Modal` / `Drawer` 使用 `TransitionPlayer` 实现进出场过渡。
//! 自定义 widget 可直接使用 `Animation<T>` + `on_update(dt)` 实现动画。

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

pub(crate) mod animated;
pub(crate) mod core;
pub(crate) mod easing;
pub(crate) mod group;
pub(crate) mod keyframe;
pub(crate) mod spring;
pub(crate) mod traits;
pub(crate) mod transition;

pub use animated::*;
pub use core::*;
pub use easing::*;
pub use group::*;
pub use keyframe::*;
pub use spring::*;
pub use transition::*;

// Animatable trait 已迁移至 ui::traits::animation，通过 ui::traits 统一导出。
