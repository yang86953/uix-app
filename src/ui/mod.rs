//! 界面能力 — 组件框架、布局、主题、内置组件与声明式 View。
//!
//! # SMC 边界（SMC-04）
//!
//! 本模块是 ui System 的公开边界与私有实现。目标 Module 与依赖
//! （按通用 SMC 定义：窄契约单向无环、能力端口由 System 组装期注入）：
//!
//! | Module | 职责 | 依赖 |
//! |--------|------|------|
//! | [`reactive`] | 响应式状态（State / Computed / Effect）与 Provider 上下文 | 无（只依赖 core / draw） |
//! | [`event`] | 事件模型与注册（SystemEvent / SemanticEvent / EventResult / 系统事件注册） | reactive（handler 指纹捕获，1 处窄契约） |
//! | [`layout`] | 布局引擎与盒模型（Flex / Grid / LayoutChild / BoxModel） | 无（只依赖 core / draw） |
//! | [`animation`] | 动画（Animated / Keyframe / Spring / Transition）与 Animatable 契约 | reactive（Animated 绑定 State） |
//! | [`overlay`] | 浮层注册表与 Placement 定位 | 无（只依赖 core） |
//! | [`theme`] | 主题令牌、设计令牌与 Style 样式系统 | layout（样式引用布局枚举） |
//! | [`accessibility`] | 无障碍语义快照与覆盖 | component（语义树消费）、component_snapshot（类型） |
//! | [`component`] | 组件运行时：WidgetTree / WidgetNode / 组件契约 / 管理器 / 配置与 i18n | reactive、event、layout、animation、theme、overlay、accessibility（全部单向窄契约） |
//! | [`view`] | 声明式 View DSL：View / ViewNode / 扩展 trait / Provider 组件 | component、reactive、event、animation、theme、accessibility |
//! | [`virtualization`] | 虚拟滚动（VirtualScroll / VirtualScrollBuilder） | view、component |
//! | [`widgets`] | 内置组件库与声明式组合子（combinators） | component、view、reactive、event、animation、layout、theme、overlay、accessibility |
//! | [`form`] | 类型化表单模型与表单组件 | widgets、component、view、reactive、event |
//!
//! 兄弟隔离约束（SMC-04）：reactive / layout / overlay 对任何兄弟
//! Module 引用为 0；event / animation / theme / accessibility 只消费依赖基座
//! （reactive / component / layout）；component 不引用 view / widgets / form /
//! virtualization（双向环已拆：build_view_children 归边界端口、Provider 组件归
//! view、tree_measure 收编 child_from_tree、DynamicLabel 归组件侧）；
//! widgets 为消费汇聚点（仅下行引用，无被引用反向边）。
//!
//! System 私有边界（跨 Module 契约与模式外实现，任何 Module 不拥有）：
//!
//! | 归属 | 内容 |
//! |------|------|
//! | [`adapter`] | ViewAdapter：ViewNode → WidgetNode 展开与 reconcile（view/component/widgets 粘合） |
//! | [`render_handler`] | 节点级渲染注册表（表格单元格 / 展开 / 下拉选项 / 虚拟列表项）与空态渲染回调（EmptyRenderer / render_empty_for） |
//! | [`style_paint`] | Style 绘制辅助（跨 component/widgets 的视觉应用，theme::style 再导出） |
//! | [`tree_dynamic`] | 动态内容刷新协调：表格单元格 / 日历格子 / 折叠内容 / Select 选项 / 虚拟列表 |
//! | [`text_selection`] | 跨组件文字拖选协调（Typography / Label / RichText 参与者） |
//! | [`tree_widget_hooks`] | 树对具体组件语义的访问点（Modal 生命周期 / viewport 滚动轴 / 显式尺寸锁 / 导航兄弟联动） |
//! | [`component_snapshot`] | 组件配置与无障碍快照（component 产出、widgets 描述、automation 消费） |
//! | [`semantic_action`] | 语义动作执行器（automation / agent / accessibility 共用） |
//! | [`automation`] | 测试替身（`test-harness` feature） |
//! | [`macros`] | 公开宏定义（`component!` / `views!` / `impl_widget_component!` 等） |
//!
//! 业务流向由 System 编排（公开面收口于本根），Module 之间除上表窄契约外
//! 零依赖；禁止 Module 直接引用、持有、发现或回调兄弟 Module 的私有实现。

