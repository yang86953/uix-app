//! # 用户层简化 API — View 体系
//!
//! 本模块提供函数式组合子 API，让用户通过 `View` trait 和 `ViewNode` 声明 UI，
//! 完全不需要了解 `WidgetTree`、`WidgetNode`、`BoxedWidget` 等内部概念。

use crate::core::{EdgeInsets, Point};
use crate::draw::Color;
use crate::ui::accessibility::accessibility_override::AccessibilityOverride;
use crate::ui::widget_runtime::provider_context::{ProviderContext, current_provider_context};
// 让声明节点携带内联组件的非视觉状态作用域标记。
use crate::ui::event::system_event_handler::{SystemEventFilter, SystemEventHandlerRegistration};
use crate::ui::event::{HandlerRegistration, SemanticEvent, SemanticKind};
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::widget_runtime::traits::Widget;
use crate::ui::widget_runtime::view_transform::ViewTransform;
use crate::ui::widget_state::{
    UixWidgetScope,
    UixWidgetScopeMarker,
    // 引入随声明根延迟提交的私有状态写入回执。
    WidgetStateCaptureReceipt,
    // 引入窗口私有状态存储和组件作用域标记。
    WidgetStateStore,
};
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
// 拆分声明式动画绑定，保持 ViewNode 核心低于规模上限。
mod animation_bindings;
// 拆分 UIX 状态过渡的目标比较与原位重定向组件。
#[doc(hidden)]
pub mod declarative_transition;
// 拆分定位链式入口，保持声明节点主体低于规模上限。
mod position;

/// 用户层 UI 声明 trait。
pub trait View: 'static {
    /// 构建并交出声明式视图根节点。
    fn build(self) -> ViewNode;
}

/// 中间节点表示。
pub struct ViewNode {
    pub(crate) widget: Box<dyn Widget>,
    pub(crate) children: Vec<ViewNode>,
    // 保存当前捕获根读取的结构性 State 绑定，建树时由所属 WidgetTree 提交。
    pub(crate) captured_state_binds: Vec<Arc<dyn StatePaintBind>>,
    // 子树作用域的重建工厂：由 `scoped()` 声明；presence 同时表示本节点的
    // 结构性 State 绑定须安装为节点作用域失效（而非整树 reconcile 请求）。
    pub(crate) scoped_rebuild: Option<Arc<dyn Fn() -> ViewNode>>,
    // 保存当前捕获根创建的 Effect，协调时由所属 WidgetTree 整体替换。
    pub(crate) captured_effects: Vec<Effect>,
    pub(crate) animated_sources: Vec<std::sync::Arc<dyn crate::ui::animation::AnimatedSource>>,
    pub(crate) provider_context: ProviderContext,
    pub(crate) style: Style,
    pub(crate) visual_transform: ViewTransform,
    // 保存由组件树布局 Module 求解的节点定位元数据。
    pub(crate) position: crate::ui::position::PositionedLayout,
    // 保存当前声明节点的文字选择策略；Auto 由运行时结合祖先解析。
    pub(crate) user_select: crate::ui::UserSelect,
    // 保存当前声明节点显式覆盖的指针光标；None 表示继承父节点。
    pub(crate) cursor: Option<crate::platform::windowing::CursorType>,
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
    // 绝大多数节点没有语义覆盖，按需分配避免在每个声明节点内联大对象。
    pub(crate) accessibility_override: Option<Box<AccessibilityOverride>>,
    pub(crate) handlers: Vec<HandlerRegistration>,
    pub(crate) system_event_handlers: Vec<SystemEventHandlerRegistration>,
    pub(crate) render_handlers: Vec<RenderHandlerRegistration>,
    // 保存承载此根的全部内联组件作用域，不参与视觉或语义快照。
    pub(crate) uix_widget_scopes: Vec<UixWidgetScopeMarker>,
    // 仅由捕获根携带，用于把首次构建绑定到同一窗口状态存储。
    pub(crate) widget_state_store: Option<WidgetStateStore>,
    // 保存尚未由成功树事务接纳的组件状态写入回执。
    pub(crate) widget_state_receipts: Vec<WidgetStateCaptureReceipt>,
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

    /// 从不带声明子节点的组件创建叶节点。
    pub fn leaf(widget: impl Widget + 'static) -> Self {
        Self {
            widget: Box::new(widget),
            children: vec![],
            // 非捕获构造路径不携带结构性 State 绑定。
            captured_state_binds: Vec::new(),
            // 非捕获构造路径不携带子树作用域重建工厂。
            scoped_rebuild: None,
            // 非捕获构造路径不携带根 Effect。
            captured_effects: Vec::new(),
            animated_sources: Vec::new(),
            provider_context: current_provider_context(),
            style: Style::default(),
            visual_transform: ViewTransform::default(),
            // 新声明节点默认参与正常布局流且四边均为 auto。
            position: crate::ui::position::PositionedLayout::default(),
            // 新声明节点默认保留组件自身选择能力。
            user_select: crate::ui::UserSelect::Auto,
            // 未声明 cursor 时交给运行时沿父链继承。
            cursor: None,
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
            uix_widget_scopes: Vec::new(),
            widget_state_store: None,
            // 非捕获构造路径没有需要延迟提交的私有状态写入。
            widget_state_receipts: Vec::new(),
        }
    }

