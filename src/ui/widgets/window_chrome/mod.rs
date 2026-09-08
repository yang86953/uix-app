//! 无外观的自定义标题栏交互原语。

use std::any::Any;

use crate::core::{Constraints, EdgeInsets, Rect, Size, WidgetId};
use crate::platform::windowing::{CursorType, KeyCode, MouseButton};
// 从平台公开值契约再导出缩放方向，保持 UI 与 prelude 使用路径集中。
pub use crate::platform::windowing::WindowResizeEdge;
use crate::ui::event::WindowAction;
use crate::ui::layout::engine::BoxModel;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::traits::{
    EventHandler, Widget, WidgetCapabilities, WidgetLayout, WidgetRender,
};
use crate::ui::widget_snapshot::SnapshotFields;
// 引入窗口交互区域使用的对齐与子布局契约。
use crate::ui::layout::{AlignItems, FlexDirection, LayoutChild};
// 引入窗口交互区域使用的样式类型。
use crate::ui::theme::style::{Style, StyleState, apply_style};
use crate::ui::view::{View, ViewNode};
// 引入窗口交互区域复用的基础容器。
use crate::ui::widgets::Container;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

/// 自定义标题栏可触发的标准窗口控制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControl {
    /// 将窗口最小化到系统任务栏。
    Minimize,
    /// 在最大化与还原之间切换。
    MaximizeRestore,
    /// 请求关闭当前窗口。
    Close,
}

