//! # Widget 组件契约
//!
//! ## 组件模型
//!
//! 1. **数据 struct** — 组件首先是承载字段的结构体。
//! 2. **可选能力 trait** — 按需实现 `WidgetLayout` / `WidgetRender` /
//!    `EventHandler` / `WidgetLifecycle`；未实现的方法走 trait 默认行为。
//! 3. **`WidgetComponent` 胶水** — 用 `impl_widget_component!` 声明实现了哪些能力。
//!
//! ```ignore
//! pub struct Button { text: String, ... }
//!
//! impl_widget_component!(Button; Layout, Render, Event, Lifecycle; tab_index => 1);
//!
//! impl WidgetLayout for Button {
//!     fn measure(&self, constraints: Constraints) -> Size { ... }
//! }
//! impl WidgetRender for Button {
//!     fn render(&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) { ... }
//! }
//! ```

use crate::core::{ComponentId, Constraints, EdgeInsets, Rect, Size};
use crate::draw::geometry::spatial::{Ray3D, SpatialContext};
use crate::draw::scene::PicturePolicy;
use crate::ui::core::paint_context::PaintContext;
use crate::ui::event::{SemanticEvent, WindowAction};
use crate::ui::layout::{AlignItems, LayoutChild};
use crate::ui::overlay::OverlayEntry;
use crate::ui::widget::{EventResult, SystemEvent, WidgetNode, WidgetTree};
use std::any::Any;
use std::time::Duration;

/// 能力位标记。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetCapabilities(u16);

impl WidgetCapabilities {
    pub const LAYOUT: u16 = 0b0_0001;
    pub const RENDER: u16 = 0b0_0010;
    pub const EVENT: u16 = 0b0_0100;
    pub const LIFECYCLE: u16 = 0b0_1000;
    pub const ANIMATION: u16 = 0b1_0000;
    pub const TEXT_INPUT: u16 = 0b10_0000;

    pub const fn new() -> Self {
        Self(0)
    }
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
    pub fn insert(&mut self, cap: u16) {
        self.0 |= cap;
    }
    pub fn contains(&self, cap: u16) -> bool {
        self.0 & cap != 0
    }
    pub fn bits(&self) -> u16 {
        self.0
    }
}

/// 组件核心标识 — 所有 widget 必须实现。
pub trait WidgetComponent: 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn snapshot_fields(&self) -> crate::ui::component_snapshot::SnapshotFields {
        crate::ui::component_snapshot::snapshot_fields_from_any(self.as_any())
    }
    /// 返回此 widget 实现了哪些能力。
    fn capabilities(&self) -> WidgetCapabilities;
    /// 返回当前 widget 的内部可见性。
    fn visible(&self) -> bool {
        true
    }
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
        vec![]
    }
    /// Declaration-time View children owned by a component.
    ///
    /// Unlike `build`, this hook preserves the complete ViewNode subtree,
    /// including handlers and styles. It is used by components whose public
    /// builder accepts a custom View while keeping that View outside the
    /// component's business state.
    fn build_view_children(&self) -> Vec<crate::ui::view::ViewNode> {
        vec![]
    }
    /// 默认 Tab 键导航索引（> 0 表示组件默认可通过 Tab 聚焦）。
    /// 应用层可通过 `WidgetNode::tab_index()` 覆盖。
    fn tab_index(&self) -> i32 {
        0
    }
    /// 是否将子树作为独立节点暴露给语义快照。
    ///
    /// 复合交互组件可返回 `false`，让内部纯展示 View 不形成嵌套控件语义。
    fn exposes_semantic_children(&self) -> bool {
        true
    }
    fn picture_policy(&self) -> PicturePolicy {
        PicturePolicy::Never
    }
    fn has_dynamic_content(&self) -> bool {
        false
    }

    // ── 可选能力上转型（宏自动生成） ──
    fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        None
    }
    fn as_render(&self) -> Option<&dyn WidgetRender> {
        None
    }
    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        None
    }
    fn as_event(&self) -> Option<&dyn EventHandler> {
        None
    }
    fn as_event_mut(&mut self) -> Option<&mut dyn EventHandler> {
        None
    }
    fn as_lifecycle(&self) -> Option<&dyn WidgetLifecycle> {
        None
    }
    fn as_lifecycle_mut(&mut self) -> Option<&mut dyn WidgetLifecycle> {
        None
    }
    fn as_animation(&self) -> Option<&dyn WidgetAnimation> {
        None
    }
    fn as_animation_mut(&mut self) -> Option<&mut dyn WidgetAnimation> {
        None
    }
    fn as_text_input(&self) -> Option<&dyn WidgetTextInput> {
        None
    }
}

