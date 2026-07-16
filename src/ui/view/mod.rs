//! # 用户层简化 API — View 体系
//!
//! 本模块提供函数式组合子 API，让用户通过 `View` trait 和 `ViewNode` 声明 UI，
//! 完全不需要了解 `WidgetTree`、`WidgetNode`、`BoxedWidget` 等内部概念。

use crate::core::{EdgeInsets, Point, Rect};
use crate::draw::Color;
use crate::ui::accessibility_override::AccessibilityOverride;
use crate::ui::event::{HandlerRegistration, SemanticEvent, SemanticKind};
use crate::ui::foundation::provider_context::{current_provider_context, ProviderContext};
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::style::{BoxShadowDef, ColorValue, Style, TypographyToken};
use crate::ui::system_event_handler::{SystemEventFilter, SystemEventHandlerRegistration};
use crate::ui::traits::WidgetComponent;
use crate::ui::view_transform::ViewTransform;
use crate::ui::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AriaAttribute, EventResult,
    FocusHandle, SystemEvent,
};

pub(crate) mod adapter;
pub mod combinators;

pub use crate::ui::window_chrome::{
    window_control, window_control_named, window_drag_region, WindowControl,
};
pub(crate) use adapter::ViewAdapter;
pub use combinators::{
    button, canvas, column, column_fit, dynamic_label, embed, grid, input, label, row, scroll,
    show, space, ButtonBuilder, GridBuilder, InputBuilder, IntoLabelContent, IntoViewChildren,
    ScrollBuilder,
};

/// 用户层 UI 声明 trait。
pub trait View: 'static {
    fn build(self) -> ViewNode;
}

/// 中间节点表示。
pub struct ViewNode {
    pub(crate) widget: Box<dyn WidgetComponent>,
    pub(crate) children: Vec<ViewNode>,
    pub(crate) animated_sources: Vec<std::sync::Arc<dyn crate::ui::animation::AnimatedSource>>,
    pub(crate) provider_context: ProviderContext,
    pub(crate) style: Style,
    pub(crate) visual_transform: ViewTransform,
    pub(crate) enter_animation: Option<crate::ui::animation::AnimationConfig>,
    pub(crate) enter_deadline: Option<std::time::Instant>,
    pub(crate) leave_animation: Option<crate::ui::animation::AnimationConfig>,
    pub(crate) stagger_enter: Option<(f64, crate::ui::animation::AnimationConfig)>,
    /// DSL 显式设置的 flex_grow（含 0.0）；与 Style::default 区分，避免被 apply 吞掉。
    pub(crate) flex_grow_override: Option<f32>,
    /// DSL 显式设置的 flex_shrink（含 1.0）。
    pub(crate) flex_shrink_override: Option<f32>,
    pub(crate) z_index: i32,
    pub(crate) key: Option<String>,
    pub(crate) automation_id: Option<String>,
    pub(crate) tab_index: Option<i32>,
    pub(crate) focus_handle: Option<FocusHandle>,
    pub(crate) accessibility_override: Option<AccessibilityOverride>,
    pub(crate) handlers: Vec<HandlerRegistration>,
    pub(crate) system_event_handlers: Vec<SystemEventHandlerRegistration>,
    pub(crate) render_handlers: Vec<RenderHandlerRegistration>,
}

impl View for ViewNode {
    fn build(self) -> ViewNode {
        self
    }
}

impl ViewNode {
    pub(crate) fn widget_type_id(&self) -> std::any::TypeId {
        self.widget.as_any().type_id()
    }

    pub fn leaf(widget: impl WidgetComponent + 'static) -> Self {
        Self {
            widget: Box::new(widget),
            children: vec![],
            animated_sources: Vec::new(),
            provider_context: current_provider_context(),
            style: Style::default(),
            visual_transform: ViewTransform::default(),
            enter_animation: None,
            enter_deadline: None,
            leave_animation: None,
            stagger_enter: None,
            flex_grow_override: None,
            flex_shrink_override: None,
            z_index: 0,
            key: None,
            automation_id: None,
            tab_index: None,
            focus_handle: None,
            accessibility_override: None,
            handlers: Vec::new(),
            system_event_handlers: Vec::new(),
            render_handlers: Vec::new(),
        }
    }

