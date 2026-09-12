//! Component-owned dynamic child factories scheduled by the framework.
use crate::core::WidgetId;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{Widget, ViewNode};

/// Stable lifecycle points at which a component may materialize dynamic children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicRefresh {
    DirectMount,
    Mount,
    Animation,
    Content,
    Fallback,
    VirtualCells,
    Viewport,
    Event,
    AfterReconcile,
}

/// Children captured from the next declaration, before a component's dynamic work.
pub struct DynamicChildInput {
    pub authored: Vec<ViewNode>,
    pub built: Vec<ViewNode>,
    pub deferred: bool,
}

/// Extension for components whose child factories need a live tree-owned capture scope.
/// The coordinator has no mutable component borrow while it coordinates the tree.
pub trait DynamicChildrenCoordinator: Send + Sync {
    fn defer_view_children(&self, _current: &dyn Widget, _next: &dyn Widget) -> bool { false }
    fn reserved_child_keys(&self) -> &'static [&'static str] { &[] }
    fn refresh(&self, _tree: &mut WidgetTree, _owner: WidgetId, _phase: DynamicRefresh, _viewport_height: Option<f32>) -> bool { false }
    fn reconcile(&self, _tree: &mut WidgetTree, _owner: WidgetId, input: DynamicChildInput) -> Result<bool, DynamicChildInput> { Err(input) }
}

impl WidgetTree {
    pub(crate) fn dynamic_children_coordinator(&self, id: WidgetId) -> Option<&'static dyn DynamicChildrenCoordinator> {
        self.get(id)?.widget().dynamic_children_coordinator()
    }
    pub(crate) fn refresh_dynamic_children(&mut self, id: WidgetId, phase: DynamicRefresh, viewport_height: Option<f32>) -> bool {
        let Some(coordinator) = self.dynamic_children_coordinator(id) else { return false; };
        coordinator.refresh(self, id, phase, viewport_height)
    }
    pub fn render_handlers_mut(&mut self) -> &mut super::render_handler::RenderHandlerTable { &mut self.render_handler_table }
    pub fn render_handlers(&self) -> &super::render_handler::RenderHandlerTable {
        &self.render_handler_table
    }
}