/// Optional platform-neutral text-input capability.
///
/// `app` uses this trait to activate the platform IME only for the focused
/// text editor and to position the native composition/candidate UI without
/// depending on a concrete widget type.
pub trait WidgetTextInput: WidgetComponent {
    fn accepts_text_input(&self) -> bool {
        true
    }

    fn text_input_cursor_rect(&self) -> Rect {
        Rect::zero()
    }
}

/// 布局行为：尺寸、弹性、子节点排列。
pub trait WidgetLayout: WidgetComponent {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::zero())
    }
    fn flex_grow(&self) -> f32 {
        0.0
    }
    fn flex_shrink(&self) -> f32 {
        1.0
    }
    fn align_self(&self) -> Option<AlignItems> {
        None
    }
    fn grid_cell(&self) -> Option<usize> {
        None
    }
    fn grid_column_span(&self) -> u32 {
        1
    }
    fn grid_row_span(&self) -> u32 {
        1
    }
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
        children: &[ComponentId],
        _tree: &WidgetTree,
    ) -> Vec<LayoutChild> {
        children
            .iter()
            .copied()
            .map(|id| LayoutChild::new(id, Size::zero()))
            .collect()
    }
    /// Arrange children from the snapshot produced by [`Self::measure_children`].
    /// Implementations must not call child `measure` recursively here.
    fn layout_children(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
    ) -> Vec<(ComponentId, Rect)> {
        let _ = (frame, children, tree);
        Vec::new()
    }
}

/// 渲染行为：绘制、覆盖层、脏区域。
pub trait WidgetRender: WidgetComponent {
    fn render(&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree);
    fn uses_palette(&self) -> bool {
        true
    }
    fn draw_margin(&self) -> f32 {
        0.0
    }
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
    fn children_clip(&self, _frame: Rect) -> Option<Rect> {
        None
    }
    fn overlay_entry(&self, _id: ComponentId, _frame: Rect) -> Option<OverlayEntry> {
        None
    }
}

/// 事件行为：输入事件处理、滚动偏移、命中测试。
pub trait EventHandler: WidgetComponent {
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
    fn semantic_event(&self, _id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        None
    }
    /// 取走本次已处理事件产生的布局请求。
    ///
    /// 仅当组件的内部运行态改变了子节点布局时返回 `true`；纯绘制状态变化
    /// 保持 `false`，避免把普通交互扩大为布局遍历。
    fn take_layout_request(&mut self) -> bool {
        false
    }
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
    fn active_timer(&self) -> Option<(u64, Duration)> {
        None
    }
    fn wants_capture_phase(&self) -> bool {
        false
    }
    /// Opt in to receiving every PointerMove while the pointer remains inside
    /// the widget's hit-test frame. Default false enables the boundary-aware
    /// PointerMove fast path.
    fn wants_continuous_pointer_move(&self) -> bool {
        false
    }
    fn hit_test_frame(&self, actual_frame: Rect) -> Rect {
        actual_frame
    }
    /// 是否继续命中测试子节点。交互包装器可关闭它，让任意展示内容都由包装器接管。
    fn hit_test_children(&self) -> bool {
        true
    }
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
pub trait WidgetLifecycle: WidgetComponent {
    fn on_init(&mut self) {}
    fn on_attach(&mut self) {}
    fn on_mount(&mut self) {}
    fn on_active(&mut self) {}
    fn on_inactive(&mut self) {}
    fn on_theme_changed(&mut self) {}
    fn on_unmount(&mut self) {}
    fn on_detach(&mut self) {}
    fn on_destroy(&mut self) {}
}

/// Animation behavior advanced by the centralized App loop.
pub trait WidgetAnimation: WidgetComponent {
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
    fn into_node(self) -> WidgetNode;
}

impl<T: WidgetComponent + 'static> IntoWidgetNode for T {
    fn into_node(self) -> WidgetNode {
        WidgetNode::leaf(Box::new(self))
    }
}

impl IntoWidgetNode for WidgetNode {
    fn into_node(self) -> WidgetNode {
        self
    }
}
