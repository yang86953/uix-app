//! Component-owned dynamic child factories scheduled by the framework.
use crate::core::WidgetId;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{ViewNode, Widget};

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
    fn defer_view_children(&self, _current: &dyn Widget, _next: &dyn Widget) -> bool {
        false
    }
    fn reserved_child_keys(&self) -> &'static [&'static str] {
        &[]
    }
    fn refresh(
        &self,
        _context: &mut ComponentContext<'_>,
        _phase: DynamicRefresh,
        _viewport_height: Option<f32>,
    ) -> bool {
        false
    }
    fn reconcile(
        &self,
        _context: &mut ComponentContext<'_>,
        input: DynamicChildInput,
    ) -> Result<bool, DynamicChildInput> {
        Err(input)
    }
}

impl WidgetTree {
    pub(crate) fn dynamic_children_coordinator(
        &self,
        id: WidgetId,
    ) -> Option<&'static dyn DynamicChildrenCoordinator> {
        self.get(id)?.widget().dynamic_children_coordinator()
    }
    pub(crate) fn refresh_dynamic_children(
        &mut self,
        id: WidgetId,
        phase: DynamicRefresh,
        viewport_height: Option<f32>,
    ) -> bool {
        let Some(coordinator) = self.dynamic_children_coordinator(id) else {
            return false;
        };
        coordinator.refresh(&mut ComponentContext::new(self, id), phase, viewport_height)
    }
    pub(crate) fn render_handlers_mut(&mut self) -> &mut super::render_handler::RenderHandlerTable {
        &mut self.render_handler_table
    }
    pub(crate) fn render_handlers(&self) -> &super::render_handler::RenderHandlerTable {
        &self.render_handler_table
    }
}

/// A component's synchronous coordination capability. The framework owns its
/// construction and fixes the owner; tree mutation and lifecycle transactions
/// are available only through these child operations.
pub struct ComponentContext<'a> {
    tree: &'a mut WidgetTree,
    owner: WidgetId,
}
impl<'a> ComponentContext<'a> {
    pub(crate) fn new(tree: &'a mut WidgetTree, owner: WidgetId) -> Self {
        Self { tree, owner }
    }
    pub fn owner(&self) -> WidgetId {
        self.owner
    }
    pub fn accepts_coordination_work(&self) -> bool {
        self.tree.accepts_coordination_work()
    }
    fn contains(&self, id: WidgetId) -> bool {
        let mut cursor = Some(id);
        while let Some(id) = cursor {
            let Some(node) = self.tree.get(id) else {
                return false;
            };
            if id == self.owner {
                return true;
            }
            cursor = node.parent();
        }
        false
    }
    /// Read the owner or one of its descendants. Other subtrees are inaccessible.
    pub fn get(&self, id: WidgetId) -> Option<&crate::ui::widget_runtime::widget::BoxedWidget> {
        self.contains(id).then(|| self.tree.get(id)).flatten()
    }
    pub fn is_pending_removal_subtree(&self, id: WidgetId) -> bool {
        !self.contains(id) || self.tree.is_pending_removal_subtree(id)
    }
    pub fn renderer<T: std::any::Any>(&self) -> Option<&T> {
        self.tree.render_handlers().get::<T>(self.owner)
    }
    pub fn renderer_mut<T: std::any::Any>(&mut self) -> Option<&mut T> {
        self.tree.render_handlers_mut().get_mut::<T>(self.owner)
    }
    pub fn capture_context(&self) -> crate::ui::adapter::DynamicViewCaptureContext {
        crate::ui::adapter::ViewAdapter::dynamic_capture_context(self.tree, self.owner)
    }
    pub fn reconcile_children(&mut self, children: Vec<ViewNode>) -> bool {
        crate::ui::adapter::ViewAdapter::reconcile_dynamic_children(self.tree, self.owner, children)
    }
    pub fn append_child(&mut self, child: ViewNode) -> bool {
        crate::ui::adapter::ViewAdapter::append_dynamic_child(self.tree, self.owner, child)
    }
    pub fn remove_child(&mut self, child: WidgetId) -> bool {
        crate::ui::adapter::ViewAdapter::remove_dynamic_child(self.tree, self.owner, child)
    }
    pub fn cancel_child_removal(&mut self, child: WidgetId) -> bool {
        crate::ui::adapter::ViewAdapter::cancel_dynamic_child_removal(self.tree, self.owner, child)
    }
}
