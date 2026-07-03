//! UIX UI — 响应式 Widget 框架。
//!
//! # 快速开始
//!
//! ```ignore
//! use uix::ui::*;
//!
//! // 1. 定义 widget
//! define_widget! {
//!     pub MyWidget {
//!         label: String,
//!     }
//!     @new -> Self { Self { label: "Hello".into() } }
//!
//!     render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
//!         ctx.fill_rect(frame, ctx.tokens().color_primary_bg(), None);
//!         ctx.draw_text(&self.label, frame.origin(), ctx.tokens().color_text(), 14.0);
//!     }
//! }
//!
//! // 2. 组合 widget 树
//! let node = tree! {
//!     Container::new().size(800.0, 600.0) => [
//!         MyWidget::new(),
//!         Button::new("Click").primary(),
//!     ]
//! };
//!
//! // 3. 构建并渲染
//! let mut widget_tree = WidgetTree::new();
//! widget_tree.build(node);
//! ```
//!
//! # 模块结构
//!
//! - [`core`] — Widget 运行时（`widget` 子模块）、上下文与构建器
//! - [`render`] — 渲染管线（事件循环、绘制上下文、图层合成）
//! - [`foundation`] — 基础能力（状态、样式、国际化、剪贴板）
//! - [`widgets`] — 内置组件库（按 Ant Design 分类）
//! - [`layout`] — Flexbox + Grid 布局引擎
//! - [`view`] — 简化声明式 API
//! - [`api`] — 稳定公开契约

extern crate self as uix_ui;

// ── 基础设施 ──────────────────────────────────────────────────
pub mod animation;
pub mod core;
pub mod foundation;
pub mod layout;
pub mod macros;
pub mod managers;
pub mod render;
pub mod theme;

// ── 组件与 API ────────────────────────────────────────────────
pub mod widgets;

// ── 用户层简化 API（View 体系）──
pub mod view;

// ── 稳定公开 API ───────────────────────────────────────────────
pub mod api;

// ── 兼容层：保持旧模块路径不变 ────────────────────────────────
pub use core::widget as widget;
pub use core::{children, context};
pub use foundation::{
    clipboard, config as config_provider, focus_trap, locale, state, style, virtual_scroll,
};
pub mod render_context {
    pub use crate::render::{
        apply_style, DebugRenderService, LayerNode, LayerTree, PaintContext, RenderContext,
        TextRenderService, ThemeSnapshot, resolve_font_size,
    };
}
pub mod layer {
    pub use crate::render::{LayerNode, LayerTree};
}
pub mod debug_render {
    pub use crate::render::DebugRenderService;
}
pub mod text_render {
    pub use crate::render::TextRenderService;
}
pub use render::event_loop as render_loop;

pub use api::*;

// ── 简化 API 便利重导出 ──
pub use view::adapter::ViewAdapter;
pub use view::app::App;
pub use view::{StyleExt, Ui, View, ViewNode};
