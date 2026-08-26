//! 界面能力 — 组件框架、布局、主题、内置组件与声明式 View。
//!
//! # SMC 边界（SMC-04）
//!
//! 本模块是 ui System 的公开边界与私有实现。目标 Module 与依赖
//! （按通用 SMC 定义：窄契约单向无环、能力端口由 System 组装期注入）：
//!
//! | Module | 职责 | 依赖 |
//! |--------|------|------|
//! | `reactive` | 响应式状态（State / Computed / Effect）与 Provider 上下文 | 无（只依赖 core / draw） |
//! | `event` | 事件模型与注册（SystemEvent / SemanticEvent / EventResult / 系统事件注册） | reactive（handler 指纹捕获，1 处窄契约） |
//! | `layout` | 布局引擎与盒模型（Flex / Grid / LayoutChild / BoxModel） | 无（只依赖 core / draw） |
//! | `animation` | 动画（Animated / Keyframe / Spring / Transition）与 Animatable 契约 | reactive（Animated 绑定 State） |
//! | `overlay` | 浮层注册表与 Placement 定位 | 无（只依赖 core） |
//! | `theme` | 主题令牌、设计令牌与 Style 样式系统 | layout（样式引用布局枚举） |
//! | `accessibility` | 无障碍语义快照与覆盖 | widget（语义树消费）、widget_snapshot（类型） |
//! | `widget` | 组件运行时：WidgetTree / WidgetNode / 组件契约 / 管理器 / 配置与 i18n | reactive、event、layout、animation、theme、overlay、accessibility（全部单向窄契约） |
//! | `view` | 声明式 View DSL：View / ViewNode / 扩展 trait / Provider 组件 | widget、reactive、event、animation、theme、accessibility |
//! | `virtualization` | 虚拟滚动（VirtualScroll / VirtualScrollBuilder） | view、widget |
//! | `widgets` | 内置组件库与声明式组合子（combinators） | widget、view、reactive、event、animation、layout、theme、overlay、accessibility |
//! | `form` | 类型化表单模型与表单组件 | widgets、widget、view、reactive、event |
//!
//! 兄弟隔离约束（SMC-04）：reactive / layout / overlay 对任何兄弟
//! Module 引用为 0；event / animation / theme / accessibility 只消费依赖基座
//! （reactive / widget / layout）；widget 不引用 view / widgets / form /
//! virtualization（双向环已拆：build_view_children 归边界端口、Provider 组件归
//! view、tree_measure 收编 child_from_tree、DynamicLabel 归组件侧）；
//! widgets 为消费汇聚点（仅下行引用，无被引用反向边）。
//!
//! System 私有边界（跨 Module 契约与模式外实现，任何 Module 不拥有）：
//!
//! | 归属 | 内容 |
//! |------|------|
//! | `adapter` | ViewAdapter：ViewNode → WidgetNode 展开与 reconcile（view/widget/widgets 粘合） |
//! | `render_handler` | 节点级渲染注册表（表格单元格 / 展开 / 下拉选项 / 虚拟列表项）与空态渲染回调（EmptyRenderer / render_empty_for） |
//! | `style_paint` | Style 绘制辅助（跨 widget/widgets 的视觉应用，theme::style 再导出） |
//! | `tree_dynamic` | 动态内容刷新协调：表格单元格 / 日历格子 / 折叠内容 / Select 选项 / 虚拟列表 |
//! | `text_selection` | 跨组件文字拖选协调（Typography / Label / RichText 参与者） |
//! | `tree_widget_hooks` | 树对具体组件语义的访问点（Modal 生命周期 / viewport 滚动轴 / 显式尺寸锁 / 导航兄弟联动） |
//! | `widget_snapshot` | 组件配置与无障碍快照（widget 产出、widgets 描述、automation 消费） |
//! | `semantic_action` | 语义动作执行器（automation / agent / accessibility 共用） |
//! | `transform_origin` | View 与 widget 共享的公开二维变换原点值契约 |
//! | `automation` | 测试替身（`test-harness` feature） |
//! | [`macros`] | 公开宏定义（`widget!` / `views!` / `impl_widget!` 等） |
//!
//! 业务流向由 System 编排（公开面收口于本根），Module 之间除上表窄契约外
//! 零依赖；禁止 Module 直接引用、持有、发现或回调兄弟 Module 的私有实现。

