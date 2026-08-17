//! 无外观的自定义标题栏交互原语。

use std::any::Any;

use crate::core::{ComponentId, Constraints, EdgeInsets, Rect, Size};
use crate::platform::windowing::{CursorType, KeyCode, MouseButton};
// 从平台公开值契约再导出缩放方向，保持 UI 与 prelude 使用路径集中。
pub use crate::platform::windowing::WindowResizeEdge;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::traits::{
    EventHandler, WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetRender,
};
use crate::ui::component_snapshot::SnapshotFields;
use crate::ui::event::WindowAction;
use crate::ui::layout::engine::BoxModel;
// 引入标准窗口控件组合使用的对齐契约。
use crate::ui::layout::{AlignItems, JustifyContent, LayoutChild};
// 引入主题中性颜色角色。
use crate::ui::theme::NeutralRole;
// 引入窗口交互区域与标准外观使用的样式类型。
use crate::ui::theme::style::{ColorValue, Style, StyleState, apply_style};
use crate::ui::view::{View, ViewNode};
// 引入标准窗口控件的容器、图标与行组合原语。
use crate::ui::widgets::{Container, Icon, row};
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

// 定义标准窗口控制的默认宽度。
const STANDARD_WINDOW_CONTROL_WIDTH: f32 = 46.0;
// 定义标准窗口控制的默认高度。
const STANDARD_WINDOW_CONTROL_HEIGHT: f32 = 40.0;
// 定义标准窗口控制图标尺寸。
const STANDARD_WINDOW_CONTROL_ICON_SIZE: f32 = 14.0;

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

    fn action(control: WindowControl) -> WindowAction {
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

impl WidgetComponent for WindowInteractionRegion {
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
        children: &[ComponentId],
        tree: &WidgetTree,
    ) -> Vec<LayoutChild> {
        self.container.measure_children(frame, children, tree)
    }

    fn layout_children(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
    ) -> Vec<(ComponentId, Rect)> {
        self.container.layout_children(frame, children, tree)
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
                self.pending = Some(Self::action(control));
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
                self.pending = Some(Self::action(control));
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

// 构造一个带标准外观与默认无障碍语义的窗口控制。
fn standard_window_control(
    // 接收平台窗口动作。
    control: WindowControl,
    // 接收 Lucide 图标名称。
    icon: &'static str,
) -> ViewNode {
    // 复用窗口交互包装器并只把图标作为展示内容。
    window_control(
        // 传入平台窗口动作。
        control,
        // 构造不形成嵌套交互目标的图标叶视图。
        ViewNode::leaf(Icon::new(icon).size(STANDARD_WINDOW_CONTROL_ICON_SIZE)),
    )
    // 采用标准标题栏控制宽度。
    .width(STANDARD_WINDOW_CONTROL_WIDTH)
    // 采用标准标题栏高度。
    .height(STANDARD_WINDOW_CONTROL_HEIGHT)
    // 在交叉轴居中图标。
    .align(AlignItems::Center)
    // 在主轴居中图标。
    .justify(JustifyContent::Center)
    // 使用当前主题的中性悬停背景。
    .bg_hover(ColorValue::Neutral(NeutralRole::FillSecondary))
    // 使用当前主题的中性焦点背景。
    .bg_focus(ColorValue::Neutral(NeutralRole::Fill))
    // 使用当前主题的中性按下背景。
    .bg_active(ColorValue::Neutral(NeutralRole::FillTertiary))
}

/// 构造标准最小化、最大化/还原与关闭窗口控制组合。
pub fn window_controls(
    // 控制是否包含最小化动作。
    show_minimize: bool,
    // 控制是否包含最大化/还原动作。
    show_maximize: bool,
    // 控制是否包含关闭动作。
    show_close: bool,
) -> ViewNode {
    // 按文档顺序预留最多三个控制节点。
    let mut controls = Vec::with_capacity(3);
    // 按声明决定是否加入最小化控制。
    if show_minimize {
        // 添加带标准语义与图标的最小化控制。
        controls.push(standard_window_control(
            // 使用平台最小化动作。
            WindowControl::Minimize,
            // 使用标准最小化图标。
            "minus",
        ));
    }
    // 按声明决定是否加入最大化/还原控制。
    if show_maximize {
        // 添加带标准语义与图标的最大化/还原控制。
        controls.push(standard_window_control(
            // 使用平台最大化/还原动作。
            WindowControl::MaximizeRestore,
            // 使用标准最大化图标。
            "maximize-2",
        ));
    }
    // 按声明决定是否加入关闭控制。
    if show_close {
        // 添加带标准语义与图标的关闭控制。
        controls.push(standard_window_control(
            // 使用平台关闭动作。
            WindowControl::Close,
            // 使用标准关闭图标。
            "x",
        ));
    }
    // 使用无间距行容器保持标准标题栏排列。
    row(controls)
}

// 集中验证标准窗口控制组合的公开结构契约。
#[cfg(test)]
mod tests {
    // 引入当前模块的窗口控制构造器与快照类型。
    use super::*;
    // 引入构造指针事件所需的平台中立输入值。
    use crate::core::Point;
    // 引入空修饰键掩码。
    use crate::platform::windowing::KeyMod;

    // 提取组合节点直接子项的窗口动作顺序。
    fn child_controls(node: &ViewNode) -> Vec<WindowControl> {
        // 把每个交互包装器快照投影为窗口动作。
        node.children
            // 按源码顺序遍历直接子项。
            .iter()
            // 要求每个子项都是窗口控制交互包装器。
            .map(|child| match child.widget.snapshot_fields() {
                // 返回快照持有的窗口动作。
                SnapshotFields::WindowControl { control, .. } => control,
                // 非窗口控制子项表示组合契约被破坏。
                _ => panic!("window_controls 只能包含窗口控制子项"),
            })
            // 收集稳定动作顺序。
            .collect()
    }

    // 验证默认三按钮集合保持文档顺序。
    #[test]
    fn standard_controls_preserve_documented_order() {
        // 构造全部标准窗口控制。
        let controls = window_controls(true, true, true);
        // 顺序必须固定为最小化、最大化/还原、关闭。
        assert_eq!(
            // 提取实际动作顺序。
            child_controls(&controls),
            // 声明文档化动作顺序。
            vec![
                // 首项为最小化。
                WindowControl::Minimize,
                // 次项为最大化/还原。
                WindowControl::MaximizeRestore,
                // 末项为关闭。
                WindowControl::Close,
            ]
        );
        // 每个标准动作必须保留非空的默认无障碍名称。
        assert!(controls.children.iter().all(
            // 检查每个交互包装器的快照语义。
            |child| matches!(
                // 读取组件公开快照字段。
                child.widget.snapshot_fields(),
                // 只接受带非空名称的窗口控制快照。
                SnapshotFields::WindowControl { accessible_name, .. } if !accessible_name.is_empty()
            )
        ));
    }

    // 验证显示开关只影响对应动作节点。
    #[test]
    fn standard_controls_respect_visibility_flags() {
        // 只构造最大化/还原控制。
        let maximize_only = window_controls(false, true, false);
        // 组合中只能保留最大化/还原动作。
        assert_eq!(
            // 提取选择性组合动作。
            child_controls(&maximize_only),
            // 声明唯一预期动作。
            vec![WindowControl::MaximizeRestore]
        );
        // 全部关闭时仍返回合法的空行视图。
        let hidden = window_controls(false, false, false);
        // 空组合不能残留任何交互节点。
        assert!(hidden.children.is_empty());
    }

    // 验证公开缩放热区同时表达光标与精确原生动作。
    #[test]
    fn resize_region_maps_cursor_and_pointer_action() {
        // 声明全部方向与标准平台缩放光标的完整映射。
        let cursor_cases = [
            // 上边使用垂直缩放光标。
            (WindowResizeEdge::Top, CursorType::ResizeV),
            // 下边使用垂直缩放光标。
            (WindowResizeEdge::Bottom, CursorType::ResizeV),
            // 左边使用水平缩放光标。
            (WindowResizeEdge::Left, CursorType::ResizeH),
            // 右边使用水平缩放光标。
            (WindowResizeEdge::Right, CursorType::ResizeH),
            // 左上角使用西北到东南缩放光标。
            (WindowResizeEdge::TopLeft, CursorType::ResizeNW),
            // 右上角使用东北到西南缩放光标。
            (WindowResizeEdge::TopRight, CursorType::ResizeNE),
            // 左下角使用东北到西南缩放光标。
            (WindowResizeEdge::BottomLeft, CursorType::ResizeNE),
            // 右下角使用西北到东南缩放光标。
            (WindowResizeEdge::BottomRight, CursorType::ResizeNW),
        ];
        // 逐一验证公开热区附带正确光标。
        for (edge, expected_cursor) in cursor_cases {
            // 使用无外观内容构造当前方向的窗口缩放热区。
            let region = window_resize_region(edge, ViewNode::leaf(Container::new()));
            // 热区必须显式保存与方向一致的平台光标。
            assert_eq!(region.cursor, Some(expected_cursor));
        }
        // 直接构造同一方向的交互组件以验证事件动作。
        let mut interaction = WindowInteractionRegion::resize(WindowResizeEdge::BottomRight);
        // 主按钮按下必须由缩放热区消费。
        assert_eq!(
            // 提交平台中立的左键 PointerDown。
            interaction.on_event(&SystemEvent::PointerDown {
                // 坐标由窗口管理器使用当前原生事件解释。
                pos: Point::new(3.0, 4.0),
                // 只允许主按钮启动原生缩放。
                button: MouseButton::Left,
                // 本场景没有键盘修饰键。
                mods: KeyMod::NONE,
            }),
            // 缩放热区必须终止子节点传播。
            EventResult::Handled,
        );
        // 动作必须保留原始右下角方向，不能退化为通用移动。
        assert_eq!(
            interaction.take_window_action(),
            Some(WindowAction::BeginResizeDrag(WindowResizeEdge::BottomRight)),
        );
    }
}
