//! Widget 框架核心：运行时树与上下文。
//!
//! # SMC 边界（SMC-04）
//!
//! widget Module 拥有组件运行时：WidgetTree / WidgetNode / 组件契约
//! （`traits`）、管理器（`managers`）、配置（`config`）、i18n（`locale`）、
//! 剪贴板（`clipboard`）、焦点陷阱（`focus_trap`）与动态文本标签
//! （`dynamic_label`，原 `view::combinators::DynamicLabel`，框架级基础设施
//! 归组件侧避免 widget → widgets / view 依赖）。
//!
//! 窄契约依赖：reactive、event、layout、animation、theme、overlay、
//! accessibility（全部单向；widgets / view 侧不得被本模块引用）。

pub(crate) mod children;
pub(crate) mod paint_context;
pub(crate) mod paint_scope;
pub(crate) mod widget;

pub(crate) mod app_state;
pub(crate) mod build_theme;
// 构建期窗口视口作用域与 @media 断点记录。
pub(crate) mod build_viewport;
pub mod clipboard;
/// 全局组件配置及其作用域访问接口。
pub mod config;
pub(crate) mod dynamic_label;
pub(crate) mod focus_handle;
/// 键盘焦点陷阱的生命周期与树内导航接口。
pub mod focus_trap;
/// 组件本地化语言与翻译资源接口。
pub(crate) mod managers;
pub(crate) mod measurement;
pub(crate) mod provider_context;
pub(crate) mod traits;
pub(crate) mod tree_measure;
pub(crate) mod view_transform;
pub(crate) mod widget_handle;

pub use app_state::AppState;
pub use children::WidgetChildren;
pub use focus_handle::{FocusHandle, FocusHandleError};
pub use paint_context::PaintContext;
pub use widget_handle::WidgetHandle;
