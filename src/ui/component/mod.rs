//! Widget 框架核心：运行时树与上下文。
//!
//! # SMC 边界（SMC-04）
//!
//! component Module 拥有组件运行时：WidgetTree / WidgetNode / 组件契约
//! （`traits`）、管理器（`managers`）、配置（`config`）、i18n（`locale`）、
//! 剪贴板（`clipboard`）、焦点陷阱（`focus_trap`）与动态文本标签
//! （`dynamic_label`，原 `view::combinators::DynamicLabel`，框架级基础设施
//! 归组件侧避免 component → widgets / view 依赖）。
//!
//! 窄契约依赖：reactive、event、layout、animation、theme、overlay、
//! accessibility（全部单向；widgets / view 侧不得被本模块引用）。

pub mod children;
pub mod paint_context;
pub mod paint_scope;
pub mod widget;

pub(crate) mod app_state;
pub mod clipboard;
pub(crate) mod component_handle;
pub mod config;
pub(crate) mod dynamic_label;
pub(crate) mod focus_handle;
pub mod focus_trap;
pub mod locale;
pub(crate) mod managers;
pub(crate) mod provider_context;
pub(crate) mod traits;
pub(crate) mod tree_measure;
pub(crate) mod view_transform;

pub use app_state::AppState;
pub use children::WidgetChildren;
pub use component_handle::ComponentHandle;
pub use focus_handle::{FocusHandle, FocusHandleError};
pub use paint_context::PaintContext;