    pub fn new(widget: impl WidgetComponent + 'static, children: Vec<ViewNode>) -> Self {
        Self {
            widget: Box::new(widget),
            children,
            animated_sources: Vec::new(),
            provider_context: current_provider_context(),
            style: Style::default(),
            visual_transform: ViewTransform::default(),
            enter_animation: None,
            enter_deadline: None,
            leave_animation: None,
            stagger_enter: None,
            flex_grow_override: None,
            flex_shrink_override: None,
            z_index: 0,
            key: None,
            automation_id: None,
            tab_index: None,
            focus_handle: None,
            accessibility_override: None,
            handlers: Vec::new(),
            system_event_handlers: Vec::new(),
            render_handlers: Vec::new(),
        }
    }

    pub fn color(mut self, color: impl Into<ColorValue>) -> Self {
        self.style.color = color.into();
        self
    }

    pub fn font_size(mut self, size: impl Into<TypographyToken>) -> Self {
        self.style.font_size = size.into();
        self
    }

    pub fn bg(mut self, color: impl Into<ColorValue>) -> Self {
        self.style.background = Some(color.into());
        self
    }

    /// Binds the background to an existing declarative animation source.
    pub fn bg_animated(self, color: &crate::ui::animation::Animated<Color>) -> Self {
        self.bg(color.value())
    }

    /// 设置指针悬停时的背景色；未设置时沿用普通背景。
    pub fn bg_hover(mut self, color: impl Into<ColorValue>) -> Self {
        self.style.background_hover = Some(color.into());
        self
    }

    /// 设置焦点状态的背景色；未设置时沿用悬停或普通背景。
    pub fn bg_focus(mut self, color: impl Into<ColorValue>) -> Self {
        self.style.background_focus = Some(color.into());
        self
    }

    /// 设置按压或键盘激活期间的背景色；未设置时沿用普通背景。
    pub fn bg_active(mut self, color: impl Into<ColorValue>) -> Self {
        self.style.background_active = Some(color.into());
        self
    }

    pub fn padding(mut self, p: impl Into<EdgeInsets>) -> Self {
        self.style.padding = p.into();
        self
    }

    /// 设置水平内边距，并保留已有的垂直内边距。
    pub fn padding_h(mut self, value: f32) -> Self {
        self.style.padding.left = value;
        self.style.padding.right = value;
        self
    }

    /// 设置垂直内边距，并保留已有的水平内边距。
    pub fn padding_v(mut self, value: f32) -> Self {
        self.style.padding.top = value;
        self.style.padding.bottom = value;
        self
    }

    pub fn margin(mut self, m: impl Into<EdgeInsets>) -> Self {
        self.style.margin = m.into();
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.style.width = Some(w);
        self
    }

    /// Binds width to an existing declarative animation source.
    pub fn width_animated(self, width: &crate::ui::animation::Animated<f32>) -> Self {
        self.width(width.value())
    }

    pub fn height(mut self, h: f32) -> Self {
        self.style.height = Some(h);
        self
    }

    /// Binds height to an existing declarative animation source.
    pub fn height_animated(self, height: &crate::ui::animation::Animated<f32>) -> Self {
        self.height(height.value())
    }

    /// Offsets this node's rendered subtree without changing its layout slot.
    pub fn offset(mut self, offset: Point) -> Self {
        self.visual_transform.offset = offset;
        self
    }

    /// Binds the visual subtree offset to an existing animation source.
    pub fn offset_animated(self, offset: &crate::ui::animation::Animated<Point>) -> Self {
        self.offset(offset.value())
    }

    /// Scales this node's rendered subtree around its layout-frame center.
    pub fn scale(mut self, scale: f32) -> Self {
        self.visual_transform.scale = scale;
        self
    }

    /// Binds the visual subtree scale to an existing animation source.
    pub fn scale_animated(self, scale: &crate::ui::animation::Animated<f32>) -> Self {
        self.scale(scale.value())
    }

    /// Plays a one-shot visual transition when this node is first mounted.
    pub fn enter_animation(mut self, animation: crate::ui::animation::AnimationConfig) -> Self {
        assert!(
            animation.is_enter(),
            "enter_animation requires fade_in, slide_in, or zoom_in"
        );
        self.enter_animation = Some(animation);
        self.enter_deadline = None;
        self
    }