#[allow(hidden_glob_reexports)]
// SMC-04：Module 为 pub(crate)，公开面收口于本根；glob 再导出仅为根级 API。
pub(crate) mod accessibility;
#[path = "coordination/adapter/mod.rs"]
pub(crate) mod adapter;
pub(crate) mod animation;
#[cfg(feature = "test-harness")]
#[path = "accessibility/actions/automation.rs"]
pub(crate) mod automation;
pub(crate) mod event;
#[allow(hidden_glob_reexports)] // SMC-04：Module 为 pub(crate)，glob 再导出仅为根级 API。
pub(crate) mod form;
#[path = "theme/i18n.rs"]
pub(crate) mod i18n;
#[allow(hidden_glob_reexports)] // SMC-04：Module 为 pub(crate)，glob 再导出仅为根级 API。
pub(crate) mod layout;
pub mod macros;
pub(crate) mod overlay;
#[path = "coordination/widget_patch.rs"]
pub(crate) mod widget_patch;
pub(crate) mod widget_runtime;
pub(crate) mod widget_snapshot;
// 公开 UI System 自有的布局定位模式与四边值契约。
#[path = "layout/position.rs"]
mod position;
pub(crate) mod reactive;
#[path = "coordination/render_handler.rs"]
pub(crate) mod render_handler;
#[path = "accessibility/actions/semantic_action.rs"]
pub(crate) mod semantic_action;
#[path = "theme/painting/style_paint/mod.rs"]
pub(crate) mod style_paint;
// 集中生成并绘制文本装饰线段，保持 draw System 的中性边界。
#[path = "theme/painting/text_decoration.rs"]
pub(crate) mod text_decoration;
// 将 UI 字体族列表单向解析为 draw System 字体句柄。
#[path = "theme/painting/text_family.rs"]
pub(crate) mod text_family;
// 将 UI 字重单向适配为 draw System 的常规或合成粗体字形提交。
#[path = "event/text_selection/mod.rs"]
pub(crate) mod text_selection;
#[path = "theme/painting/text_weight.rs"]
pub(crate) mod text_weight;
pub(crate) mod theme;
#[path = "layout/transform_origin.rs"]
pub(crate) mod transform_origin;
#[path = "coordination/tree_dynamic/mod.rs"]
pub(crate) mod tree_dynamic;
#[path = "coordination/tree_widget_hooks.rs"]
pub(crate) mod tree_widget_hooks;
// 公开 UI System 自有的文字选择策略值契约。
#[path = "event/user_select.rs"]
mod user_select;
pub(crate) mod view;
pub(crate) mod virtualization;
pub(crate) mod widgets;

