//! 图形引擎 trait 体系（v2 重构）。
//!
//! 架构：
//! - `UpdateStrategy` — 帧更新策略（全屏/脏区/叠加）
//! - `RenderingBackend` — 渲染后端（6 方法，新后端只需实现这些）
//! - `Canvas2D` — 2D 绘制能力（全默认实现委托 Rasterizer）
//! - `GraphicsEngine` — 引擎编排（生命周期 + 帧控制 + Canvas2D 入口）

mod canvas_2d;
mod engine;
mod rendering_backend;
mod update_strategy;

pub use canvas_2d::Canvas2D;
pub use engine::GraphicsEngine;
pub use rendering_backend::RenderingBackend;
pub use update_strategy::UpdateStrategy;