    /// Retains this node for a one-shot visual transition after keyed removal.
    pub fn leave_animation(mut self, animation: crate::ui::animation::AnimationConfig) -> Self {
        assert!(
            animation.is_exit(),
            "leave_animation requires fade_out, slide_out, or zoom_out"
        );
        self.leave_animation = Some(animation);
        self
    }

    /// Staggers one-shot enter transitions for immediate children.
    pub fn stagger_enter(
        mut self,
        interval_secs: f64,
        animation: crate::ui::animation::AnimationConfig,
    ) -> Self {
        assert!(
            animation.is_enter(),
            "stagger_enter requires fade_in, slide_in, or zoom_in"
        );
        let interval_secs = if interval_secs.is_finite() && interval_secs > 0.0 {
            interval_secs
        } else {
            0.0
        };
        self.stagger_enter = Some((interval_secs, animation));
        self
    }

    pub fn flex_grow(mut self, g: f32) -> Self {
        self.style.flex_grow = g;
        self.flex_grow_override = Some(g);
        self
    }

    pub fn flex_shrink(mut self, s: f32) -> Self {
        self.style.flex_shrink = s;
        self.flex_shrink_override = Some(s);
        self
    }

    /// 交叉轴对齐（flex `align-items`）。
    pub fn align(mut self, a: crate::ui::layout::AlignItems) -> Self {
        self.style.align_items = a;
        self
    }

    pub fn align_self(mut self, a: crate::ui::layout::AlignItems) -> Self {
        self.style.align_self = Some(a);
        self
    }

    pub fn grid_cell(mut self, cell: usize) -> Self {
        self.style.grid_cell = Some(cell);
        self
    }

    pub fn grid_span(mut self, columns: u32, rows: u32) -> Self {
        self.style.grid_column_span = columns.max(1);
        self.style.grid_row_span = rows.max(1);
        self
    }

    pub fn gap(mut self, g: f32) -> Self {
        self.style.gap = g;
        self
    }

    /// 保留子项的自然主轴尺寸，用于 ScrollView 的内容容器。
    pub fn overflow_content(mut self) -> Self {
        self.style.overflow_content = true;
        self
    }

    pub fn border(mut self, width: f32, color: impl Into<ColorValue>) -> Self {
        self.style.border_width = EdgeInsets::uniform(width);
        self.style.border_color = Some(color.into());
        self
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.style.border_radius = r;
        self
    }

    /// Binds corner radius to an existing declarative animation source.
    pub fn radius_animated(self, radius: &crate::ui::animation::Animated<f32>) -> Self {
        self.radius(radius.value())
    }

    pub fn opacity(mut self, o: f32) -> Self {
        self.style.opacity = o;
        self
    }

    /// Binds opacity to an existing declarative animation source.
    pub fn opacity_animated(self, opacity: &crate::ui::animation::Animated<f32>) -> Self {
        self.opacity(opacity.value())
    }

    /// Binds foreground/text color to an existing declarative animation source.
    pub fn color_animated(self, color: &crate::ui::animation::Animated<Color>) -> Self {
        self.color(color.value())
    }

    /// 保留节点身份，但让整棵子树退出布局、绘制、命中与焦点候选。
    pub fn visible(mut self, visible: bool) -> Self {
        self.style.visible = visible;
        self
    }

    /// 设置以节点边界为中心、无偏移的盒阴影。
    pub fn shadow(mut self, blur: f32, color: impl Into<Color>) -> Self {
        self.style.box_shadow = Some(BoxShadowDef::new(color.into(), blur, 0.0, 0.0));
        self
    }