pub use crate::core::WidgetId;
pub use crate::platform::capabilities::StatusLevel;
pub use crate::platform::windowing::{
    ControlSize, CursorType, KeyCode, KeyMod, MouseButton, ScrollDirection,
};
pub use animation::traits::Animatable;
pub use animation::{
    Animated, Animation, AnimationConfig, AnimationGroup, AnimationGroupError, AnimationGroupItem,
    Easing, Keyframe, KeyframeAnimation, KeyframeDirection, KeyframeError, KeyframeFillMode,
    KeyframePlayback, Spring, SpringAnimation, Transition,
};
pub(crate) use widget_runtime::children;
pub use widget_runtime::clipboard::{copy_to_clipboard, read_text_from_clipboard};
pub use widget_runtime::config::{
    Config, WidgetConfig, WidgetOverrides, WidgetTokenOverrides, use_config, with_config,
};
pub use widget_runtime::focus_trap::FocusTrap;
pub use widget_runtime::locale::{Locale, en_us, use_locale, with_locale, zh_cn};
pub use widget_runtime::traits::{
    EventHandler, IntoWidgetNode, Widget, WidgetAnimation, WidgetCapabilities, WidgetLayout,
    WidgetLifecycle, WidgetRender, WidgetTextInput,
};
pub(crate) use widget_runtime::widget::WidgetTree;
pub use widget_runtime::{
    AppState, FocusHandle, FocusHandleError, PaintContext, WidgetChildren, WidgetHandle,
};
pub use widget_snapshot::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AriaAttribute, SelectionSnapshot,
    SnapshotCollapsePanel, SnapshotField, SnapshotFields, SnapshotSource, SnapshotTransferItem,
    SnapshotValue, WidgetConfigSnapshot,
};
// 树组件 capability 启用时才从 UI 门面导出树节点快照模型。
#[cfg(feature = "tree-widgets")]
// 该类型与 Tree 和 TreeSelect 的快照变体共享同一边界。
pub use widget_snapshot::SnapshotTreeNode;
// 表格 capability 启用时才从 UI 门面导出专属快照列模型。
pub use event::{
    ClickEvent, EventResult, HandlerId, HandlerOptions, HandlerRegistration, HandlerTable,
    SemanticEvent, SemanticKind, SemanticPayload, SystemEvent, SystemEventKind,
};
pub use form::*;
pub use i18n::{register_translations, set_translations, t_lookup, t_lookup_fmt};
pub use layout::{
    AlignItems, BoxModel, FlexDirection, FlexLayout, GridLayout, GridTrack, JustifyContent,
    LayoutChild, LayoutEngine, LayoutEngineScratch, LayoutOutput,
};
#[cfg(feature = "table")]
// 保持启用场景下既有的两个公开类型路径。
pub use widget_snapshot::{SnapshotTableColumn, SnapshotTableColumnGroup};
// 导出统一 overlay backdrop 请求及区域策略。
pub use overlay::{
    OverlayBackdropBlur, OverlayBackdropRegion, OverlayEntry, OverlayId, OverlayKind, OverlayStack,
    Placement,
};
// 导出所有 View 节点共享的定位值契约。
pub use position::{PositionInsets, PositionMode};
pub use reactive::state::{Computed, Effect, State, StateSlotId};
pub use render_handler::{EmptyContext, EmptyRenderer, render_empty_for};
pub use semantic_action::{SemanticAction, SemanticActionKind};
pub use theme::style::{ColorValue, PaletteColor, Style, StyleSet, StyleState, TypographyToken};
// 公开 UI System 自有的背景图层值契约。
pub use theme::style::{
    BackgroundAxisPosition, BackgroundImage, BackgroundPosition, BackgroundRepeat,
};
// 公开 UI System 自有的边框线型契约。
pub use theme::style::BorderStyle;
// 公开 UI System 自有的字体族列表值契约。
pub use theme::style::FontFamily;
// 公开 UI System 自有的字体粗细值契约。
pub use theme::style::FontWeight;
// 公开 UI System 自有的行高值契约。
pub use theme::style::LineHeight;
// 公开 UI System 自有的文本水平对齐契约。
pub use theme::style::TextAlign;
// 公开 UI System 自有的文本装饰契约。
pub use theme::style::TextDecoration;
pub use theme::traits::{
    IBoxShadowTokens, ISpacingTokens, ITypographyTokens, ThemeTokens, TokenProvider,
};
pub use theme::{
    ColorScale, DATA_VISUALIZATION_PALETTE, DataVisualizationPalette, DesignTokens, DynTokens,
    FunctionalColorRole, IColorTokens, NEUTRAL_PALETTE, NeutralColorScale, NeutralRole, PrimaryHue,
    ShadowToken, Theme, ThemePrimitives, TokenPatch, generate_color_scale,
};
// 导出由 UI System 拥有的二维变换原点公开值契约。
pub use transform_origin::{TransformOrigin, TransformOriginValue};
// 导出所有 View 节点共享的文字选择策略。
pub use user_select::UserSelect;
pub use view::providers::ConfigProvider;
pub use view::providers::LocaleProvider;
pub use view::{AccessibilityExt, EventExt, StyleExt, TransitionExt, View, ViewNode};
pub use virtualization::{VirtualScroll, VirtualScrollBuilder};
pub use widgets::combinators::{
    ButtonBuilder, GridBuilder, InputBuilder, IntoLabelContent, IntoViewChildren, ScrollBuilder,
    button, canvas, column, column_fit, dynamic_label, embed, grid, input, label, row, scroll,
    show, space,
};
pub use widgets::window_chrome::{
    WindowControl, WindowResizeEdge, window_control, window_control_named, window_drag_region,
    window_resize_region,
};
// 公开独立 UIX 窗口控制组合构造器。
pub use widgets::window_controls::window_controls;
pub use widgets::*;

// 保持既有公开模块路径（公开面收口前的兼容层）：组件配置 / i18n / 剪贴板 /
// 焦点陷阱 / 响应式状态 / 样式 / 虚拟滚动。
pub use reactive::state;
pub use theme::style;
pub use virtualization::virtual_scroll;
pub use widget_runtime::clipboard;
pub use widget_runtime::config;
pub use widget_runtime::focus_trap;
pub use widget_runtime::locale;
pub use widget_runtime::managers::{
    DragManager, FocusManager, InteractionManager, StateManager, TextManager, WidgetManagers,
};
// 为内联组件宏提供窗口私有状态的隐藏运行时实现。
#[doc(hidden)]
#[path = "widget_runtime/state.rs"]
pub mod widget_state;
// 提供 UI 线程窗口循环期间的主题切换请求通道。
#[doc(hidden)]
#[path = "theme/theme_request.rs"]
pub mod theme_request;