    /// 从组件及其声明式直接子节点创建节点。
    pub fn new(widget: impl Widget + 'static, children: Vec<ViewNode>) -> Self {
        Self {
            widget: Box::new(widget),
            children,
            // 非捕获构造路径不携带结构性 State 绑定。
            captured_state_binds: Vec::new(),
            // 非捕获构造路径不携带子树作用域重建工厂。
            scoped_rebuild: None,
            // 非捕获构造路径不携带根 Effect。
            captured_effects: Vec::new(),
            animated_sources: Vec::new(),
            provider_context: current_provider_context(),
            style: Style::default(),
            visual_transform: ViewTransform::default(),
            // 新声明子树默认参与正常布局流且四边均为 auto。
            position: crate::ui::position::PositionedLayout::default(),
            // 新声明子树默认保留组件自身选择能力。
            user_select: crate::ui::UserSelect::Auto,
            // 未声明 cursor 时交给运行时沿父链继承。
            cursor: None,
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
            uix_widget_scopes: Vec::new(),
            widget_state_store: None,
            // 非捕获构造路径没有需要延迟提交的私有状态写入。
            widget_state_receipts: Vec::new(),
        }
    }

    /// 为内联组件展开根追加非视觉的私有状态作用域标记。
    #[doc(hidden)]
    pub fn uix_widget_scope(mut self, scope: UixWidgetScope, root_ordinal: u64) -> Self {
        // 追加而非覆盖，以保留多个内联组件共享同一实际根的身份。
        self.uix_widget_scopes
            .push(UixWidgetScopeMarker::new(scope, root_ordinal));
        // 返回携带完整作用域栈的原节点。
        self
    }

    // 由捕获适配器绑定首次构建的窗口私有状态存储。
    pub(crate) fn set_widget_state_store(&mut self, store: WidgetStateStore) {
        // 仅根节点需要保存存储所有权线索。
        self.widget_state_store = Some(store);
    }

    // 让捕获适配器把一次组件状态 journal 的所有权附到声明根。
    pub(crate) fn push_widget_state_receipt(&mut self, receipt: WidgetStateCaptureReceipt) {
        // 保持捕获顺序，以便统一在树事务成功后接纳全部写入。
        self.widget_state_receipts.push(receipt);
    }

    /// 设置文本颜色。
    pub fn color(mut self, color: impl Into<ColorValue>) -> Self {
        self.style.color = color.into();
        self
    }

    /// 设置背景颜色。
    pub fn background_color(mut self, color: impl Into<ColorValue>) -> Self {
        // 保存可在主题解析阶段统一求值的背景色。
        self.style.background = Some(color.into());
        // 返回完成样式更新的节点。
        self
    }

    /// 设置字体大小令牌。
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

    // 声明当前节点及其文本子树的文字选择策略。
    pub fn user_select(mut self, value: crate::ui::UserSelect) -> Self {
        // 保留显式 Auto，以便运行时按同一父子规则重新协调。
        self.user_select = value;
        // 返回携带结构性交互元数据的原节点。
        self
    }

    /// 设置常态背景色。
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

    /// 设置四边内边距。
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

    /// 设置四边外边距。
    pub fn margin(mut self, m: impl Into<EdgeInsets>) -> Self {
        self.style.margin = m.into();
        self
    }

    /// 设置显式宽度。
    pub fn width(mut self, w: f32) -> Self {
        self.style.width = Some(w);
        self
    }

