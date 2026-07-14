//! 无外观的自定义标题栏交互原语。

use std::any::Any;

use crate::core::{ComponentId, Constraints, EdgeInsets, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::native::traits::input::{KeyCode, MouseButton};
use crate::ui::component_snapshot::SnapshotFields;
use crate::ui::event::WindowAction;
use crate::ui::layout::{AlignItems, LayoutChild};
use crate::ui::style::Style;
use crate::ui::traits::{
    EventHandler, WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetRender,
};
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::Container;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

/// 自定义标题栏可触发的标准窗口控制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControl {
    Minimize,
    /// 在最大化与还原之间切换。
    MaximizeRestore,
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
    Control(WindowControl),
}

/// 布局、绘制均复用普通容器；该组件只增加窗口交互语义，不规定任何外观。
pub(crate) struct WindowInteractionRegion {
    interaction: WindowInteraction,
    accessible_name: Option<String>,
    container: Container,
    armed: bool,
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
            armed: false,
            pending: None,
        }
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
            self.armed = false;
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
    fn render(&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.container.render(frame, ctx, tree);
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
                self.armed = true;
                EventResult::Handled
            }
            (
                WindowInteraction::Control(control),
                SystemEvent::PointerUp {
                    button: MouseButton::Left,
                    ..
                },
            ) if self.armed => {
                self.armed = false;
                self.pending = Some(Self::action(control));
                EventResult::Handled
            }
            (WindowInteraction::Control(_), SystemEvent::PointerLeave | SystemEvent::FocusOut) => {
                self.armed = false;
                EventResult::Handled
            }
            (
                WindowInteraction::Control(_),
                SystemEvent::KeyDown {
                    key: KeyCode::Enter | KeyCode::Space,
                    ..
                },
            ) => {
                self.armed = true;
                EventResult::Handled
            }
            (
                WindowInteraction::Control(control),
                SystemEvent::KeyUp {
                    key: KeyCode::Enter | KeyCode::Space,
                    ..
                },
            ) if self.armed => {
                self.armed = false;
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
        false
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
