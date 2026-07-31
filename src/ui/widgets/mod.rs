//! 内置组件库，按 Ant Design 分类组织。
//!
//! 分类与 demo 页面一一对应：通用 / 布局 / 导航 / 输入 / 数据展示 / 反馈 / 其他。
//!
//! # SMC 边界（SMC-04）
//!
//! widgets Module 拥有具体组件实现与声明式组合子（`combinators`，原
//! `view::combinators`：组合子构建具体组件，归组件侧避免 view → widgets 环）。

pub(crate) mod combinators;
pub mod containers;
pub mod display;
pub mod feedback;
pub mod general;
pub mod input;
pub mod navigation;
pub mod other;
pub(crate) mod window_chrome;

// ── 扁平重导出──
pub use combinators::{button, canvas, column, embed, label, row};
pub use containers::*;
pub use display::*;
pub use feedback::*;
pub use general::*;
pub use input::*;
pub use navigation::*;
pub use other::*;

// ── 常用子模块重导出──
pub use display::tree;
pub use general::icon;
pub use other::chart;
pub use other::rich_text;
pub use other::scroll_view;
