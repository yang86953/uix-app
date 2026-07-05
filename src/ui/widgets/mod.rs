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

// ── 扁平重导出（保持 `uix::ui::Button` 等路径不变）──
pub use containers::*;
pub use display::*;
pub use feedback::*;
pub use general::*;
pub use input::*;
pub use navigation::*;
pub use other::*;

// ── 子模块路径兼容（如 `widgets::icon::init_lucide_font`）──
pub use containers::layout as layout;
pub use display::tree as tree;
pub use general::icon as icon;
pub use other::chart as chart;
pub use other::rich_text as rich_text;
pub use other::scroll_view as scroll_view;
