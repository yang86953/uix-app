//! 内置组件库，按 Ant Design 分类组织。
//!
//! 分类与 demo 页面一一对应：通用 / 布局 / 导航 / 输入 / 数据展示 / 反馈 / 其他。

pub mod containers;
pub mod display;
pub mod feedback;
pub mod general;
pub mod input;
pub mod navigation;
pub mod other;

// ── 扁平重导出──
pub use containers::*;
pub use display::*;
pub use feedback::*;
pub use general::*;
pub use input::*;
pub use navigation::*;
pub use other::*;

// ── 常用子模块重导出──
pub use containers::layout;
pub use display::tree;
pub use general::icon;
pub use other::chart;
pub use other::rich_text;
pub use other::scroll_view;