// Public widget traits and exported macros mention these opaque bridge
// types. Keep them nameable without making the runtime module hierarchy an
// application-facing API.
#[doc(hidden)]
pub mod __private {
    /// 宏展开所需的组件契约（macro 路径必须公开；运行时层级保持 pub(crate)）。
    pub mod traits {
        pub use super::super::adapter::ViewChildrenProvider;
        pub use super::super::widget_runtime::traits::*;
    }
    pub use super::widget_runtime::widget::{WidgetNode, WidgetTree};
    // 为独立集成性能测试提供窄构建端口，不公开 System 私有适配器类型。
    #[cfg(feature = "test-harness")]
    pub fn build_view_tree_for_test(view: impl super::view::View) -> WidgetTree {
        super::adapter::ViewAdapter::build(view)
    }
    // 在真实树所有者上执行完整声明协调，保持集成测试与生产入口一致。
    #[cfg(feature = "test-harness")]
    pub fn reconcile_view_tree_for_test(tree: &mut WidgetTree, view: impl super::view::View) {
        super::adapter::ViewAdapter::reconcile(tree, view);
    }
    // 复制当前真实动画源登记，供性能测试复现逐窗调度器持有的工作身份。
    #[cfg(feature = "test-harness")]
    pub fn animated_source_ids_for_test(tree: &WidgetTree) -> Vec<super::WidgetId> {
        tree.animated_source_registrations()
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    }
    // 按逐窗调度器提供的身份推进真实动画源，保留生产路径的采样与失效语义。
    #[cfg(feature = "test-harness")]
    pub fn update_animated_sources_at_for_test(
        tree: &mut WidgetTree,
        ids: &[super::WidgetId],
        now: std::time::Instant,
        dt: f64,
    ) -> Vec<(super::WidgetId, bool)> {
        tree.update_animation_nodes_at(ids, now, dt)
    }
    // 保留优化前几何扩容策略，供同一测试进程执行交替基线。
    #[cfg(feature = "test-harness")]
    pub fn update_animated_sources_geometric_for_test(
        tree: &mut WidgetTree,
        ids: &[super::WidgetId],
        now: std::time::Instant,
        dt: f64,
    ) -> Vec<(super::WidgetId, bool)> {
        let mut updates = Vec::new();
        tree.update_animation_nodes_at_into(ids, now, dt, &mut updates);
        updates
    }
    // 复用显式工作区推进相同来源，供同场景比较临时申请与常驻容量。
    #[cfg(feature = "test-harness")]
    pub fn update_animated_sources_into_for_test(
        tree: &mut WidgetTree,
        ids: &[super::WidgetId],
        now: std::time::Instant,
        dt: f64,
        updates: &mut Vec<(super::WidgetId, bool)>,
    ) {
        tree.update_animation_nodes_at_into(ids, now, dt, updates);
    }
    // 复制直接子节点身份供集成测试核对 keyed 协调是否保持顺序与复用。
    #[cfg(feature = "test-harness")]
    pub fn view_tree_children_for_test(
        tree: &WidgetTree,
        parent: super::WidgetId,
    ) -> Vec<super::WidgetId> {
        tree.get(parent)
            .map(|node| super::widget_runtime::widget::WidgetCore::children(node).to_vec())
            .unwrap_or_default()
    }
    // 只读观察节点最终文字选择策略，供协调集成测试验证继承语义。
    #[cfg(feature = "test-harness")]
    pub fn view_tree_effective_user_select_for_test(
        tree: &WidgetTree,
        id: super::WidgetId,
    ) -> Option<super::UserSelect> {
        tree.get(id).map(|node| node.effective_user_select())
    }
    // 强制缩窄 keyed 摘要供碰撞回退测试使用，并返回旧掩码。
    #[cfg(feature = "test-harness")]
    pub fn set_reconcile_key_fingerprint_mask_for_test(mask: u64) -> u64 {
        super::adapter::set_reconcile_key_fingerprint_mask_for_test(mask)
    }
    // 重导出代码生成器使用的私有组件状态桥接。
    pub use super::widget_snapshot::snapshot_fields_from_any;
    pub use super::widget_state::{
        UixWidgetScope, uix_widget_child_scope, uix_widget_scope, uix_widget_state,
    };
    // 重导出 UIX transition 生成器使用的目标比较与原位重定向桥接。
    pub use super::view::declarative_transition::{
        UixDeclarativeTransition, UixTransitionError, UixTransitionProperty, UixTransitionSpec,
        uix_apply_transition, uix_transition_identity, uix_transition_state,
    };
    // 重导出 uix-lang setTheme 内置操作使用的主题请求通道。
    pub use super::theme_request::{
        uix_clear_theme_requester, uix_install_theme_requester, uix_set_theme,
    };
}

#[cfg(feature = "test-harness")]
// 将测试支撑实现统一存放在根 tests 目录。
#[path = "../../tests/support/ui/test_harness.rs"]
// 保留 UI 测试支撑公开模块的原有契约。
pub mod test_harness;