    /// 设置显式高度。
    pub fn height(mut self, h: f32) -> Self {
        self.style.height = Some(h);
        self
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

    /// 设置围绕布局帧中心应用的二维仿射视觉变换。
    pub fn affine_transform(mut self, transform: crate::draw::Transform) -> Self {
        // 只更新声明矩阵，保留既有 offset、scale 与动画绑定。
        self.visual_transform.affine = transform;
        // 返回可继续链式声明的节点。
        self
    }

    /// 设置布局帧确定后解析的二维视觉变换原点。
    pub fn transform_origin(mut self, origin: crate::ui::TransformOrigin) -> Self {
        // 只更新原点值，保留既有矩阵、offset、scale 与动画绑定。
        self.visual_transform.origin = origin;
        // 返回可继续链式声明的节点。
        self
    }

    /// 设置指针命中当前节点时请求的平台光标。
    pub fn cursor(mut self, cursor: crate::platform::windowing::CursorType) -> Self {
        // 显式保存 Arrow 以允许子节点覆盖父节点的继承光标。
        self.cursor = Some(cursor);
        // 返回可继续链式声明的节点。
        self
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

    /// 设置 Flex 扩展系数并保留显式覆盖语义。
    pub fn flex_grow(mut self, g: f32) -> Self {
        self.style.flex_grow = g;
        self.flex_grow_override = Some(g);
        self
    }

    /// 设置 Flex 收缩系数并保留显式覆盖语义。
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

    /// 设置当前子项的交叉轴覆盖对齐方式。
    pub fn align_self(mut self, a: crate::ui::layout::AlignItems) -> Self {
        self.style.align_self = Some(a);
        self
    }

    /// 设置兼容的一维 Grid 单元索引。
    pub fn grid_cell(mut self, cell: usize) -> Self {
        self.style.grid_cell = Some(cell);
        self
    }

    /// 设置至少为一的 Grid 跨列数和跨行数。
    pub fn grid_span(mut self, columns: u32, rows: u32) -> Self {
        self.style.grid_column_span = columns.max(1);
        self.style.grid_row_span = rows.max(1);
        self
    }

    /// 设置直接子项的统一间距。
    pub fn gap(mut self, g: f32) -> Self {
        self.style.gap = g;
        self
    }

    /// 保留子项的自然主轴尺寸，用于 ScrollView 的内容容器。
    pub fn overflow_content(mut self) -> Self {
        self.style.overflow_content = true;
        self
    }

    /// 显式设置直接子树是否裁剪到当前节点边界。
    pub fn clip_content(mut self, clip: bool) -> Self {
        // 保存显式真假值以支持状态样式清除旧裁剪。
        self.style.clip_content = Some(clip);
        // 返回节点以继续声明式链式配置。
        self
    }

    /// 设置统一宽度和颜色的边框。
    pub fn border(mut self, width: f32, color: impl Into<ColorValue>) -> Self {
        self.style.border_width = EdgeInsets::uniform(width);
        self.style.border_color = Some(color.into());
        self
    }

    /// 设置圆角半径。
    pub fn radius(mut self, r: f32) -> Self {
        self.style.border_radius = r;
        self
    }

    /// 设置节点透明度。
    pub fn opacity(mut self, o: f32) -> Self {
        self.style.opacity = o;
        self
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

    /// 设置同层节点的绘制与命中顺序。
    pub fn z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }

    /// 设置声明式协调使用的稳定节点身份。
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
        self.accessibility_override = Some(Box::new(AccessibilityOverride::replace(accessibility)));
        self
    }

    // 原位复用已分配的覆盖对象，连续链式声明不重复申请堆内存。
    fn update_accessibility_override(
        &mut self,
        update: impl FnOnce(AccessibilityOverride) -> AccessibilityOverride,
    ) {
        // 仅在首次声明覆盖时分配，随后复用同一 Box。
        let accessibility_override = self
            .accessibility_override
            .get_or_insert_with(|| Box::new(AccessibilityOverride::default()));
        // 暂时取出值以复用现有消费式 builder，同时保留外层分配。
        **accessibility_override = update(std::mem::take(accessibility_override.as_mut()));
    }

    /// 覆写节点 role，同时保留组件实时派生的 name 与 state。
    pub fn role(mut self, role: AccessibilityRole) -> Self {
        self.update_accessibility_override(|current| current.with_role(role));
        self
    }

    /// 覆写节点可访问名称；空字符串会显式清除组件默认名称。
    pub fn accessible_name(mut self, name: impl Into<String>) -> Self {
        self.update_accessibility_override(|current| current.with_name(name));
        self
    }

    /// 覆写节点完整无障碍状态。
    pub fn accessibility_state(mut self, state: AccessibilityState) -> Self {
        self.update_accessibility_override(|current| current.with_state(state));
        self
    }

    /// 追加或按名称替换一个 ARIA 属性。
    pub fn aria(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.update_accessibility_override(|current| {
            current.with_attribute(AriaAttribute::new(name, value))
        });
        self
    }

    fn with_system_event_handler(mut self, registration: SystemEventHandlerRegistration) -> Self {
        self.system_event_handlers.push(registration);
        self
    }

    /// 注册指定类型的语义事件处理器。
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

    /// 注册携带 State 稳定捕获指纹的语义事件处理器。
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

    /// 注册携带 Computed 稳定捕获指纹的语义事件处理器。
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

    /// 注册携带窗口身份捕获指纹的语义事件处理器。
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

    /// 注册携带窗口身份捕获指纹的点击闭包。
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

    /// 注册可以读取和修改完整点击语义事件的处理器。
    pub fn on_click_event<F: FnMut(&mut SemanticEvent) + 'static>(mut self, f: F) -> Self {
        self.handlers
            .push(HandlerRegistration::new(SemanticKind::Click, Box::new(f)));
        self
    }

    /// 注册携带 State 稳定捕获指纹的完整点击事件处理器。
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

    /// 注册携带窗口身份捕获指纹的完整点击事件处理器。
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
    fn into_node(self) -> crate::ui::widget_runtime::widget::WidgetNode {
        crate::ui::adapter::ViewAdapter::expand(self)
    }
}

/// 为所有可转换为 [`ViewNode`] 的 builder 提供原始事件声明。
mod ext;

pub use self::ext::*;