    pub fn z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }

    pub fn key(mut self, k: impl Into<String>) -> Self {
        self.key = Some(k.into());
        self
    }

    /// Assigns a stable selector for test automation without affecting
    /// reconciliation identity or runtime behavior.
    pub fn automation_id(mut self, id: impl Into<String>) -> Self {
        self.automation_id = Some(id.into());
        self
    }

    /// 设置 Tab 导航顺序；`0` 表示不进入 Tab 顺序。
    pub fn tab_index(mut self, index: i32) -> Self {
        self.tab_index = Some(index.max(0));
        self
    }

    /// 让节点进入或退出默认 Tab 顺序。
    pub fn focusable(self, focusable: bool) -> Self {
        self.tab_index(if focusable { 1 } else { 0 })
    }

    /// 将应用持有的编程式焦点句柄绑定到该节点。
    pub fn focus_handle(mut self, handle: &FocusHandle) -> Self {
        self.focus_handle = Some(handle.clone());
        self
    }

    /// 完整替换节点对外暴露的无障碍快照。
    pub fn accessibility(mut self, accessibility: AccessibilitySnapshot) -> Self {
        self.accessibility_override = Some(AccessibilityOverride::replace(accessibility));
        self
    }

    /// 覆写节点 role，同时保留组件实时派生的 name 与 state。
    pub fn role(mut self, role: AccessibilityRole) -> Self {
        self.accessibility_override = Some(
            self.accessibility_override
                .take()
                .unwrap_or_default()
                .with_role(role),
        );
        self
    }

    /// 覆写节点可访问名称；空字符串会显式清除组件默认名称。
    pub fn accessible_name(mut self, name: impl Into<String>) -> Self {
        self.accessibility_override = Some(
            self.accessibility_override
                .take()
                .unwrap_or_default()
                .with_name(name),
        );
        self
    }

    /// 覆写节点完整无障碍状态。
    pub fn accessibility_state(mut self, state: AccessibilityState) -> Self {
        self.accessibility_override = Some(
            self.accessibility_override
                .take()
                .unwrap_or_default()
                .with_state(state),
        );
        self
    }

    /// 追加或按名称替换一个 ARIA 属性。
    pub fn aria(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.accessibility_override = Some(
            self.accessibility_override
                .take()
                .unwrap_or_default()
                .with_attribute(AriaAttribute::new(name, value)),
        );
        self
    }

    fn with_system_event_handler(mut self, registration: SystemEventHandlerRegistration) -> Self {
        self.system_event_handlers.push(registration);
        self
    }

    pub fn on_semantic(
        mut self,
        kind: SemanticKind,
        handler: impl FnMut(&mut SemanticEvent) + 'static,
    ) -> Self {
        self.handlers
            .push(HandlerRegistration::new(kind, Box::new(handler)));
        self
    }

    /// 处理命中节点及其冒泡路径上的原始系统事件。
    ///
    /// 返回 [`EventResult::Handled`] 会停止继续冒泡；返回
    /// [`EventResult::NotHandled`] 或 [`EventResult::Bubbled`] 时，框架继续调用
    /// 组件自身 handler 并向父节点传播。每次 reconcile 会用新声明替换旧闭包。
    pub fn on_event(mut self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> Self {
        self.system_event_handlers
            .push(SystemEventHandlerRegistration::new(Box::new(handler)));
        self
    }

    /// 在捕获阶段以及节点自己的目标/冒泡阶段处理所有原始事件。
    pub fn on_event_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> Self {
        self.with_system_event_handler(
            SystemEventHandlerRegistration::new(Box::new(handler)).capture_phase(),
        )
    }

    /// 处理指针 down/up/move/enter/leave 与双击事件。
    ///
    /// 声明该 handler 即明确选择接收命中范围内的连续 `PointerMove`。
    pub fn on_pointer(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> Self {
        self.with_system_event_handler(SystemEventHandlerRegistration::filtered(
            SystemEventFilter::Pointer,
            Box::new(handler),
        ))
    }

    /// 在捕获阶段处理子树指针事件，同时也处理节点自身事件。
    pub fn on_pointer_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> Self {
        self.with_system_event_handler(
            SystemEventHandlerRegistration::filtered(SystemEventFilter::Pointer, Box::new(handler))
                .capture_phase(),
        )
    }

    /// 处理聚焦节点收到的 `KeyDown` / `KeyUp`。
    pub fn on_key(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> Self {
        self.with_system_event_handler(SystemEventHandlerRegistration::filtered(
            SystemEventFilter::Key,
            Box::new(handler),
        ))
    }

    /// 在捕获阶段处理聚焦子树的键盘事件。
    pub fn on_key_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> Self {
        self.with_system_event_handler(
            SystemEventHandlerRegistration::filtered(SystemEventFilter::Key, Box::new(handler))
                .capture_phase(),
        )
    }

    /// 处理节点焦点进入与离开事件。
    pub fn on_focus(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> Self {
        self.with_system_event_handler(SystemEventHandlerRegistration::filtered(
            SystemEventFilter::Focus,
            Box::new(handler),
        ))
    }

    /// 处理命中路径上的滚轮事件。
    pub fn on_scroll(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> Self {
        self.with_system_event_handler(SystemEventHandlerRegistration::filtered(
            SystemEventFilter::Scroll,
            Box::new(handler),
        ))
    }

    pub fn on_semantic_capture<T>(
        mut self,
        kind: SemanticKind,
        state: &crate::ui::state::State<T>,
        handler: impl FnMut(&mut SemanticEvent) + 'static,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        self.handlers
            .push(HandlerRegistration::new(kind, Box::new(handler)).with_state_capture(state));
        self
    }

    pub fn on_semantic_computed_capture<T>(
        mut self,
        kind: SemanticKind,
        computed: &crate::ui::state::Computed<T>,
        handler: impl FnMut(&mut SemanticEvent) + 'static,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(kind, Box::new(handler)).with_computed_capture(computed),
        );
        self
    }

    pub fn on_semantic_window_capture(
        mut self,
        kind: SemanticKind,
        window_id: crate::core::WindowId,
        handler: impl FnMut(&mut SemanticEvent) + 'static,
    ) -> Self {
        self.handlers
            .push(HandlerRegistration::new(kind, Box::new(handler)).with_window_capture(window_id));
        self
    }

    /// 默认点击路径：绑定 `State` 指纹，reconcile 可稳定复用（[使用](docs/使用.md)）。
    ///
    /// 与 [`button`] 的 `on_click` 对齐，可用于 `label` / `embed` 等任意 View。
    pub fn on_click<T, F>(mut self, state: &crate::ui::state::State<T>, mut f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut(&crate::ui::state::State<T>) + 'static,
    {
        let captured = state.clone();
        self.handlers.push(
            HandlerRegistration::new(
                SemanticKind::Click,
                Box::new(move |event| {
                    if event.is_primary_click() {
                        f(&captured);
                    }
                }),
            )
            .with_state_capture(state),
        );
        self
    }

    /// 无 State 的点击闭包；每次 reconcile **保守重绑**（[使用](docs/使用.md)）。
    ///
    /// 命名保留 `_fn`：Rust 无法与 [`Self::on_click`] 重载；有 State 时优先 `on_click(&state, …)`（[使用](docs/使用.md)）。
    pub fn on_click_fn<F: FnMut() + 'static>(mut self, mut f: F) -> Self {
        self.handlers.push(HandlerRegistration::new(
            SemanticKind::Click,
            Box::new(move |event| {
                if event.is_primary_click() {
                    f();
                }
            }),
        ));
        self
    }

    /// 兼容别名：指纹同 [`Self::on_click`]，闭包不接收 `&State`。
    ///
    /// 新代码优先 `on_click(&state, |s| …)`；无 State 用 [`Self::on_click_fn`]。
    #[doc(alias = "on_click")]
    pub fn on_click_capture<T, F>(self, state: &crate::ui::state::State<T>, mut f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut() + 'static,
    {
        self.on_click(state, move |_| f())
    }

    pub fn on_click_window_capture<F>(mut self, window_id: crate::core::WindowId, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(
                SemanticKind::Click,
                Box::new(move |event| {
                    if event.is_primary_click() {
                        f();
                    }
                }),
            )
            .with_window_capture(window_id),
        );
        self
    }

    pub fn on_click_event<F: FnMut(&mut SemanticEvent) + 'static>(mut self, f: F) -> Self {
        self.handlers
            .push(HandlerRegistration::new(SemanticKind::Click, Box::new(f)));
        self
    }

    pub fn on_click_event_capture<T, F>(mut self, state: &crate::ui::state::State<T>, f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut(&mut SemanticEvent) + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(SemanticKind::Click, Box::new(f)).with_state_capture(state),
        );
        self
    }

    pub fn on_click_event_window_capture<F>(
        mut self,
        window_id: crate::core::WindowId,
        f: F,
    ) -> Self
    where
        F: FnMut(&mut SemanticEvent) + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(SemanticKind::Click, Box::new(f))
                .with_window_capture(window_id),
        );
        self
    }
}

