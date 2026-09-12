//! # Widget 组件契约
//!
//! ## 组件模型
//!
//! 1. **数据 struct** — 组件首先是承载字段的结构体。
//! 2. **可选能力 trait** — 按需实现 `WidgetLayout` / `WidgetRender` /
//!    `EventHandler` / `WidgetLifecycle`；未实现的方法走 trait 默认行为。
//! 3. **`Widget` 胶水** — 用 `impl_widget!` 声明实现了哪些能力。
//!

use crate::core::{Constraints, EdgeInsets, Rect, Size, WidgetId};
use crate::draw::geometry::spatial::{Ray3D, SpatialContext};
use crate::draw::scene::PicturePolicy;
use crate::ui::event::{SemanticEvent, WindowAction};
use crate::ui::layout::{AlignItems, LayoutChild};
use crate::ui::overlay::OverlayEntry;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::{EventResult, SystemEvent, WidgetNode, WidgetTree};
use std::any::Any;
use std::time::Duration;

/// 能力位标记。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetCapabilities(u16);

impl WidgetCapabilities {
    /// 布局能力位。
    pub const LAYOUT: u16 = 0b0_0001;
    /// 渲染能力位。
    pub const RENDER: u16 = 0b0_0010;
    /// 事件处理能力位。
    pub const EVENT: u16 = 0b0_0100;
    /// 生命周期能力位。
    pub const LIFECYCLE: u16 = 0b0_1000;
    /// 动画能力位。
    pub const ANIMATION: u16 = 0b1_0000;
    /// 平台文本输入能力位。
    pub const TEXT_INPUT: u16 = 0b10_0000;
    /// 组件类型可能产生浮层的内部能力位。
    pub(crate) const MAY_PRODUCE_OVERLAY: u16 = 0b100_0000;
    /// 组件类型可能在事件或语义处理后请求布局的内部能力位。
    pub(crate) const MAY_REQUEST_EVENT_LAYOUT: u16 = 0b1000_0000;
    /// 组件类型需要窗口动作、动态协调、滚动或语义转换收尾的内部能力位。
    pub(crate) const REQUIRES_EXTENDED_EVENT_FINISH: u16 = 0b1_0000_0000;

    /// 创建不包含任何可选能力的标记。
    pub const fn new() -> Self {
        Self(0)
    }
    /// 从原始能力位创建标记。
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
    /// 插入一个或多个能力位。
    pub fn insert(&mut self, cap: u16) {
        self.0 |= cap;
    }
    /// 判断是否包含指定能力位中的任意一位。
    pub fn contains(&self, cap: u16) -> bool {
        self.0 & cap != 0
    }
    /// 返回原始能力位。
    pub fn bits(&self) -> u16 {
        self.0
    }
}

