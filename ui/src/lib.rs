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
//! # 核心模块
//!
//! - [`widget`] — Widget trait、WidgetTree、事件系统
//! - [`widgets`] — 内置组件库（Button、Input、Modal 等 60+）
//! - [`layout`] — Flexbox + Grid 布局引擎
//! - [`render_context`] — 渲染上下文（文本/图形/主题令牌）
//! - [`spatial`] — 空间坐标系统（`SpatialContext`、物理单位、3D 变换）
//!   通过 `ctx.spatial()` 访问，支持：
//!   - 物理单位：`10.mm()`, `5.cm()`, `12.pt()`
//!   - 3D 变换：`translate()`, `rotate_z()`, `scale()`
//!   - 透视投影：`set_perspective()`, `set_camera_look_at()`
//! - [`state`] — 响应式状态管理（State/Computed/Effect）
//! - [`theme`] — 设计令牌系统
//! - [`animation`] — 动画与过渡系统
//! - [`macros`] — `define_widget!` 与 `tree!` 宏
//! - [`managers`] — 事件焦点/拖拽/动画管理器
//!
//! # 布局方式
//!
//! ```ignore
//! use uix::ui::layout::engine::{FlexLayout, LayoutEngine, LayoutChild};
//! ```

pub mod animation;
pub mod children;
pub mod clipboard;
pub mod config_provider;
pub mod context;
pub mod focus_trap;
pub mod layout;
pub mod layer;
pub mod locale;
pub mod macros;
pub mod managers;
pub mod render_context;
pub mod render_loop;
pub mod state;
pub mod style;
pub mod theme;
pub mod virtual_scroll;
pub mod widget;
pub mod widgets;

// ── 稳定公开 API ───────────────────────────────────────────────
pub mod api;

pub use api::*;