impl crate::ui::IntoWidgetNode for ViewNode {
    fn into_node(self) -> crate::ui::core::widget::WidgetNode {
        adapter::ViewAdapter::expand(self)
    }
}

/// 为所有可转换为 [`ViewNode`] 的 builder 提供原始事件声明。
pub trait EventExt: Into<ViewNode> + Sized {
    fn on_event(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_event(handler)
    }

    fn on_event_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> ViewNode {
        self.into().on_event_capture(handler)
    }

    fn on_pointer(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_pointer(handler)
    }

    fn on_pointer_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> ViewNode {
        self.into().on_pointer_capture(handler)
    }

    fn on_key(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_key(handler)
    }

    fn on_key_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> ViewNode {
        self.into().on_key_capture(handler)
    }

    fn on_focus(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_focus(handler)
    }

    fn on_scroll(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_scroll(handler)
    }

    fn tab_index(self, index: i32) -> ViewNode {
        self.into().tab_index(index)
    }

    fn focusable(self, focusable: bool) -> ViewNode {
        self.into().focusable(focusable)
    }

    fn focus_handle(self, handle: &FocusHandle) -> ViewNode {
        self.into().focus_handle(handle)
    }
}

impl<T: Into<ViewNode>> EventExt for T {}

/// 为所有可转换为 [`ViewNode`] 的 builder 提供无障碍声明。
pub trait AccessibilityExt: Into<ViewNode> + Sized {
    fn accessibility(self, accessibility: AccessibilitySnapshot) -> ViewNode {
        self.into().accessibility(accessibility)
    }