/// 组件核心标识 — 所有 widget 必须实现。
pub trait Widget: 'static {
    /// Opt into declaration comparison without allocating an owned snapshot.
    /// Implement declaration_config_changed and interaction_disabled when enabled.
    fn reconciles_without_snapshot(&self) -> bool { false }
    /// Cheap effective interaction gate, when available without capturing a snapshot.
    fn interaction_disabled(&self) -> Option<bool> { None }

    /// Optional tree-owned dynamic child coordination.
    fn dynamic_children_coordinator(&self) -> Option<&'static dyn crate::ui::DynamicChildrenCoordinator> { None }

    /// Adopt a new declaration while preserving this instance's runtime state.
    /// Return the declaration unchanged when this widget does not support patching.
    fn reconcile_from(&mut self, next: Box<dyn Widget>) -> Result<bool, Box<dyn Widget>> {
        Err(next)
    }
    /// Compare authored configuration omitted from or mixed with runtime snapshot fields.
    fn declaration_config_changed(&self, _next: &dyn Widget) -> Option<bool> {
        None
    }
    /// Classify geometry changes that cannot be inferred from a public snapshot.
    fn declaration_layout_changed(&self, _next: &dyn Widget) -> Option<bool> {
        None
    }
    /// Compare controlled state and private layout inputs across declarations.
    fn declaration_runtime_changed(&self, _next: &dyn Widget) -> bool {
        false
    }
    /// Apply authored style to the component's own layout and visual configuration.
    fn apply_declaration_style(
        &mut self,
        _style: &crate::ui::Style,
        _declared: &crate::ui::StyleDiff,
        _flex_grow: Option<f32>,
        _flex_shrink: Option<f32>,
    ) {}
    /// Axes in which a clipping viewport allows content overflow.
    fn viewport_overflow_axes(&self) -> (bool, bool) { (false, false) }
    /// Explicit width and height constraints owned by this component.
    fn layout_size_locks(&self) -> (bool, bool) { (false, false) }
    /// Whether this component currently owns a present overlay.
    fn overlay_is_present(&self) -> bool { false }
    /// Whether absent overlay content must be destroyed.
    fn overlay_destroy_on_close(&self) -> bool { false }
    /// Consume and apply a component-context close request.
    fn consume_context_close(&mut self) -> bool { false }
    /// Handle an outside-click dismissal for this overlay owner.
    fn dismiss_overlay(&mut self) {  }
    /// Restore pointer focus to a focusable descendant.
    fn pointer_focus_descendant(&self) -> bool { false }
    /// Expose the current editable value to semantic consumers.
    fn semantic_text_value(&self) -> Option<String> { None }
    /// Declare a native window drag region.
    fn window_drag_region(&self) -> bool { false }
    /// Invalidate the parent subtree after an action changes shared sibling state.
    fn invalidate_action_siblings(&self) -> bool { false }
    /// Select a child subtree for focus after child visibility changes.
    fn preferred_focus_child(&self) -> Option<usize> { None }
    /// Optional participation in cross-node text selection.
    fn as_text_selection(&self) -> Option<&dyn WidgetTextSelection> { None }
    /// Optional mutable text-selection policy port.
    fn as_text_selection_mut(&mut self) -> Option<&mut dyn WidgetTextSelection> { None }
    /// Mutable platform text editor port.
    fn as_text_input_mut(&mut self) -> Option<&mut dyn WidgetTextInput> { None }
    /// 以动态类型借用组件。
    fn as_any(&self) -> &dyn Any;
    /// 以动态类型可变借用组件。
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// 将组件所有权转换为动态类型。
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    /// 返回调试检查器使用的真实 Rust 动态类型名。
    fn debug_type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    /// 返回自动化和语义快照使用的稳定字段。
    fn snapshot_fields(&self) -> crate::ui::widget_snapshot::WidgetSnapshotFields {
        // 未登记的自定义组件没有类型化快照，直接返回未知，避免扫描全部内置类型。
        crate::ui::widget_snapshot::WidgetSnapshotFields::UNKNOWN
    }
    /// 返回此 widget 实现了哪些能力。
    fn capabilities(&self) -> WidgetCapabilities;
    /// 返回当前 widget 的内部可见性。
    fn visible(&self) -> bool {
        true
    }
    /// 构建并交出组件直接拥有的子组件。
    fn build(&self) -> Vec<Box<dyn Widget>> {
        vec![]
    }
    /// 在直接子节点集合变化后同步组件拥有的派生运行态。
    fn on_children_changed(&mut self, child_count: usize) {
        // 默认组件不缓存子节点派生状态，因此只消费通知参数。
        let _ = child_count;
    }
    /// 直接或间接子树的可见成员变化后，清除依赖旧子树的布局派生状态。
    fn on_child_visibility_changed(&mut self) {}
    /// 声明期 View 子节点能力端口（SMC-04：由 System 私有边界
    /// `ViewChildrenProvider` 承载，避免 widget → view 依赖）。
    ///
    /// 返回 `Some` 表示该组件持有完整 ViewNode 子树（含 handlers 与样式）；
    /// 实现经 `widget!` 宏的 `build_view_children` 方法自动生成。
    fn as_view_children(&self) -> Option<&dyn crate::ui::adapter::ViewChildrenProvider> {
        None
    }
    /// 默认 Tab 键导航索引（> 0 表示组件默认可通过 Tab 聚焦）。
    /// 应用层可通过 `WidgetNode::tab_index()` 覆盖。
    fn tab_index(&self) -> i32 {
        0
    }
    /// 焦点位于本组件时是否把 Tab 作为原始输入消费而不是移动焦点。
    ///
    /// 默认 `false`（Tab 交给树层焦点导航）；真实终端等把 Tab 交给
    /// 子进程解释的组件返回 `true`。
    fn consumes_tab_key(&self) -> bool {
        false
    }
    /// 是否将子树作为独立节点暴露给语义快照。
    ///
    /// 复合交互组件可返回 `false`，让内部纯展示 View 不形成嵌套控件语义。
    fn exposes_semantic_children(&self) -> bool {
        true
    }

    /// 组件声明的无障碍动作（E-05）：`widget!` 的 `semantic_actions` 槽位
    /// 生成此方法；在 role 推断之外声明自定义能力（如连续值 `Adjust`）。
    /// 声明进入语义快照，并经窗口 owner thread 的自动化执行器路由。
    fn declared_semantic_actions(&self) -> &'static [crate::ui::SemanticAction] {
        &[]
    }
    /// 返回组件的离屏 Picture 缓存策略。
    fn picture_policy(&self) -> PicturePolicy {
        PicturePolicy::Never
    }
    /// 判断组件内容是否会在没有结构变化时动态更新。
    fn has_dynamic_content(&self) -> bool {
        false
    }
    /// 声明组件类型是否可能产生浮层；手写组件默认保守参与重建。
    #[doc(hidden)]
    fn may_produce_overlay(&self) -> bool {
        true
    }
    /// 声明组件是否可能通过 `take_layout_request` 请求布局。
    #[doc(hidden)]
    fn may_request_event_layout(&self) -> bool {
        // 手写组件默认保守参与检查，避免新增能力声明改变既有语义。
        true
    }
    /// 声明组件处理事件后是否需要布局请求以外的扩展收尾。
    #[doc(hidden)]
    fn requires_extended_event_finish(&self) -> bool {
        // 手写组件默认保守执行完整收尾，保持既有扩展能力兼容。
        true
    }

    // ── 可选能力上转型（宏自动生成） ──
    /// 按能力标记上转型为布局契约。
    fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        None
    }
    /// 按能力标记上转型为只读渲染契约。
    fn as_render(&self) -> Option<&dyn WidgetRender> {
        None
    }
    /// 按能力标记上转型为可变渲染契约。
    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        None
    }
    /// 按能力标记上转型为只读事件契约。
    fn as_event(&self) -> Option<&dyn EventHandler> {
        None
    }
    /// 按能力标记上转型为可变事件契约。
    fn as_event_mut(&mut self) -> Option<&mut dyn EventHandler> {
        None
    }
    /// 按能力标记上转型为只读生命周期契约。
    fn as_lifecycle(&self) -> Option<&dyn WidgetLifecycle> {
        None
    }
    /// 按能力标记上转型为可变生命周期契约。
    fn as_lifecycle_mut(&mut self) -> Option<&mut dyn WidgetLifecycle> {
        None
    }
    /// 按能力标记上转型为只读动画契约。
    fn as_animation(&self) -> Option<&dyn WidgetAnimation> {
        None
    }
    /// 按能力标记上转型为可变动画契约。
    fn as_animation_mut(&mut self) -> Option<&mut dyn WidgetAnimation> {
        None
    }
    /// 按能力标记上转型为文本输入契约。
    fn as_text_input(&self) -> Option<&dyn WidgetTextInput> {
        None
    }
}