#[allow(hidden_glob_reexports)]
// SMC-04：Module 为 pub(crate)，公开面收口于本根；glob 再导出仅为根级 API。
pub(crate) mod accessibility;
pub(crate) mod adapter;
pub(crate) mod animation;
#[cfg(feature = "test-harness")]
pub(crate) mod automation;
pub(crate) mod component;
pub(crate) mod component_patch;
pub(crate) mod component_snapshot;
pub(crate) mod event;
#[allow(hidden_glob_reexports)] // SMC-04：Module 为 pub(crate)，glob 再导出仅为根级 API。
pub(crate) mod form;
pub(crate) mod i18n;
#[allow(hidden_glob_reexports)] // SMC-04：Module 为 pub(crate)，glob 再导出仅为根级 API。
pub(crate) mod layout;
pub mod macros;
pub(crate) mod overlay;
pub(crate) mod reactive;
pub(crate) mod render_handler;
pub(crate) mod semantic_action;
pub(crate) mod style_paint;
pub(crate) mod text_selection;
pub(crate) mod theme;
pub(crate) mod tree_dynamic;
pub(crate) mod tree_widget_hooks;
pub(crate) mod view;
pub(crate) mod virtualization;
pub(crate) mod widgets;

pub use crate::core::ComponentId;
pub use crate::native::capabilities::system::StatusLevel;
pub use crate::native::windowing::input::{
    ControlSize, CursorType, KeyCode, KeyMod, MouseButton, ScrollDirection,
};
pub use animation::traits::Animatable;
pub use animation::{
    Animated, Animation, AnimationConfig, AnimationGroup, AnimationGroupError, AnimationGroupItem,
    Easing, Keyframe, KeyframeAnimation, KeyframeError, Spring, SpringAnimation, Transition,
};
pub(crate) use component::children;
pub use component::clipboard::{copy_to_clipboard, read_text_from_clipboard};
pub use component::config::{
    use_config, with_config, ComponentConfig, ComponentOverrides, ComponentTokenOverrides, Config,
};
pub use component::focus_trap::FocusTrap;
pub use component::locale::{en_us, use_locale, with_locale, zh_cn, Locale};
pub use component::traits::{
    EventHandler, IntoWidgetNode, WidgetAnimation, WidgetCapabilities, WidgetComponent,
    WidgetLayout, WidgetLifecycle, WidgetRender, WidgetTextInput,
};
pub(crate) use component::widget::WidgetTree;
pub use component::{
    AppState, ComponentHandle, FocusHandle, FocusHandleError, PaintContext, WidgetChildren,
};
pub use component_snapshot::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AriaAttribute,
    ComponentConfigSnapshot, SelectionSnapshot, SnapshotCollapsePanel, SnapshotField,
    SnapshotFields, SnapshotSource, SnapshotTransferItem, SnapshotValue,
};
// 树组件 capability 启用时才从 UI 门面导出树节点快照模型。
#[cfg(feature = "tree-widgets")]
// 该类型与 Tree 和 TreeSelect 的快照变体共享同一边界。
pub use component_snapshot::SnapshotTreeNode;
// 表格 capability 启用时才从 UI 门面导出专属快照列模型。
#[cfg(feature = "table")]
// 保持启用场景下既有的两个公开类型路径。
pub use component_snapshot::{SnapshotTableColumn, SnapshotTableColumnGroup};
pub use event::{
    ClickEvent, EventResult, HandlerId, HandlerOptions, HandlerRegistration, HandlerTable,
    SemanticEvent, SemanticKind, SemanticPayload, SystemEvent, SystemEventKind,
};
pub use form::*;
pub use i18n::{register_translations, set_translations, t_lookup, t_lookup_fmt};
pub use layout::{
    AlignItems, BoxModel, FlexDirection, FlexLayout, GridLayout, GridTrack, JustifyContent,
    LayoutChild, LayoutEngine, LayoutOutput,
};
pub use overlay::{OverlayEntry, OverlayId, OverlayKind, OverlayStack, Placement};
pub use reactive::state::{Computed, Effect, State, StateSlotId};
pub use render_handler::{render_empty_for, EmptyContext, EmptyRenderer};
pub use semantic_action::{SemanticAction, SemanticActionKind};
pub use theme::style::{ColorValue, PaletteColor, Style, StyleSet, StyleState, TypographyToken};
pub use theme::traits::{
    IBoxShadowTokens, ISpacingTokens, ITypographyTokens, ThemeTokens, TokenProvider,
};
pub use theme::{
    generate_color_scale, ColorScale, DataVisualizationPalette, DesignTokens, DynTokens,
    FunctionalColorRole, IColorTokens, NeutralColorScale, NeutralRole, PrimaryHue, ShadowToken,
    Theme, ThemePrimitives, TokenPatch, DATA_VISUALIZATION_PALETTE, NEUTRAL_PALETTE,
};
pub use view::providers::ConfigProvider;
pub use view::providers::LocaleProvider;
pub use view::{AccessibilityExt, EventExt, StyleExt, TransitionExt, View, ViewNode};
pub use virtualization::{VirtualScroll, VirtualScrollBuilder};
pub use widgets::combinators::{
    button, canvas, column, column_fit, dynamic_label, embed, grid, input, label, row, scroll,
    show, space, ButtonBuilder, GridBuilder, InputBuilder, IntoLabelContent, IntoViewChildren,
    ScrollBuilder,
};
pub use widgets::window_chrome::{
    window_control, window_control_named, window_drag_region, WindowControl,
};
// 公开标准窗口控制组合构造器。
pub use widgets::window_chrome::window_controls;
pub use widgets::*;

