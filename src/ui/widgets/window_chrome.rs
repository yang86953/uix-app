//! 无外观的自定义标题栏交互原语。

use std::any::Any;

use crate::core::{ComponentId, Constraints, EdgeInsets, Rect, Size};
use crate::platform::windowing::{KeyCode, MouseButton};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::component::traits::{
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
    Minimize,
    /// 在最大化与还原之间切换。
    MaximizeRestore,
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
            WindowInteraction::Drag => SnapshotFields::Unknown,
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
}