    fn role(self, role: AccessibilityRole) -> ViewNode {
        self.into().role(role)
    }

    fn accessible_name(self, name: impl Into<String>) -> ViewNode {
        self.into().accessible_name(name)
    }

    fn accessibility_state(self, state: AccessibilityState) -> ViewNode {
        self.into().accessibility_state(state)
    }

    fn aria(self, name: &'static str, value: impl Into<String>) -> ViewNode {
        self.into().aria(name, value)
    }
}

impl<T: Into<ViewNode>> AccessibilityExt for T {}

/// 为所有可转换为 [`ViewNode`] 的 builder 提供挂载过渡。
pub trait TransitionExt: Into<ViewNode> + Sized {
    fn enter_animation(self, animation: crate::ui::animation::AnimationConfig) -> ViewNode {
        self.into().enter_animation(animation)
    }

    fn leave_animation(self, animation: crate::ui::animation::AnimationConfig) -> ViewNode {
        self.into().leave_animation(animation)
    }

    fn stagger_enter(
        self,
        interval_secs: f64,
        animation: crate::ui::animation::AnimationConfig,
    ) -> ViewNode {
        self.into().stagger_enter(interval_secs, animation)
    }
}

impl<T: Into<ViewNode>> TransitionExt for T {}

/// 为所有 `Into<ViewNode>` 类型提供样式链（[使用](docs/使用.md)）。
///
/// 实现委托 [`ViewNode`] 同名方法，避免双份逻辑漂移。
/// **链式顺序**：先写 builder 专有方法（如 `button(…).primary().on_click(…)`），再写本 trait
/// （`bg` / `padding`…）——一旦进入 `ViewNode`，`primary` 等 builder 方法不可再调。
pub trait StyleExt: Into<ViewNode> + Sized {
    /// Assigns a stable selector for test automation. Put builder-specific
    /// methods before this call because it materializes a [`ViewNode`].
    fn automation_id(self, id: impl Into<String>) -> ViewNode {
        self.into().automation_id(id)
    }

    fn color(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().color(color)
    }

    fn font_size(self, size: impl Into<TypographyToken>) -> ViewNode {
        self.into().font_size(size)
    }

    fn bg(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().bg(color)
    }

    fn bg_animated(self, color: &crate::ui::animation::Animated<Color>) -> ViewNode {
        self.into().bg_animated(color)
    }

    /// 设置指针悬停时的背景色；未设置时沿用普通背景。
    fn bg_hover(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().bg_hover(color)
    }

    /// 设置焦点状态的背景色；未设置时沿用悬停或普通背景。
    fn bg_focus(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().bg_focus(color)
    }

    /// 设置按压或键盘激活期间的背景色；未设置时沿用普通背景。
    fn bg_active(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().bg_active(color)
    }

    fn padding(self, p: impl Into<EdgeInsets>) -> ViewNode {
        self.into().padding(p)
    }

    fn margin(self, m: impl Into<EdgeInsets>) -> ViewNode {
        self.into().margin(m)
    }

    fn width(self, w: f32) -> ViewNode {
        self.into().width(w)
    }