// 保持既有公开模块路径（公开面收口前的兼容层）：组件配置 / i18n / 剪贴板 /
// 焦点陷阱 / 响应式状态 / 样式 / 虚拟滚动。
pub use component::clipboard;
pub use component::config;
pub use component::focus_trap;
pub use component::locale;
pub use component::managers::{
    DragManager, FocusManager, InteractionManager, StateManager, TextManager, WidgetManagers,
};
pub use reactive::state;
pub use theme::style;
pub use virtualization::virtual_scroll;
// 为内联组件宏提供窗口私有状态的隐藏运行时实现。
#[doc(hidden)]
pub mod component_state;
// 提供 UI 线程窗口循环期间的主题切换请求通道。
#[doc(hidden)]
pub mod theme_request;

// Public component traits and exported macros mention these opaque bridge
// types. Keep them nameable without making the runtime module hierarchy an
// application-facing API.
#[doc(hidden)]
pub mod __private {
    /// 宏展开所需的组件契约（macro 路径必须公开；运行时层级保持 pub(crate)）。
    pub mod traits {
        pub use super::super::adapter::ViewChildrenProvider;
        pub use super::super::component::traits::*;
    }
    pub use super::component::widget::{WidgetNode, WidgetTree};
    // 重导出代码生成器使用的私有组件状态桥接。
    pub use super::component_state::{
        uix_component_child_scope, uix_component_scope, uix_component_state, UixComponentScope,
    };
    pub use super::component_snapshot::snapshot_fields_from_any;
    // 重导出 uix-lang setTheme 内置操作使用的主题请求通道。
    pub use super::theme_request::{
        uix_clear_theme_requester, uix_install_theme_requester, uix_set_theme,
    };
}

#[cfg(feature = "test-harness")]
pub mod test_harness {
    pub use super::adapter::ViewAdapter;
    pub use super::automation::{
        AutomationAction, AutomationActionKind, AutomationError, AutomationErrorCode,
        AutomationNode, AutomationSelection, AutomationSnapshot, AutomationTarget, TestApp,
        AUTOMATION_DIR_ENV, AUTOMATION_SCHEMA,
    };
    pub use super::component::widget::{WidgetCore, WidgetTree};
}
