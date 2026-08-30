//! 内置组件库，按 Ant Design 分类组织。
//!
//! 分类与 demo 页面一一对应：通用 / 布局 / 导航 / 输入 / 数据展示 / 反馈 / 其他。
//!
//! # SMC 边界（SMC-04）
//!
//! widgets Module 拥有具体组件实现与声明式组合子（`combinators`，原
//! `view::combinators`：组合子构建具体组件，归组件侧避免 view → widgets 环）。

use crate::draw::Color;

// 输入、导航和反馈弹层共享的 token 动画颜色辅助不属于 feedback capability。
pub(crate) fn fade_token_color(color: Color, opacity: f32) -> Color {
    // 将动画范围限制到有效不透明度区间。
    let opacity = opacity.clamp(0.0, 1.0);
    // 保留 token 自身基础 alpha，只缩放当前动画进度。
    let alpha = (f32::from(color.a) * opacity).round().clamp(0.0, 255.0) as u8;
    // 返回保持原 RGB 的动画颜色。
    color.with_alpha(alpha)
}

// 输入与导航组件共享的双向绑定原语：依赖捕获与「相等则不写」受控写回。
pub(crate) mod binding;
pub(crate) mod combinators;
pub mod containers;
pub mod display;
// 反馈 capability 启用时才编译完整组件族与全局门面。
#[cfg(feature = "feedback")]
pub mod feedback;
pub mod general;
pub mod input;
// 导航 capability 启用时才编译完整组件族。
#[cfg(feature = "navigation")]
pub mod navigation;
pub mod other;
// 触发方式与提示方位由输入、通用和导航组件共同使用，归基础交互层所有。
mod overlay_types;
// 浮层定位几何（气泡 placement 与垂直下拉）的共享实现。
pub(crate) mod overlay;
// 输入与反馈组件共享提示气泡几何及绘制原语。
pub(crate) mod tooltip_primitives;
pub(crate) mod window_chrome;
pub(crate) mod window_controls;

// ── 扁平重导出──
// 基础组件始终通过 widgets 私有路径消费画布与行组合子。
pub(crate) use combinators::{canvas, row};
// 导航与反馈组件都通过 widgets 私有路径消费按钮组合子。
#[cfg(any(feature = "feedback", feature = "navigation"))]
pub(crate) use combinators::button;
// 只有反馈组件通过 widgets 私有路径消费这些内容组合子。
#[cfg(feature = "feedback")]
pub(crate) use combinators::{column, embed, label};
pub use containers::*;
pub use display::*;
// 反馈 capability 启用时才保留扁平公开导入面。
#[cfg(feature = "feedback")]
pub use feedback::*;
pub use general::*;
pub use input::*;
// 导航 capability 启用时才保留扁平公开导入面。
#[cfg(feature = "navigation")]
pub use navigation::*;
pub use other::*;
// 共享交互枚举不随反馈组件族关闭。
pub use overlay_types::*;

// ── 常用子模块重导出──
// 树组件 capability 启用时才保留兼容子模块路径。
#[cfg(feature = "tree-widgets")]
// 启用后保持 ui::widgets::tree 的既有导入路径。
pub use display::tree;
pub use general::icon;
// 图表 capability 启用时才保留兼容子模块路径。
#[cfg(feature = "charts")]
// 启用后保持 ui::widgets::chart 的既有导入路径。
pub use other::chart;
// 富文本 capability 启用时才保留兼容子模块路径。
#[cfg(feature = "rich-text")]
pub use other::rich_text;
pub use other::scroll_view;
