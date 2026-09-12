//! Independent state and layout capabilities, with optional UI tree runtime.
#[cfg(feature = "reactive")]
pub mod reactive;
#[cfg(feature = "reactive")]
pub use reactive::state::{self, Computed, Effect, State, StateSlotId};
#[cfg(feature = "layout")]
pub mod layout;
#[cfg(feature = "layout")]
pub use layout::{
    AlignItems, BoxModel, FlexDirection, FlexLayout, GridLayout, GridTrack, JustifyContent,
    LayoutChild, LayoutEngine, LayoutEngineScratch, LayoutOutput,
};
#[allow(hidden_glob_reexports)]
// SMC-04：Module 为 pub(crate)，公开面收口于本根；glob 再导出仅为根级 API。
#[cfg(feature = "ui")]
pub(crate) mod accessibility;
#[path = "coordination/adapter/mod.rs"]
#[cfg(feature = "ui")]
pub(crate) mod adapter;
#[cfg(feature = "ui")]
pub(crate) mod animation;
#[cfg(feature = "test-harness")]
#[path = "accessibility/actions/automation.rs"]
#[cfg(feature = "ui")]
pub(crate) mod automation;
#[cfg(feature = "ui")]
pub(crate) mod event;
#[allow(hidden_glob_reexports)] // SMC-04：Module 为 pub(crate)，glob 再导出仅为根级 API。
#[path = "theme/i18n.rs"]
#[cfg(feature = "ui")]
pub(crate) mod i18n;
#[allow(hidden_glob_reexports)] // SMC-04：Module 为 pub(crate)，glob 再导出仅为根级 API。
#[cfg(feature = "ui")]
pub mod macros;
#[cfg(feature = "ui")]
pub(crate) mod overlay;
#[path = "coordination/widget_patch.rs"]
#[cfg(feature = "ui")]
pub(crate) mod widget_patch;
#[cfg(feature = "ui")]
pub(crate) mod widget_runtime;
#[cfg(feature = "ui")]
pub(crate) mod widget_snapshot;
// 公开 UI System 自有的布局定位模式与四边值契约。
#[path = "layout/position.rs"]
#[cfg(feature = "ui")]
mod position;

#[path = "coordination/render_handler.rs"]
#[cfg(feature = "ui")]
pub(crate) mod render_handler;
#[path = "accessibility/actions/semantic_action.rs"]
#[cfg(feature = "ui")]
pub(crate) mod semantic_action;
#[path = "theme/painting/style_paint/mod.rs"]
#[cfg(feature = "ui")]
pub(crate) mod style_paint;
// 集中生成并绘制文本装饰线段，保持 draw System 的中性边界。
#[path = "theme/painting/text_decoration.rs"]
#[cfg(feature = "ui")]
pub(crate) mod text_decoration;
// 将 UI 字体族列表单向解析为 draw System 字体句柄。
#[path = "theme/painting/text_family.rs"]
#[cfg(feature = "ui")]
pub(crate) mod text_family;
// 将 UI 字重单向适配为 draw System 的常规或合成粗体字形提交。
#[path = "coordination/dynamic_children.rs"]
#[cfg(feature = "ui")]
mod dynamic_children;
#[path = "event/text_selection/mod.rs"]
#[cfg(feature = "ui")]
pub(crate) mod text_selection;
#[path = "theme/painting/text_weight.rs"]
#[cfg(feature = "ui")]
pub(crate) mod text_weight;
#[cfg(feature = "ui")]
pub(crate) mod theme;
#[path = "layout/transform_origin.rs"]
#[cfg(feature = "ui")]
pub(crate) mod transform_origin;
#[cfg(feature = "ui")]
pub use dynamic_children::{
    ComponentContext, DynamicChildInput, DynamicChildrenCoordinator, DynamicRefresh,
};
#[path = "coordination/tree_widget_hooks.rs"]
#[cfg(feature = "ui")]
pub(crate) mod tree_widget_hooks;
// 公开 UI System 自有的文字选择策略值契约。
#[path = "event/user_select.rs"]
#[cfg(feature = "ui")]
mod user_select;
#[cfg(feature = "ui")]
pub(crate) mod view;

