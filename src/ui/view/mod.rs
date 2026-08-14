//! # 用户层简化 API — View 体系
//!
//! 本模块提供函数式组合子 API，让用户通过 `View` trait 和 `ViewNode` 声明 UI，
//! 完全不需要了解 `WidgetTree`、`WidgetNode`、`BoxedWidget` 等内部概念。

use crate::core::{EdgeInsets, Point};
use crate::draw::Color;
use crate::ui::accessibility::accessibility_override::AccessibilityOverride;
use crate::ui::component::provider_context::{current_provider_context, ProviderContext};
// 让声明节点携带内联组件的非视觉状态作用域标记。
use crate::ui::component_state::{
    // 引入随声明根延迟提交的私有状态写入回执。
    ComponentStateCaptureReceipt,
    // 引入窗口私有状态存储和组件作用域标记。
    ComponentStateStore, UixComponentScope, UixComponentScopeMarker,
};
use crate::ui::component::traits::WidgetComponent;
use crate::ui::component::view_transform::ViewTransform;
use crate::ui::event::system_event_handler::{SystemEventFilter, SystemEventHandlerRegistration};
use crate::ui::event::{HandlerRegistration, SemanticEvent, SemanticKind};
use crate::ui::render_handler::RenderHandlerRegistration;
// 引入声明根显式交接的 State 绑定与 Effect 类型。
use crate::ui::reactive::state::{Effect, StatePaintBind};
use crate::ui::theme::style::{BoxShadowDef, ColorValue, Style, TypographyToken};
use crate::ui::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AriaAttribute, EventResult,
    FocusHandle, SystemEvent,
};
// 引入声明根保存动态绑定句柄所需的共享指针。
use std::sync::Arc;

pub(crate) mod providers;

/// 用户层 UI 声明 trait。
pub trait View: 'static {
    fn build(self) -> ViewNode;
}