    fn width_animated(self, width: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().width_animated(width)
    }

    fn height(self, h: f32) -> ViewNode {
        self.into().height(h)
    }

    fn height_animated(self, height: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().height_animated(height)
    }

    fn offset(self, offset: Point) -> ViewNode {
        self.into().offset(offset)
    }

    fn offset_animated(self, offset: &crate::ui::animation::Animated<Point>) -> ViewNode {
        self.into().offset_animated(offset)
    }

    fn scale(self, scale: f32) -> ViewNode {
        self.into().scale(scale)
    }

    fn scale_animated(self, scale: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().scale_animated(scale)
    }

    fn flex_grow(self, g: f32) -> ViewNode {
        self.into().flex_grow(g)
    }

    fn flex_shrink(self, s: f32) -> ViewNode {
        self.into().flex_shrink(s)
    }

    fn align(self, a: crate::ui::layout::AlignItems) -> ViewNode {
        self.into().align(a)
    }

    fn align_self(self, a: crate::ui::layout::AlignItems) -> ViewNode {
        self.into().align_self(a)
    }

    fn grid_cell(self, cell: usize) -> ViewNode {
        self.into().grid_cell(cell)
    }

    fn grid_span(self, columns: u32, rows: u32) -> ViewNode {
        self.into().grid_span(columns, rows)
    }

    fn gap(self, g: f32) -> ViewNode {
        self.into().gap(g)
    }

    /// 保留子项的自然主轴尺寸，用于 ScrollView 的内容容器。
    fn overflow_content(self) -> ViewNode {
        self.into().overflow_content()
    }

    fn border(self, width: f32, color: impl Into<ColorValue>) -> ViewNode {
        self.into().border(width, color)
    }

    fn radius(self, r: f32) -> ViewNode {
        self.into().radius(r)
    }

    fn radius_animated(self, radius: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().radius_animated(radius)
    }

    fn opacity(self, o: f32) -> ViewNode {
        self.into().opacity(o)
    }

    fn opacity_animated(self, opacity: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().opacity_animated(opacity)
    }

    fn color_animated(self, color: &crate::ui::animation::Animated<Color>) -> ViewNode {
        self.into().color_animated(color)
    }

    /// 保留节点身份，但让整棵子树退出布局、绘制、命中与焦点候选。
    fn visible(self, visible: bool) -> ViewNode {
        self.into().visible(visible)
    }
}

impl<T: Into<ViewNode>> StyleExt for T {}

/// 简化渲染上下文——用户层进行自定义绘制时使用的 API。
pub struct Ui<'a> {
    pub(crate) fill_rect_fn: Option<Box<dyn FnMut(Rect, Color, Option<f32>) + 'a>>,
    pub(crate) stroke_rect_fn: Option<Box<dyn FnMut(Rect, Color, f32) + 'a>>,
    pub(crate) text_fn: Option<Box<dyn FnMut(&str, Point, Color, f32) + 'a>>,
    pub(crate) text_center_fn: Option<Box<dyn FnMut(&str, Rect, Color, f32) + 'a>>,
    pub(crate) color_fn: Option<Box<dyn FnMut(&str) -> Color + 'a>>,
}

impl<'a> Ui<'a> {
    pub fn fill_rect(&mut self, rect: Rect, color: impl Into<Color>, radius: Option<f32>) {
        if let Some(ref mut f) = self.fill_rect_fn {
            f(rect, color.into(), radius);
        }
    }

    pub fn stroke_rect(&mut self, rect: Rect, color: impl Into<Color>, width: f32) {
        if let Some(ref mut f) = self.stroke_rect_fn {
            f(rect, color.into(), width);
        }
    }

    pub fn text(&mut self, text: &str, pos: Point, color: impl Into<Color>, font_size: f32) {
        if let Some(ref mut f) = self.text_fn {
            f(text, pos, color.into(), font_size);
        }
    }

    pub fn text_center(&mut self, text: &str, rect: Rect, color: impl Into<Color>, font_size: f32) {
        if let Some(ref mut f) = self.text_center_fn {
            f(text, rect, color.into(), font_size);
        }
    }

    pub fn color(&mut self, name: &str) -> Color {
        if let Some(ref mut f) = self.color_fn {
            f(name)
        } else {
            Color::black()
        }
    }
}