/// Optional platform-neutral text-input capability.
///
/// `app` uses this trait to activate the platform IME only for the focused
/// text editor and to position the native composition/candidate UI without
/// depending on a concrete widget type.
pub trait WidgetTextInput: Widget {
    /// Capture text and character-indexed selection without exposing editor storage.
    fn text_edit_snapshot(&self) -> Option<TextEditSnapshot> { None }
    /// Restore a character-indexed selection after a synthetic editing operation.
    fn restore_text_edit_selection(&mut self, _start: usize, _end: usize) {}

    /// 判断组件当前是否接受平台文本输入。
    fn accepts_text_input(&self) -> bool {
        true
    }

    /// 返回布局绘制坐标中的光标矩形；窗口层统一映射为输入法表面坐标。
    fn text_input_cursor_rect(&self) -> Rect {
        Rect::zero()
    }
}

/// 布局行为：尺寸、弹性、子节点排列。
pub trait WidgetLayout: Widget {
    /// 声明直接子项的 Flex 方向与默认交叉轴对齐；非 Flex 布局返回 None。
    fn flex_layout_axes(&self) -> Option<(crate::ui::layout::FlexDirection, AlignItems)> {
        None
    }
    /// 在给定约束下测量组件固有尺寸。
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::zero())
    }
    /// 在不应用父级 Flex basis 策略时测量组件的自然内容尺寸。
    fn measure_natural(&self, constraints: Constraints) -> Size {
        // 默认组件没有独立的 Flex basis 策略，直接复用普通测量。
        self.measure(constraints)
    }
    /// 允许透明包装组件在同一测量轮次内由直接子节点决定自身尺寸。
    fn measure_from_children(
        &self,
        _constraints: Constraints,
        _children: &[WidgetId],
        _tree: &WidgetTree,
    ) -> Option<Size> {
        None
    }
    /// 在指定父主轴上覆盖弹性基值；None 使用测得的 border-box 主尺寸。
    fn flex_basis(&self, _parent_direction: crate::ui::layout::FlexDirection) -> Option<f32> {
        None
    }
    /// 不可被 Flex 收缩或 Stretch 压破的 border-box 最小尺寸。
    fn minimum_size(&self) -> Size {
        Size::zero()
    }
    /// 组件内核自有样式声明的 min/max 尺寸约束；未声明项由声明节点元数据补足。
    ///
    /// 只有持有完整 [`crate::ui::Style`] 的容器需要实现；其余组件的约束经
    /// 声明节点统一交付到布局子项，无需逐个复制。
    fn size_constraints(&self) -> crate::ui::theme::style::SizeConstraints {
        crate::ui::theme::style::SizeConstraints::NONE
    }
    /// 返回 Flex 扩展系数。
    fn flex_grow(&self) -> f32 {
        0.0
    }
    /// 返回 Flex 收缩系数。
    fn flex_shrink(&self) -> f32 {
        1.0
    }
    /// 返回当前子项的交叉轴覆盖对齐方式。
    fn align_self(&self) -> Option<AlignItems> {
        None
    }
    /// 返回兼容的一维 Grid 单元索引。
    fn grid_cell(&self) -> Option<usize> {
        None
    }
    /// 返回 Grid 跨列数。
    fn grid_column_span(&self) -> u32 {
        1
    }
    /// 返回 Grid 跨行数。
    fn grid_row_span(&self) -> u32 {
        1
    }
    /// 返回参与父布局计算的外边距。
    fn layout_margin(&self) -> EdgeInsets {
        EdgeInsets::zero()
    }
    /// Whether positioned children may expand this node during layout convergence.
    fn child_overflow_expands_parent(&self) -> bool {
        true
    }
    /// Whether the direct child at `index` participates in this layout
    /// container's visible subtree. This does not overwrite the child's
    /// authored visibility.
    fn child_visible(&self, _index: usize) -> bool {
        true
    }
    /// Prepare a pass-local measurement snapshot for the next child arrange.
    ///
    /// Exact-fill containers keep the default zero-sized descriptors so they do
    /// not measure children they will stretch unconditionally.
    fn measure_children(
        &self,
        _frame: Rect,
        children: &[WidgetId],
        _tree: &WidgetTree,
    ) -> Vec<LayoutChild> {
        children
            .iter()
            .copied()
            .map(|id| LayoutChild::new(id, Size::zero()))
            .collect()
    }
    /// 把子节点测量结果写入调用方工作区；默认兼容既有拥有型实现。
    #[doc(hidden)]
    fn measure_children_into(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>,
    ) {
        output.clear();
        output.extend(self.measure_children(frame, children, tree));
    }
    /// Arrange children from the snapshot produced by [`Self::measure_children`].
    /// Implementations must not call child `measure` recursively here.
    fn layout_children(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        let _ = (frame, children, tree);
        Vec::new()
    }
    /// 把子节点位置写入调用方工作区；默认兼容既有拥有型实现。
    #[doc(hidden)]
    fn layout_children_into(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
        _scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        output.clear();
        output.extend(self.layout_children(frame, children, tree));
    }
}

