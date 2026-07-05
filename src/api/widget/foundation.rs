//! # 基础能力
//!
//! 状态管理、样式、国际化、配置、管理器与平台辅助类型。

// ── 状态管理 ──
pub use crate::widget::state::{Computed, Effect, State};
pub use crate::widget::style::{Style, StyleVariant};

// ── 国际化 ──
pub use crate::widget::locale::{en_us, use_locale, with_locale, zh_cn, Locale, LocaleProvider};

// ── 配置提供 ──
pub use crate::widget::config::{
    use_config, with_config, ButtonOverrides, ComponentConfig, ComponentOverrides, ConfigProvider,
    FormLayout, FormOverrides, InputOverrides, SelectOverrides,
};

// ── 管理器 ──
pub use crate::widget::managers::{
    DragEventResult, DragManager, EventManager, FocusManager, ImageManager, InteractionManager,
    StateManager, StyleManager, TextManager, WidgetManagers,
};

// ── 剪贴板 ──
pub use crate::widget::clipboard::copy_to_clipboard;

// ── 焦点锁定 ──
pub use crate::widget::focus_trap::FocusTrap;

// ── 虚拟滚动 ──
pub use crate::widget::virtual_scroll::VirtualScroll;

// ── 渲染上下文 ──
pub use crate::widget::scene::RenderContext;

// ── 平台基础类型 ──
pub use crate::platform::{KeyCode, KeyMod, MouseButton};