#[cfg(feature = "ui")]
pub use crate::core::WidgetId;
#[cfg(feature = "ui")]
pub use crate::platform::capabilities::StatusLevel;
#[cfg(feature = "ui")]
pub use crate::platform::windowing::{
    ControlSize, CursorType, KeyCode, KeyMod, MouseButton, ScrollDirection,
};
#[cfg(feature = "ui")]
pub use animation::traits::Animatable;
#[cfg(feature = "ui")]
pub use animation::{
    Animated, Animation, AnimationConfig, AnimationGroup, AnimationGroupError, AnimationGroupItem,
    Easing, Keyframe, KeyframeAnimation, KeyframeDirection, KeyframeError, KeyframeFillMode,
    KeyframePlayback, Spring, SpringAnimation, Transition, reduced_motion, set_reduced_motion,
};
#[cfg(feature = "ui")]
pub(crate) use widget_runtime::children;
#[cfg(feature = "ui")]
pub use widget_runtime::clipboard::{copy_to_clipboard, read_text_from_clipboard};
#[cfg(feature = "ui")]
pub use widget_runtime::config::{StyleScope, WidgetTokenOverrides};
#[cfg(feature = "ui")]
pub use widget_runtime::focus_trap::FocusTrap;
#[cfg(feature = "ui")]
pub use widget_runtime::provider_context::{
    ProviderContext, use_context, with_context, with_provider_context,
};
#[cfg(feature = "ui")]
#[cfg(feature = "ui")]
pub use widget_runtime::traits::{
    EventHandler, IntoWidgetNode, TextEditSnapshot, Widget, WidgetAnimation, WidgetCapabilities,
    WidgetLayout, WidgetLifecycle, WidgetRender, WidgetTextInput, WidgetTextSelection,
};
#[cfg(feature = "ui")]
pub(crate) use widget_runtime::widget::WidgetTree;
#[cfg(feature = "ui")]
pub use widget_runtime::{
    AppState, FocusHandle, FocusHandleError, PaintContext, WidgetChildren, WidgetHandle,
};
#[cfg(feature = "ui")]
pub use widget_snapshot::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AriaAttribute, SelectionSnapshot,
    SnapshotField, SnapshotModel, SnapshotSource, SnapshotValue, WidgetConfigSnapshot,
    WidgetSnapshotFields,
};

// 表格 capability 启用时才从 UI 门面导出专属快照列模型。
#[cfg(feature = "ui")]
pub use event::{
    ClickEvent, EventResult, HandlerId, HandlerOptions, HandlerRegistration, HandlerTable,
    SemanticEvent, SemanticKind, SemanticPayload, SystemEvent, SystemEventKind,
};

#[cfg(feature = "ui")]
pub use i18n::{register_translations, set_translations, t_lookup, t_lookup_fmt};

// 导出统一 overlay backdrop 请求及区域策略。
#[cfg(feature = "ui")]
pub use overlay::{
    OverlayBackdropBlur, OverlayBackdropRegion, OverlayEntry, OverlayId, OverlayKind, OverlayStack,
    Placement,
};
// 导出所有 View 节点共享的定位值契约。
#[cfg(feature = "ui")]
pub use position::{PositionInsets, PositionMode};

#[cfg(feature = "ui")]
#[cfg(feature = "ui")]
pub use semantic_action::{SemanticAction, SemanticActionKind};
#[cfg(feature = "ui")]
pub use theme::style::{
    ColorValue, DeclaredStyleSet, StateFlags, Style, StyleDiff, StyleSet, StyleState,
    TypographyToken,
};
// 公开 UI System 自有的背景图层值契约。
#[cfg(feature = "ui")]
pub use theme::style::{
    BackgroundAxisPosition, BackgroundAxisSize, BackgroundImage, BackgroundPosition,
    BackgroundRepeat, BackgroundSize,
};
// 公开 UI System 自有的边框线型契约。
#[cfg(feature = "ui")]
pub use theme::style::BorderStyle;
// 公开 UI System 自有的四角圆角值契约。
#[cfg(feature = "ui")]
pub use theme::style::CornerRadii;
// 公开 UI System 自有的字体族列表值契约。
#[cfg(feature = "ui")]
pub use theme::style::FontFamily;
// 公开 UI System 自有的字体粗细值契约。
#[cfg(feature = "ui")]
pub use theme::style::FontWeight;
// 公开 UI System 自有的行高值契约。
#[cfg(feature = "ui")]
pub use theme::style::LineHeight;
// 公开 UI System 自有的文本水平对齐契约。
#[cfg(feature = "ui")]
pub use theme::style::TextAlign;
// 公开 UI System 自有的文本装饰契约。
#[cfg(feature = "ui")]
pub use theme::style::TextDecoration;
#[cfg(feature = "ui")]
pub use theme::traits::{ThemeTokens, TokenProvider};
#[cfg(feature = "ui")]
pub use theme::{ModeTokens, Theme, TokenPatch, TokenValue};
// 导出由 UI System 拥有的二维变换原点公开值契约。
#[cfg(feature = "ui")]
pub use transform_origin::{TransformOrigin, TransformOriginValue};
// 导出所有 View 节点共享的文字选择策略。
#[cfg(feature = "ui")]
pub use user_select::UserSelect;
#[cfg(feature = "ui")]
pub use view::providers::ContextProvider;
#[cfg(feature = "ui")]
#[cfg(feature = "ui")]
pub use view::{AccessibilityExt, EventExt, StyleExt, TransitionExt, View, ViewNode};