/// 渲染行为：绘制、覆盖层、脏区域。
pub trait WidgetRender: Widget {
    /// 在已排列矩形内绘制组件内容。
    fn render(&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree);
    /// 判断组件绘制是否依赖当前主题色板。
    fn uses_palette(&self) -> bool {
        true
    }
    /// 返回布局边界之外额外受绘制影响的距离。
    fn draw_margin(&self) -> f32 {
        0.0
    }
    /// 返回包含组件全部绘制像素的脏矩形。
    fn dirty_rect(&self, frame: Rect) -> Rect {
        let m = self.draw_margin();
        if m > 0.0 {
            Rect::new(
                frame.x - m,
                frame.y - m,
                frame.w + m * 2.0,
                frame.h + m * 2.0,
            )
        } else {
            frame
        }
    }
    /// 返回应用到直接子树的可选裁剪矩形。
    fn children_clip(&self, _frame: Rect) -> Option<Rect> {
        None
    }
    /// 在父坐标系中作用于直接子树的绘制和命中变换；不改变终态布局。
    /// `Some(identity)` 仍保留动态合成边界，避免静止帧被固化为 Picture。
    fn children_transform(&self, _frame: Rect) -> Option<crate::draw::Transform> {
        None
    }
    /// 乘到直接子树的合成透明度；由子树继承，不逐后代重复相乘。
    fn children_opacity(&self) -> f32 {
        1.0
    }
    /// 声明该组件是否需要在真实子树完成后获得覆盖绘制阶段。
    fn paint_after_children(&self) -> bool {
        // 普通组件只绘制 Content，避免父背景在子树之后重复覆盖。
        false
    }
    /// 创建可选的浮层登记。
    fn overlay_entry(&self, _id: WidgetId, _frame: Rect) -> Option<OverlayEntry> {
        None
    }
    /// 使用当前逻辑表面创建浮层登记，供依赖窗口边界的定位组件覆盖。
    fn overlay_entry_for_surface(
        // 借用当前渲染组件。
        &self,
        // 接收浮层所属组件标识。
        id: WidgetId,
        // 接收组件布局矩形。
        frame: Rect,
        // 接收当前逻辑表面矩形。
        _surface: Rect,
        // 返回与旧接口相同的可选浮层登记。
    ) -> Option<OverlayEntry> {
        // 默认委托旧接口，保持现有组件行为不变。
        self.overlay_entry(id, frame)
    }
}