/// 中间节点表示。
pub struct ViewNode {
    pub(crate) widget: Box<dyn WidgetComponent>,
    pub(crate) children: Vec<ViewNode>,
    // 保存当前捕获根读取的结构性 State 绑定，建树时由所属 WidgetTree 提交。
    pub(crate) captured_state_binds: Vec<Arc<dyn StatePaintBind>>,
    // 保存当前捕获根创建的 Effect，协调时由所属 WidgetTree 整体替换。
    pub(crate) captured_effects: Vec<Effect>,
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
    // 保存承载此根的全部内联组件作用域，不参与视觉或语义快照。
    pub(crate) uix_component_scopes: Vec<UixComponentScopeMarker>,
    // 仅由捕获根携带，用于把首次构建绑定到同一窗口状态存储。
    pub(crate) component_state_store: Option<ComponentStateStore>,
    // 保存尚未由成功树事务接纳的组件状态写入回执。
    pub(crate) component_state_receipts: Vec<ComponentStateCaptureReceipt>,
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
            // 非捕获构造路径不携带结构性 State 绑定。
            captured_state_binds: Vec::new(),
            // 非捕获构造路径不携带根 Effect。
            captured_effects: Vec::new(),
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
            uix_component_scopes: Vec::new(),
            component_state_store: None,
            // 非捕获构造路径没有需要延迟提交的私有状态写入。
            component_state_receipts: Vec::new(),
        }
    }

    pub fn new(widget: impl WidgetComponent + 'static, children: Vec<ViewNode>) -> Self {
        Self {
            widget: Box::new(widget),
            children,
            // 非捕获构造路径不携带结构性 State 绑定。
            captured_state_binds: Vec::new(),
            // 非捕获构造路径不携带根 Effect。
            captured_effects: Vec::new(),
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
            uix_component_scopes: Vec::new(),
            component_state_store: None,
            // 非捕获构造路径没有需要延迟提交的私有状态写入。
            component_state_receipts: Vec::new(),
        }
    }

    /// 为内联组件展开根追加非视觉的私有状态作用域标记。
    #[doc(hidden)]
    pub fn uix_component_scope(
        mut self,
        scope: UixComponentScope,
        root_ordinal: u64,
    ) -> Self {
        // 追加而非覆盖，以保留多个内联组件共享同一实际根的身份。
        self.uix_component_scopes
            .push(UixComponentScopeMarker::new(scope, root_ordinal));
        // 返回携带完整作用域栈的原节点。
        self
    }

    // 由捕获适配器绑定首次构建的窗口私有状态存储。
    pub(crate) fn set_component_state_store(&mut self, store: ComponentStateStore) {
        // 仅根节点需要保存存储所有权线索。
        self.component_state_store = Some(store);
    }

    // 让捕获适配器把一次组件状态 journal 的所有权附到声明根。
    pub(crate) fn push_component_state_receipt(&mut self, receipt: ComponentStateCaptureReceipt) {
        // 保持捕获顺序，以便统一在树事务成功后接纳全部写入。
        self.component_state_receipts.push(receipt);
    }

    pub fn color(mut self, color: impl Into<ColorValue>) -> Self {
        self.style.color = color.into();
        self
    }

    pub fn font_size(mut self, size: impl Into<TypographyToken>) -> Self {
        self.style.font_size = size.into();
        self
    }

    /// 通过受控闭包精确更新节点样式，供声明式转换器保留未声明字段。
    pub fn map_style(mut self, update: impl FnOnce(&mut Style)) -> Self {
        // 只把当前节点的样式借给同步更新闭包。
        update(&mut self.style);
        // 返回完成样式更新的节点。
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

    /// 声明式进场动画（`Transition` 紧凑写法）：等价于 `enter_animation`。
    pub fn enter(self, transition: crate::ui::animation::Transition) -> Self {
        self.enter_animation(transition.into())
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

    /// 声明式离场动画（`Transition` 紧凑写法）：等价于 `leave_animation`。
    pub fn leave(self, transition: crate::ui::animation::Transition) -> Self {
        self.leave_animation(transition.into())
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

    /// 主轴对齐（flex `justify-content`）。
    pub fn justify(mut self, j: crate::ui::layout::JustifyContent) -> Self {
        self.style.justify_content = j;
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
        state: &crate::ui::reactive::state::State<T>,
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
        computed: &crate::ui::reactive::state::Computed<T>,
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

    /// 默认点击路径：绑定 `State` 指纹，reconcile 可稳定复用（公开用法见仓库 `docs/使用/事件.md`）。
    ///
    /// 与 [`crate::ui::widgets::general::button::Button`] 的 `on_click` 对齐，可用于 `label` / `embed` 等任意 View。
    pub fn on_click<T, F>(mut self, state: &crate::ui::reactive::state::State<T>, mut f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut(&crate::ui::reactive::state::State<T>) + 'static,
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

    /// 无 State 的点击闭包；每次 reconcile **保守重绑**（公开用法见仓库 `docs/使用/事件.md`）。
    ///
    /// 命名保留 `_fn`：Rust 无法与 [`Self::on_click`] 重载；有 State 时优先 `on_click(&state, …)`（公开用法见仓库 `docs/使用/事件.md`）。
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

    /// 注册文本值变化处理器，并只向调用方暴露已验证的文本载荷。
    pub fn on_change_fn<F: FnMut(&str) + 'static>(mut self, mut f: F) -> Self {
        // 把文本载荷筛选集中在公开 View 契约，避免宏依赖内部处理器结构。
        let handler = Box::new(move |event: &mut SemanticEvent| {
            // 非文本 Change 事件不触发文本输入回调。
            if let Some(value) = event.text_payload() {
                // 把当前文本借用交给调用方处理。
                f(value);
            }
        });
        // 登记统一的 Change 语义类型。
        let registration = HandlerRegistration::new(SemanticKind::Change, handler);
        // 保留与其他 View 事件相同的注册顺序。
        self.handlers.push(registration);
        // 返回可继续应用样式与自动化属性的节点。
        self
    }

    /// 兼容别名：指纹同 [`Self::on_click`]，闭包不接收 `&State`。
    ///
    /// 新代码优先 `on_click(&state, |s| …)`；无 State 用 [`Self::on_click_fn`]。
    #[doc(alias = "on_click")]
    pub fn on_click_capture<T, F>(
        self,
        state: &crate::ui::reactive::state::State<T>,
        mut f: F,
    ) -> Self
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

    pub fn on_click_event_capture<T, F>(
        mut self,
        state: &crate::ui::reactive::state::State<T>,
        f: F,
    ) -> Self
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
    fn into_node(self) -> crate::ui::component::widget::WidgetNode {
        crate::ui::adapter::ViewAdapter::expand(self)
    }
}

/// 为所有可转换为 [`ViewNode`] 的 builder 提供原始事件声明。
mod ext;

pub use self::ext::*;

// 验证公开 View 事件便利入口的载荷筛选契约。
#[cfg(test)]
mod tests {
    // 引入当前模块公开与内部测试边界。
    use super::*;
    // 引入可共享修改的测试观察值。
    use std::cell::RefCell;
    // 引入单线程共享所有权。
    use std::rc::Rc;

    // 验证 Change 事件只转发文本载荷。
    #[test]
    fn on_change_fn_forwards_text_payload() {
        // 保存处理器观察到的最新文本。
        let observed = Rc::new(RefCell::new(String::new()));
        // 克隆所有权给静态事件闭包。
        let callback_observed = Rc::clone(&observed);
        // 创建最小输入节点并登记公开 Change 处理器。
        let mut node = ViewNode::leaf(crate::ui::widgets::Input::new("")).on_change_fn(
            // 把回调文本复制到测试观察值。
            move |value| *callback_observed.borrow_mut() = value.to_string(),
        );
        // 构造带文本载荷的语义变更事件。
        let mut event = SemanticEvent::change(crate::core::ComponentId::new(1), "Belldandy");
        // 调用节点登记的唯一处理器。
        (node.handlers[0].handler)(&mut event);
        // 回调必须收到完整当前文本。
        assert_eq!(&*observed.borrow(), "Belldandy");
    }
}