impl WindowControl {
    const fn default_accessible_name(self) -> &'static str {
        match self {
            Self::Minimize => "Minimize window",
            Self::MaximizeRestore => "Maximize or restore window",
            Self::Close => "Close window",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowInteraction {
    Drag,
    Resize(WindowResizeEdge),
    Control(WindowControl),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlActivation {
    Pointer,
    Keyboard(KeyCode),
}

/// 布局、绘制均复用普通容器；该组件只增加窗口交互语义，不规定任何外观。
pub(crate) struct WindowInteractionRegion {
    interaction: WindowInteraction,
    accessible_name: Option<String>,
    container: Container,
    hovered: bool,
    focused: bool,
    activation: Option<ControlActivation>,
    system_menu_armed: bool,
    pending: Option<WindowAction>,
}

impl WindowInteractionRegion {
    fn drag() -> Self {
        Self::new(WindowInteraction::Drag, None)
    }

    fn control(control: WindowControl, accessible_name: String) -> Self {
        Self::new(WindowInteraction::Control(control), Some(accessible_name))
    }

    // 构造不参与键盘导航的窗口调整大小热区。
    fn resize(edge: WindowResizeEdge) -> Self {
        // 缩放动作由平台窗口管理器接管，不需要无障碍控件名称。
        Self::new(WindowInteraction::Resize(edge), None)
    }

    fn new(interaction: WindowInteraction, accessible_name: Option<String>) -> Self {
        Self {
            interaction,
            accessible_name,
            container: Container::new(),
            hovered: false,
            focused: false,
            activation: None,
            system_menu_armed: false,
            pending: None,
        }
    }

    pub(crate) fn is_drag_region(&self) -> bool {
        self.interaction == WindowInteraction::Drag
    }

    /// 把公开窗口控件映射为平台中立窗口动作。
    ///
    /// 指针、键盘与无障碍语义入口必须复用此映射，避免三条输入路径各自
    /// 维护关闭、最小化和最大化语义而发生漂移。
    pub(crate) const fn window_action(control: WindowControl) -> WindowAction {
        match control {
            WindowControl::Minimize => WindowAction::Minimize,
            WindowControl::MaximizeRestore => WindowAction::MaximizeRestore,
            WindowControl::Close => WindowAction::RequestClose,
        }
    }

    pub(crate) fn apply_view_style(
        &mut self,
        style: &Style,
        flex_grow: Option<f32>,
        flex_shrink: Option<f32>,
    ) {
        if style != &Style::default() {
            self.container.style = self.container.style.clone().apply(style.clone());
        }
        if let Some(grow) = flex_grow {
            self.container.style.flex_grow = grow;
        }
        if let Some(shrink) = flex_shrink {
            self.container.style.flex_shrink = shrink;
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        if self.interaction != next.interaction {
            self.hovered = false;
            self.focused = false;
            self.activation = None;
            self.system_menu_armed = false;
            self.pending = None;
        }
        self.interaction = next.interaction;
        self.accessible_name = next.accessible_name;
        self.container.sync_from(next.container);
    }
}

impl Widget for WindowInteractionRegion {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn snapshot_fields(&self) -> SnapshotFields {
        match self.interaction {
            WindowInteraction::Drag | WindowInteraction::Resize(_) => SnapshotFields::Unknown,
            WindowInteraction::Control(control) => SnapshotFields::WindowControl {
                control,
                accessible_name: self.accessible_name.clone().unwrap_or_default(),
            },
        }
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(
            WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER | WidgetCapabilities::EVENT,
        )
    }

    fn may_produce_overlay(&self) -> bool {
        false
    }

    fn tab_index(&self) -> i32 {
        i32::from(matches!(self.interaction, WindowInteraction::Control(_)))
    }

    fn exposes_semantic_children(&self) -> bool {
        matches!(self.interaction, WindowInteraction::Drag)
    }

    fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        Some(self)
    }

    fn as_render(&self) -> Option<&dyn WidgetRender> {
        Some(self)
    }

    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        Some(self)
    }

    fn as_event(&self) -> Option<&dyn EventHandler> {
        Some(self)
    }

    fn as_event_mut(&mut self) -> Option<&mut dyn EventHandler> {
        Some(self)
    }
}

impl WidgetLayout for WindowInteractionRegion {
    fn measure(&self, constraints: Constraints) -> Size {
        self.container.measure(constraints)
    }

    fn measure_natural(&self, constraints: Constraints) -> Size {
        WidgetLayout::measure_natural(&self.container, constraints)
    }

    fn flex_layout_axes(&self) -> Option<(FlexDirection, AlignItems)> {
        WidgetLayout::flex_layout_axes(&self.container)
    }

    fn flex_basis(&self, parent_direction: FlexDirection) -> Option<f32> {
        WidgetLayout::flex_basis(&self.container, parent_direction)
    }

    fn minimum_size(&self) -> Size {
        WidgetLayout::minimum_size(&self.container)
    }

    fn flex_grow(&self) -> f32 {
        WidgetLayout::flex_grow(&self.container)
    }

    fn flex_shrink(&self) -> f32 {
        WidgetLayout::flex_shrink(&self.container)
    }

    fn align_self(&self) -> Option<AlignItems> {
        self.container.align_self()
    }

    fn grid_cell(&self) -> Option<usize> {
        self.container.grid_cell()
    }

    fn grid_column_span(&self) -> u32 {
        self.container.grid_column_span()
    }

    fn grid_row_span(&self) -> u32 {
        self.container.grid_row_span()
    }

    fn layout_margin(&self) -> EdgeInsets {
        self.container.layout_margin()
    }

    fn measure_children(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
    ) -> Vec<LayoutChild> {
        self.container.measure_children(frame, children, tree)
    }

    fn measure_children_into(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>,
    ) {
        self.container
            .measure_children_into(frame, children, tree, output);
    }

    fn layout_children(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        self.container.layout_children(frame, children, tree)
    }

    fn layout_children_into(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
        scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        self.container
            .layout_children_into(frame, children, tree, scratch, output);
    }
}

impl WidgetRender for WindowInteractionRegion {
    fn render(&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let mut style = self.container.style.clone();
        style.background = style.effective_bg_for_state(StyleState {
            hovered: self.hovered,
            pressed: self.activation.is_some(),
            focused: self.focused,
            disabled: false,
        });
        let visual = BoxModel {
            margin: style.margin,
            border_width: style.border_width,
            padding: style.padding,
        }
        .visual_rect(frame);
        if visual.w > 0.0 && visual.h > 0.0 {
            apply_style(ctx, visual, &style);
        }
    }

    fn dirty_rect(&self, frame: Rect) -> Rect {
        self.container.dirty_rect(frame)
    }
}

impl EventHandler for WindowInteractionRegion {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        match (self.interaction, event) {
            (
                WindowInteraction::Drag,
                SystemEvent::PointerDoubleClick {
                    button: MouseButton::Left,
                    ..
                },
            ) => {
                self.pending = Some(WindowAction::ToggleMaximizeFromTitleBar);
                EventResult::Handled
            }
            (
                WindowInteraction::Resize(edge),
                SystemEvent::PointerDown {
                    button: MouseButton::Left,
                    ..
                },
            ) => {
                // 把当前 PointerDown 交给事件所属窗口的原生调整大小动作。
                self.pending = Some(WindowAction::BeginResizeDrag(edge));
                // 缩放热区拥有该指针手势，内容节点只负责展示。
                EventResult::Handled
            }
            (
                WindowInteraction::Drag,
                SystemEvent::PointerDown {
                    button: MouseButton::Right,
                    ..
                },
            ) => {
                self.system_menu_armed = true;
                EventResult::Handled
            }
            (
                WindowInteraction::Drag,
                SystemEvent::PointerUp {
                    button: MouseButton::Right,
                    ..
                },
            ) if self.system_menu_armed => {
                self.system_menu_armed = false;
                self.pending = Some(WindowAction::ShowSystemMenuFromTitleBar);
                EventResult::Handled
            }
            (WindowInteraction::Drag, SystemEvent::PointerUp { .. }) => {
                self.system_menu_armed = false;
                EventResult::NotHandled
            }
            (WindowInteraction::Drag, SystemEvent::PointerLeave) => {
                self.system_menu_armed = false;
                EventResult::NotHandled
            }
            (
                WindowInteraction::Drag,
                SystemEvent::PointerDown {
                    button: MouseButton::Left,
                    ..
                },
            ) => {
                self.pending = Some(WindowAction::BeginMoveDrag);
                EventResult::Handled
            }
            (
                WindowInteraction::Control(_),
                SystemEvent::PointerDown {
                    button: MouseButton::Left,
                    ..
                },
            ) => {
                if self.activation.is_none() {
                    self.activation = Some(ControlActivation::Pointer);
                }
                EventResult::Handled
            }
            (
                WindowInteraction::Control(control),
                SystemEvent::PointerUp {
                    button: MouseButton::Left,
                    ..
                },
            ) if self.activation == Some(ControlActivation::Pointer) => {
                self.activation = None;
                self.pending = Some(Self::window_action(control));
                EventResult::Handled
            }
            (WindowInteraction::Control(_), SystemEvent::PointerEnter) => {
                self.hovered = true;
                EventResult::Handled
            }
            (WindowInteraction::Control(_), SystemEvent::PointerLeave) => {
                self.hovered = false;
                if self.activation == Some(ControlActivation::Pointer) {
                    self.activation = None;
                }
                EventResult::Handled
            }
            (WindowInteraction::Control(_), SystemEvent::FocusIn) => {
                self.focused = true;
                EventResult::Handled
            }
            (WindowInteraction::Control(_), SystemEvent::FocusOut) => {
                self.focused = false;
                self.activation = None;
                EventResult::Handled
            }
            (WindowInteraction::Control(_), SystemEvent::KeyDown { key, .. })
                if matches!(key, KeyCode::Enter | KeyCode::Space) =>
            {
                if self.activation.is_none() {
                    self.activation = Some(ControlActivation::Keyboard(*key));
                }
                EventResult::Handled
            }
            (WindowInteraction::Control(control), SystemEvent::KeyUp { key, .. })
                if self.activation == Some(ControlActivation::Keyboard(*key)) =>
            {
                self.activation = None;
                self.pending = Some(Self::window_action(control));
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    fn take_window_action(&mut self) -> Option<WindowAction> {
        self.pending.take()
    }

    fn hit_test_children(&self) -> bool {
        matches!(self.interaction, WindowInteraction::Drag)
    }
}

/// 将任意展示 View 包装为原生窗口拖动区域。
///
/// 包装器不提供背景、字号或固定高度；应用可像普通 View 一样完整定制外观。
pub fn window_drag_region(content: impl View) -> ViewNode {
    ViewNode::new(WindowInteractionRegion::drag(), vec![content.build()])
}

/// 将任意展示 View 包装为自定义窗口的调整大小热区。
///
/// 应用应在自定义非客户区的四条边与四个角放置对应方向的热区；包装器会同时
/// 设置平台等价的缩放光标，并在主按钮按下时把手势移交给窗口管理器。
pub fn window_resize_region(edge: WindowResizeEdge, content: impl View) -> ViewNode {
    // 根据缩放方向选择用户可见的标准平台光标。
    let cursor = match edge {
        // 左右边共享水平缩放光标。
        WindowResizeEdge::Left | WindowResizeEdge::Right => CursorType::ResizeH,
        // 上下边共享垂直缩放光标。
        WindowResizeEdge::Top | WindowResizeEdge::Bottom => CursorType::ResizeV,
        // 左上与右下共享西北方向缩放光标。
        WindowResizeEdge::TopLeft | WindowResizeEdge::BottomRight => CursorType::ResizeNW,
        // 右上与左下共享东北方向缩放光标。
        WindowResizeEdge::TopRight | WindowResizeEdge::BottomLeft => CursorType::ResizeNE,
    };
    // 内容不形成嵌套交互目标，整个包装节点作为原生缩放热区。
    ViewNode::new(
        // 保存精确缩放边，供事件边界构造窗口动作。
        WindowInteractionRegion::resize(edge),
        // 展示内容仍由调用方完全定制。
        vec![content.build()],
    )
    // 显式设置与缩放方向一致的平台光标。
    .cursor(cursor)
}

/// 将任意展示 View 包装为自定义标题栏窗口控制。
///
/// 包装器自身负责鼠标与键盘交互，因此内容只用于展示，不形成嵌套交互目标。
pub fn window_control(control: WindowControl, content: impl View) -> ViewNode {
    window_control_named(control, control.default_accessible_name(), content)
}

/// 将展示 View 包装为带自定义无障碍名称的窗口控制。
///
/// 应用使用本地化名称或图标内容无法表达动作时，应优先使用此函数。
pub fn window_control_named(
    control: WindowControl,
    accessible_name: impl Into<String>,
    content: impl View,
) -> ViewNode {
    ViewNode::new(
        WindowInteractionRegion::control(control, accessible_name.into()),
        vec![content.build()],
    )
}

// 集中验证标准窗口控制组合的公开结构契约。
#[cfg(test)]
// 测试继续从窗口交互内核边界联合验证独立 UIX 组合。
use super::window_controls::window_controls;