/// 事件行为：输入事件处理、滚动偏移、命中测试。
pub trait EventHandler: Widget {
    /// 可选地直接返回当前是否允许交互；`None` 保持从完整无障碍快照派生的兼容路径。
    ///
    /// 内建高频控件可实现该窄查询，避免事件门控为读取单个布尔值复制完整快照。
    #[doc(hidden)]
    fn interaction_enabled(&self) -> Option<bool> {
        None
    }
    /// 处理一个已路由到组件的系统事件。
    fn on_event(&mut self, _event: &SystemEvent) -> EventResult {
        EventResult::NotHandled
    }
    /// 组件自身或后代进入/离开焦点范围时通知一次。
    ///
    /// 同一组件子树内的焦点切换不会重复触发；需要观察子控件焦点的交互包装器
    /// 应实现本钩子，而不是改变普通 `FocusIn` / `FocusOut` 的目标分发语义。
    fn on_focus_within(&mut self, _focused: bool) -> EventResult {
        EventResult::NotHandled
    }
    /// 取走本次已处理事件产生的窗口动作。
    ///
    /// 动作由 `WidgetTree` 收集，并在事件所属窗口上同步执行；组件不得直接
    /// 持有平台窗口句柄。
    fn take_window_action(&mut self) -> Option<WindowAction> {
        None
    }
    /// 将系统事件转换为组件语义事件。
    fn semantic_event(&self, _id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        None
    }
    /// 取走本次已处理事件产生的布局请求。
    ///
    /// 仅当组件的内部运行态改变了子节点布局时返回 `true`；纯绘制状态变化
    /// 保持 `false`，避免把普通交互扩大为布局遍历。
    fn take_layout_request(&mut self) -> bool {
        false
    }
    /// 返回本次事件产生的滚动位移。
    fn scroll_delta(&self, _frame: Rect) -> Option<(f32, f32)> {
        None
    }
    /// 帧间滚动偏移（用于 scroll_region 像素移动优化）。
    fn scroll_delta_for_dirty(&self) -> Option<(f32, f32)> {
        None
    }
    /// 限定 scroll_region 像素移动的 viewport；`None` 表示使用组件 frame。
    fn scroll_composite_viewport(&self, _frame: Rect) -> Option<Rect> {
        None
    }
    /// viewport 容器当前 scroll 偏移（content 坐标系）；非 viewport 返回 `None`。
    fn viewport_scroll_offset(&self) -> Option<(f32, f32)> {
        None
    }
    /// 为显露后代节点而滚动指定距离；非 viewport 返回 `false`。
    fn scroll_descendant_by(&mut self, _dx: f32, _dy: f32) -> bool {
        false
    }
    /// 返回组件当前请求的计时器标识与周期。
    fn active_timer(&self) -> Option<(u64, Duration)> {
        None
    }
    /// 判断组件是否需要在捕获阶段接收事件。
    fn wants_capture_phase(&self) -> bool {
        false
    }
    /// Opt in to receiving every PointerMove while the pointer remains inside
    /// the widget's hit-test frame. Default false enables the boundary-aware
    /// PointerMove fast path.
    fn wants_continuous_pointer_move(&self) -> bool {
        false
    }
    /// 返回组件参与二维命中测试的矩形。
    fn hit_test_frame(&self, actual_frame: Rect) -> Rect {
        actual_frame
    }
    /// 是否继续命中测试子节点。交互包装器可关闭它，让任意展示内容都由包装器接管。
    fn hit_test_children(&self) -> bool {
        true
    }
    /// 判断三维射线是否命中组件所在的二维平面区域。
    fn hit_test_3d(&self, ray: &Ray3D, _spatial: &SpatialContext, frame: Rect) -> bool {
        if let Some(hit_point) = ray.intersect_z0() {
            hit_point.x >= frame.x
                && hit_point.x <= frame.x + frame.w
                && hit_point.y >= frame.y
                && hit_point.y <= frame.y + frame.h
        } else {
            false
        }
    }
}