// 公开独立 UIX 窗口控制组合构造器。

// 保持既有公开模块路径（公开面收口前的兼容层）：组件配置 / i18n / 剪贴板 /
// 焦点陷阱 / 响应式状态 / 样式 / 虚拟滚动。

#[cfg(feature = "ui")]
pub use theme::style;

#[cfg(feature = "ui")]
pub use widget_runtime::clipboard;
#[cfg(feature = "ui")]
pub use widget_runtime::config;
#[cfg(feature = "ui")]
pub use widget_runtime::focus_trap;
#[cfg(feature = "ui")]
#[cfg(feature = "ui")]
pub use widget_runtime::managers::{DragManager, FocusManager, InteractionManager, WidgetManagers};
// 为内联组件宏提供窗口私有状态的隐藏运行时实现。
#[doc(hidden)]
#[path = "widget_runtime/state.rs"]
#[cfg(feature = "ui")]
pub mod widget_state;
// 提供 UI 线程窗口循环期间的主题切换请求通道。
#[doc(hidden)]
#[path = "theme/theme_request.rs"]
#[cfg(feature = "ui")]
pub mod theme_request;

// Public widget traits and exported macros mention these opaque bridge
// types. Keep them nameable without making the runtime module hierarchy an
// application-facing API.
#[doc(hidden)]
#[cfg(feature = "ui")]
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
    pub use super::widget_state::{
        UixWidgetScope, uix_keyboard_focus_visible_fact, uix_widget_child_scope, uix_widget_scope,
        uix_widget_state,
    };
    // 重导出生成器与 transition 运行时共用的构建期有效主题令牌读取。
    pub use super::widget_runtime::build_theme::uix_effective_build_tokens;
    // 重导出 @media 生成代码使用的构建期窗口宽度判定。
    pub use super::widget_runtime::build_viewport::uix_media_matches;
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
/// UI 测试支撑公开模块：转发自动化 API 供消费者在 test-harness 构建下编写测试。
#[cfg(feature = "ui")]
pub mod test_harness {
    pub use super::automation::{
        AUTOMATION_DIR_ENV, AUTOMATION_SCHEMA, AutomationAction, AutomationActionKind,
        AutomationError, AutomationErrorCode, AutomationNode, AutomationSelection,
        AutomationSnapshot, AutomationTarget, TestApp,
    };
}

#[cfg(feature = "ui")]
pub use view::combinators::{
    IntoViewChildren, canvas, dynamic_label, elided_label, embed, scoped, show,
};

/// Contracts for separately packaged widget libraries.
#[cfg(feature = "ui")]
pub mod integration;

#[path = "coordination/invalidation.rs"]
#[cfg(feature = "ui")]
mod invalidation;

#[cfg(feature = "ui")]
#[path = "coordination/scene_paint.rs"]
mod scene_paint;

#[cfg(feature = "ui")]
pub(crate) mod primitives;
#[cfg(feature = "ui")]
pub use primitives::{
    Container, IntoLabelContent, Label, ScrollView, TextEditState, TextEditor, column, column_fit,
    label, row,
};

#[cfg(feature = "ui")]
pub use primitives::PrimitiveSnapshot;

#[cfg(feature = "ui")]
pub use primitives::{grid, scroll};

/// Schema-neutral token values and overlays for library-generated code.
#[cfg(feature = "ui")]
pub mod tokens {
    pub use super::{ThemeTokens, TokenPatch, TokenProvider, TokenValue};
}