/// 生命周期行为。
pub trait WidgetLifecycle: Widget {
    /// 组件实例完成初始化时调用。
    fn on_init(&mut self) {}
    /// 组件附加到树时调用。
    fn on_attach(&mut self) {}
    /// 组件挂载到活动窗口时调用。
    fn on_mount(&mut self) {}
    /// 组件进入活动状态时调用。
    fn on_active(&mut self) {}
    /// 组件离开活动状态时调用。
    fn on_inactive(&mut self) {}
    /// 活动主题发生变化时调用。
    fn on_theme_changed(&mut self) {}
    /// 组件从活动窗口卸载时调用。
    fn on_unmount(&mut self) {}
    /// 组件从树分离时调用。
    fn on_detach(&mut self) {}
    /// 组件即将销毁时调用。
    fn on_destroy(&mut self) {}
}

/// Animation behavior advanced by the centralized App loop.
pub trait WidgetAnimation: Widget {
    /// Advances animation state by `dt` seconds.
    ///
    /// Returns `true` while another frame is required. Implementations should
    /// return `false` once the animated state has reached rest.
    fn update_animation(&mut self, _dt: f64) -> bool {
        false
    }

    /// Paint bounds dirtied by the animation state change.
    fn dirty_bounds(&self, frame: Rect) -> Rect {
        let _ = frame;
        Rect::zero()
    }
}

/// 转换为 WidgetNode 的 trait。
pub trait IntoWidgetNode {
    /// 将值转换为拥有完整生命周期的组件树节点。
    fn into_node(self) -> WidgetNode;
}

impl<T: Widget + 'static> IntoWidgetNode for T {
    fn into_node(self) -> WidgetNode {
        WidgetNode::leaf(Box::new(self))
    }
}

impl IntoWidgetNode for WidgetNode {
    fn into_node(self) -> WidgetNode {
        self
    }
}

/// Character-indexed editing state shared with semantic editing commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEditSnapshot {
    pub value: String,
    pub selection: Option<(usize, usize)>,
    pub caret: usize,
}

/// A component's text-selection capability, independent of its concrete type.
pub trait WidgetTextSelection {
    fn selection_enabled(&self) -> bool;
    fn selection_dragging(&self) -> bool;
    fn selection_len(&self) -> usize;
    fn selection_anchor(&self) -> usize;
    fn set_selection_range(&self, range: Option<(usize, usize)>);
    fn selection_char_at(&self, point: crate::core::Point) -> usize;
    fn selection_text(&self) -> Option<String>;
    fn set_selection_policy(&mut self, policy: crate::ui::UserSelect);
}
