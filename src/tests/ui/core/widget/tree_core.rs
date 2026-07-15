// WidgetTree unit tests.
// Split out of `tree_core.rs` to keep implementation files manageable.
use crate::tests::common::*;
use crate::ui::core::widget::*;
use crate::ui::layout::engine::{child_from_tree, child_from_tree_with_constraints};
use crate::ui::managers::StyleManager;
use crate::ui::view::combinators::{label, space};
use crate::ui::view::{column, column_fit, embed, row, scroll, ViewAdapter};
use crate::ui::widgets::Container;
use crate::ui::{Button, Drawer, Grid, Label, Modal, OverlayEntry, QRCode, TextManager, Tooltip};

struct SpyWidget {
    size: crate::core::Size,
    tab_index: i32,
    last_event: RefCell<Option<SystemEvent>>,
    events: RefCell<Vec<SystemEvent>>,
}
impl SpyWidget {
    fn new(w: f32, h: f32) -> Self {
        Self {
            size: crate::core::Size::new(w, h),
            tab_index: 0,
            last_event: RefCell::new(None),
            events: RefCell::new(Vec::new()),
        }
    }

    fn with_tab_index(mut self, tab_index: i32) -> Self {
        self.tab_index = tab_index;
        self
    }
}
impl WidgetComponent for SpyWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(
            WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER | WidgetCapabilities::EVENT,
        )
    }
    fn tab_index(&self) -> i32 {
        self.tab_index
    }
    crate::wc_upcast!(SpyWidget; WidgetLayout);
    crate::wc_upcast!(SpyWidget; WidgetRender);
    crate::wc_upcast!(SpyWidget; EventHandler);
}
impl WidgetLayout for SpyWidget {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.size)
    }
}
impl WidgetRender for SpyWidget {
    fn render(&self, _: Rect, _: &mut crate::draw::painting::PaintContext, _: &WidgetTree) {}
}
impl EventHandler for SpyWidget {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        *self.last_event.borrow_mut() = Some(event.clone());
        self.events.borrow_mut().push(event.clone());
        EventResult::Handled
    }
}

struct ScrollCompositeViewportProbe {
    pending_delta: Cell<Option<(f32, f32)>>,
    viewport: Rect,
}

impl WidgetComponent for ScrollCompositeViewportProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::EVENT)
    }

    crate::wc_upcast!(ScrollCompositeViewportProbe; EventHandler);
}

impl EventHandler for ScrollCompositeViewportProbe {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        if matches!(event, SystemEvent::Wheel { .. }) {
            self.pending_delta.set(Some((0.0, 10.0)));
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    fn scroll_delta_for_dirty(&self) -> Option<(f32, f32)> {
        self.pending_delta.take()
    }

    fn scroll_composite_viewport(&self, _frame: Rect) -> Option<Rect> {
        Some(self.viewport)
    }
}

struct CaptureSpyWidget(SpyWidget);

impl CaptureSpyWidget {
    fn new(w: f32, h: f32) -> Self {
        Self(SpyWidget::new(w, h))
    }
}

impl WidgetComponent for CaptureSpyWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        self.0.capabilities()
    }
    crate::wc_upcast!(CaptureSpyWidget; WidgetLayout);
    crate::wc_upcast!(CaptureSpyWidget; WidgetRender);
    crate::wc_upcast!(CaptureSpyWidget; EventHandler);
}

impl WidgetLayout for CaptureSpyWidget {
    fn measure(&self, constraints: Constraints) -> Size {
        self.0.measure(constraints)
    }
}

impl WidgetRender for CaptureSpyWidget {
    fn render(
        &self,
        frame: Rect,
        ctx: &mut crate::draw::painting::PaintContext,
        tree: &WidgetTree,
    ) {
        self.0.render(frame, ctx, tree)
    }
}

impl EventHandler for CaptureSpyWidget {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        self.0.on_event(event)
    }

    fn wants_capture_phase(&self) -> bool {
        true
    }
}

struct MeasureOnlyWidget;

impl WidgetComponent for MeasureOnlyWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT)
    }
    crate::wc_upcast!(MeasureOnlyWidget; WidgetLayout);
}

impl WidgetLayout for MeasureOnlyWidget {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(120.0, 80.0))
    }
}

struct ConstraintProbeWidget {
    size: Size,
    seen: Rc<RefCell<Vec<Constraints>>>,
}

impl ConstraintProbeWidget {
    fn new(size: Size, seen: Rc<RefCell<Vec<Constraints>>>) -> Self {
        Self { size, seen }
    }
}

impl WidgetComponent for ConstraintProbeWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT)
    }
    crate::wc_upcast!(ConstraintProbeWidget; WidgetLayout);
}

impl WidgetLayout for ConstraintProbeWidget {
    fn measure(&self, constraints: Constraints) -> Size {
        self.seen.borrow_mut().push(constraints);
        constraints.clamp(self.size)
    }
}

struct ContinuousSpyWidget(SpyWidget);

impl ContinuousSpyWidget {
    fn new(w: f32, h: f32) -> Self {
        Self(SpyWidget::new(w, h))
    }
}

impl WidgetComponent for ContinuousSpyWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        self.0.capabilities()
    }
    crate::wc_upcast!(ContinuousSpyWidget; WidgetLayout);
    crate::wc_upcast!(ContinuousSpyWidget; WidgetRender);
    crate::wc_upcast!(ContinuousSpyWidget; EventHandler);
}

impl WidgetLayout for ContinuousSpyWidget {
    fn measure(&self, constraints: Constraints) -> Size {
        self.0.measure(constraints)
    }
}

impl WidgetRender for ContinuousSpyWidget {
    fn render(
        &self,
        frame: Rect,
        ctx: &mut crate::draw::painting::PaintContext,
        tree: &WidgetTree,
    ) {
        self.0.render(frame, ctx, tree)
    }
}

impl EventHandler for ContinuousSpyWidget {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        self.0.on_event(event)
    }

    fn wants_continuous_pointer_move(&self) -> bool {
        true
    }
}

struct PassThroughContainer {
    size: crate::core::Size,
    children: RefCell<Vec<Box<dyn WidgetComponent>>>,
}
impl PassThroughContainer {
    fn new(w: f32, h: f32, children: Vec<Box<dyn WidgetComponent>>) -> Self {
        Self {
            size: crate::core::Size::new(w, h),
            children: RefCell::new(children),
        }
    }
}
impl WidgetComponent for PassThroughContainer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(
            WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER | WidgetCapabilities::EVENT,
        )
    }
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
        std::mem::take(&mut *self.children.borrow_mut())
    }
    crate::wc_upcast!(PassThroughContainer; WidgetLayout);
    crate::wc_upcast!(PassThroughContainer; WidgetRender);
    crate::wc_upcast!(PassThroughContainer; EventHandler);
}
impl WidgetLayout for PassThroughContainer {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.size)
    }
}
impl WidgetRender for PassThroughContainer {
    fn render(&self, _: Rect, _: &mut crate::draw::painting::PaintContext, _: &WidgetTree) {}
}
impl EventHandler for PassThroughContainer {
    fn on_event(&mut self, _: &SystemEvent) -> EventResult {
        EventResult::NotHandled
    }
}

struct ClipContainer {
    size: crate::core::Size,
    child_y: f32,
    children: RefCell<Vec<Box<dyn WidgetComponent>>>,
}

impl ClipContainer {
    fn new(w: f32, h: f32, child_y: f32, children: Vec<Box<dyn WidgetComponent>>) -> Self {
        Self {
            size: crate::core::Size::new(w, h),
            child_y,
            children: RefCell::new(children),
        }
    }
}

impl WidgetComponent for ClipContainer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER)
    }
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
        std::mem::take(&mut *self.children.borrow_mut())
    }
    crate::wc_upcast!(ClipContainer; WidgetLayout);
    crate::wc_upcast!(ClipContainer; WidgetRender);
}

impl WidgetLayout for ClipContainer {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.size)
    }

    fn measure_children(
        &self,
        _frame: Rect,
        children: &[ComponentId],
        tree: &WidgetTree,
    ) -> Vec<crate::ui::LayoutChild> {
        children
            .iter()
            .copied()
            .map(|id| child_from_tree_with_constraints(id, tree, Constraints::unconstrained()))
            .collect()
    }

    fn layout_children(
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        _tree: &WidgetTree,
    ) -> Vec<(ComponentId, Rect)> {
        children
            .iter()
            .map(|child| {
                (
                    child.id,
                    Rect::new(
                        frame.x,
                        frame.y + self.child_y,
                        child.measured_size.w,
                        child.measured_size.h,
                    ),
                )
            })
            .collect()
    }
}

impl WidgetRender for ClipContainer {
    fn render(&self, _: Rect, _: &mut crate::draw::painting::PaintContext, _: &WidgetTree) {}

    fn uses_palette(&self) -> bool {
        false
    }

    fn children_clip(&self, frame: Rect) -> Option<Rect> {
        Some(frame)
    }
}

struct ScrollClipContainer {
    size: crate::core::Size,
    child_y: f32,
    scroll_y: f32,
    children: RefCell<Vec<Box<dyn WidgetComponent>>>,
}

impl ScrollClipContainer {
    fn new(
        w: f32,
        h: f32,
        child_y: f32,
        scroll_y: f32,
        children: Vec<Box<dyn WidgetComponent>>,
    ) -> Self {
        Self {
            size: crate::core::Size::new(w, h),
            child_y,
            scroll_y,
            children: RefCell::new(children),
        }
    }
}

impl WidgetComponent for ScrollClipContainer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(
            WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER | WidgetCapabilities::EVENT,
        )
    }
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
        std::mem::take(&mut *self.children.borrow_mut())
    }
    crate::wc_upcast!(ScrollClipContainer; WidgetLayout);
    crate::wc_upcast!(ScrollClipContainer; WidgetRender);
    crate::wc_upcast!(ScrollClipContainer; EventHandler);
}

impl WidgetLayout for ScrollClipContainer {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.size)
    }

    fn measure_children(
        &self,
        _frame: Rect,
        children: &[ComponentId],
        tree: &WidgetTree,
    ) -> Vec<crate::ui::LayoutChild> {
        children
            .iter()
            .copied()
            .map(|id| child_from_tree_with_constraints(id, tree, Constraints::unconstrained()))
            .collect()
    }

    fn layout_children(
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        _tree: &WidgetTree,
    ) -> Vec<(ComponentId, Rect)> {
        children
            .iter()
            .map(|child| {
                (
                    child.id,
                    Rect::new(
                        frame.x,
                        frame.y + self.child_y,
                        child.measured_size.w,
                        child.measured_size.h,
                    ),
                )
            })
            .collect()
    }
}

impl WidgetRender for ScrollClipContainer {
    fn render(&self, _: Rect, _: &mut crate::draw::painting::PaintContext, _: &WidgetTree) {}

    fn uses_palette(&self) -> bool {
        false
    }

    fn children_clip(&self, frame: Rect) -> Option<Rect> {
        Some(frame)
    }
}

impl EventHandler for ScrollClipContainer {
    fn viewport_scroll_offset(&self) -> Option<(f32, f32)> {
        Some((0.0, self.scroll_y))
    }
}

struct LifecycleProbe {
    size: crate::core::Size,
    events: Rc<RefCell<Vec<&'static str>>>,
    uses_palette: bool,
}

impl LifecycleProbe {
    fn new(w: f32, h: f32, events: Rc<RefCell<Vec<&'static str>>>) -> Self {
        Self {
            size: crate::core::Size::new(w, h),
            events,
            uses_palette: true,
        }
    }

    fn static_colors(mut self) -> Self {
        self.uses_palette = false;
        self
    }
}

impl WidgetComponent for LifecycleProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(
            WidgetCapabilities::LAYOUT
                | WidgetCapabilities::RENDER
                | WidgetCapabilities::EVENT
                | WidgetCapabilities::LIFECYCLE,
        )
    }
    crate::wc_upcast!(LifecycleProbe; WidgetLayout);
    crate::wc_upcast!(LifecycleProbe; WidgetRender);
    crate::wc_upcast!(LifecycleProbe; EventHandler);
    crate::wc_upcast!(LifecycleProbe; WidgetLifecycle);
}

impl WidgetLayout for LifecycleProbe {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.size)
    }
}

impl WidgetRender for LifecycleProbe {
    fn render(&self, _: Rect, _: &mut crate::draw::painting::PaintContext, _: &WidgetTree) {}

    fn uses_palette(&self) -> bool {
        self.uses_palette
    }
}

impl EventHandler for LifecycleProbe {
    fn on_event(&mut self, _: &SystemEvent) -> EventResult {
        EventResult::Handled
    }
}

impl WidgetLifecycle for LifecycleProbe {
    fn on_init(&mut self) {
        self.events.borrow_mut().push("init");
    }

    fn on_attach(&mut self) {
        self.events.borrow_mut().push("attach");
    }

    fn on_mount(&mut self) {
        self.events.borrow_mut().push("mount");
    }

    fn on_active(&mut self) {
        self.events.borrow_mut().push("active");
    }

    fn on_inactive(&mut self) {
        self.events.borrow_mut().push("inactive");
    }

    fn on_theme_changed(&mut self) {
        self.events.borrow_mut().push("theme");
    }

    fn on_unmount(&mut self) {
        self.events.borrow_mut().push("unmount");
    }

    fn on_detach(&mut self) {
        self.events.borrow_mut().push("detach");
    }

    fn on_destroy(&mut self) {
        self.events.borrow_mut().push("destroy");
    }
}

#[test]
fn tree_set_root_returns_valid_id() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
    assert!(tree.get(id).is_some());
    assert_eq!(tree.root().unwrap().id(), id);
}

#[test]
fn component_ids_are_unique_across_widget_trees() {
    let mut first = WidgetTree::new();
    let mut second = WidgetTree::new();
    let first_root = first.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
    let second_root = second.set_root(Box::new(SpyWidget::new(100.0, 50.0)));

    assert_eq!(first_root.slot(), second_root.slot());
    assert_eq!(first_root.generation(), second_root.generation());
    assert_ne!(first_root, second_root);
    assert!(first.get(second_root).is_none());
    assert!(second.get(first_root).is_none());
}

#[test]
fn set_root_uses_named_bootstrap_constraints() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();

    tree.set_root(Box::new(ConstraintProbeWidget::new(
        Size::new(100.0, 50.0),
        seen.clone(),
    )));

    assert_eq!(
        seen.borrow().as_slice(),
        &[WidgetTree::root_bootstrap_constraints()]
    );
}

#[test]
fn root_bootstrap_constraints_are_finite() {
    let constraints = WidgetTree::root_bootstrap_constraints();

    assert_eq!(constraints.min, Size::zero());
    assert_eq!(constraints.max, WidgetTree::ROOT_BOOTSTRAP_SIZE);
    assert_ne!(constraints.max, Size::infinite());
}

#[test]
fn set_root_clamps_oversized_root_to_bootstrap_constraints() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(SpyWidget::new(1200.0, 900.0)));

    assert_eq!(
        tree.get(root).unwrap().frame(),
        Rect::new(
            0.0,
            0.0,
            WidgetTree::ROOT_BOOTSTRAP_SIZE.w,
            WidgetTree::ROOT_BOOTSTRAP_SIZE.h
        )
    );
}

#[test]
fn layout_bootstrap_uses_named_bootstrap_constraints_for_missing_root_frame() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(ConstraintProbeWidget::new(
        Size::new(100.0, 50.0),
        seen.clone(),
    )));
    tree.get_mut(root).unwrap().set_frame(Rect::zero());
    seen.borrow_mut().clear();

    tree.layout();

    assert_eq!(
        seen.borrow().as_slice(),
        &[WidgetTree::root_bootstrap_constraints()]
    );
    assert_eq!(
        tree.get(root).unwrap().frame(),
        Rect::new(0.0, 0.0, 100.0, 50.0)
    );
}

#[test]
fn boxed_widget_measure_uses_constraints() {
    let boxed = BoxedWidget::new(Box::new(MeasureOnlyWidget));

    assert_eq!(
        boxed.measure(Constraints::loose(Size::new(100.0, 50.0))),
        Size::new(100.0, 50.0)
    );
    assert_eq!(
        boxed.measure(Constraints::unconstrained()),
        Size::new(120.0, 80.0)
    );
}

#[test]
fn boxed_widget_layout_defaults_use_documented_flex_shrink() {
    let boxed = BoxedWidget::new(Box::new(SpyWidget::new(120.0, 80.0)));

    assert_eq!(boxed.flex_grow(), 0.0);
    assert_eq!(boxed.flex_shrink(), 1.0);
}

#[test]
fn boxed_widget_replace_component_transfers_lifecycle_state() {
    let old_events = Rc::new(RefCell::new(Vec::new()));
    let new_events = Rc::new(RefCell::new(Vec::new()));
    let mut boxed = BoxedWidget::new(Box::new(LifecycleProbe::new(
        10.0,
        10.0,
        old_events.clone(),
    )));
    boxed.set_attached(true);
    boxed.on_attach();
    boxed.set_mounted(true);
    boxed.on_mount();
    boxed.set_active(true);
    boxed.on_active();

    boxed.replace_component(Box::new(LifecycleProbe::new(
        20.0,
        20.0,
        new_events.clone(),
    )));

    assert_eq!(
        old_events.borrow().clone(),
        vec!["init", "attach", "mount", "active", "inactive", "unmount", "detach", "destroy"]
    );
    assert_eq!(
        new_events.borrow().clone(),
        vec!["init", "attach", "mount", "active"]
    );
    assert!(boxed.attached());
    assert!(boxed.mounted());
    assert!(boxed.active());
    assert!(!boxed.destroyed());
}

#[test]
fn child_from_tree_reads_component_layout_margin() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let margin = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
    let child = tree.add_child(root, Box::new(Container::new().margin(margin)));

    let layout_child = child_from_tree(child, &tree);

    assert_eq!(layout_child.margin, margin);
}

#[test]
fn child_from_tree_reads_common_style_component_margins() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let margin = EdgeInsets::new(4.0, 5.0, 6.0, 7.0);
    let style = Style::default().with_margin(margin);
    let button = tree.add_child(root, Box::new(Button::new("Ok").style(style.clone())));
    let label = tree.add_child(root, Box::new(Label::new("Name").style(style.clone())));
    let mut grid_widget = Grid::new();
    grid_widget.apply_style(&style);
    let grid = tree.add_child(root, Box::new(grid_widget));

    assert_eq!(child_from_tree(button, &tree).margin, margin);
    assert_eq!(child_from_tree(label, &tree).margin, margin);
    assert_eq!(child_from_tree(grid, &tree).margin, margin);
}

#[test]
fn child_from_tree_reads_common_style_align_self() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let mut style = Style::default();
    style.align_self = Some(crate::ui::layout::AlignItems::Start);
    let container = tree.add_child(root, Box::new(Container::new().style(style.clone())));
    let button = tree.add_child(root, Box::new(Button::new("Ok").style(style.clone())));
    let label = tree.add_child(root, Box::new(Label::new("Name").style(style)));

    assert_eq!(
        child_from_tree(container, &tree).align_self,
        Some(crate::ui::layout::AlignItems::Start)
    );
    assert_eq!(
        child_from_tree(button, &tree).align_self,
        Some(crate::ui::layout::AlignItems::Start)
    );
    assert_eq!(
        child_from_tree(label, &tree).align_self,
        Some(crate::ui::layout::AlignItems::Start)
    );
}

#[test]
fn child_from_tree_reads_common_style_grid_placement() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let style = Style::default()
        .with_grid_cell(2)
        .with_grid_column_span(3)
        .with_grid_row_span(2);
    let container = tree.add_child(root, Box::new(Container::new().style(style.clone())));
    let button = tree.add_child(root, Box::new(Button::new("Ok").style(style.clone())));
    let label = tree.add_child(root, Box::new(Label::new("Name").style(style.clone())));
    let mut grid_widget = Grid::new();
    grid_widget.apply_style(&style);
    let grid = tree.add_child(root, Box::new(grid_widget));

    for child in [container, button, label, grid] {
        let layout_child = child_from_tree(child, &tree);
        assert_eq!(layout_child.grid_cell, Some(2));
        assert_eq!(layout_child.grid_column_span, 3);
        assert_eq!(layout_child.grid_row_span, 2);
    }
}

#[test]
fn child_from_tree_with_constraints_clamps_measured_size() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(120.0, 80.0)));

    let layout_child =
        child_from_tree_with_constraints(child, &tree, Constraints::loose(Size::new(40.0, 24.0)));

    assert_eq!(layout_child.measured_size, Size::new(40.0, 24.0));
}

#[test]
fn child_from_tree_with_constraints_preserves_zero_height_measurement() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(120.0, 0.0)));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 120.0, 80.0));

    let layout_child =
        child_from_tree_with_constraints(child, &tree, Constraints::loose(Size::new(200.0, 100.0)));

    assert_eq!(layout_child.measured_size, Size::new(120.0, 0.0));
}

#[test]
fn child_from_tree_falls_back_to_parent_frame_constraints() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(40.0, 24.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(120.0, 80.0)));

    let layout_child = child_from_tree(child, &tree);

    assert_eq!(layout_child.measured_size, Size::new(40.0, 24.0));
}

#[test]
fn child_from_tree_reads_documented_flex_shrink_default() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(120.0, 80.0)));

    assert_eq!(child_from_tree(child, &tree).flex_grow, 0.0);
    assert_eq!(child_from_tree(child, &tree).flex_shrink, 1.0);
}

#[test]
fn container_layout_measures_children_with_content_constraints() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(40.0, 24.0)));
    let child = tree.add_child(root, Box::new(SpyWidget::new(120.0, 80.0)));

    tree.layout();

    assert_eq!(
        tree.get(child).unwrap().frame(),
        // 与 Space 对齐：Container 子项 flex_shrink=0，定高槽位不再压扁内容
        Rect::new(0.0, 0.0, 40.0, 80.0)
    );
}

#[test]
fn column_container_intrinsic_height_from_children() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().w(200.0)));
    let first = tree.add_child(root, Box::new(SpyWidget::new(80.0, 20.0)));
    let second = tree.add_child(root, Box::new(SpyWidget::new(80.0, 30.0)));

    tree.layout();

    assert!(
        tree.get(root).unwrap().frame().h >= 50.0,
        "column should grow to sum of child heights, got {}",
        tree.get(root).unwrap().frame().h
    );
    assert_eq!(tree.get(first).unwrap().frame().h, 20.0);
    assert_eq!(tree.get(second).unwrap().frame().h, 30.0);
}

#[test]
fn row_container_intrinsic_height_from_children() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Container::new()
            .dir(crate::ui::layout::FlexDirection::Row)
            .w(200.0),
    ));
    let short = tree.add_child(root, Box::new(SpyWidget::new(40.0, 16.0)));
    let tall = tree.add_child(root, Box::new(SpyWidget::new(40.0, 28.0)));

    tree.layout();

    assert!(
        tree.get(root).unwrap().frame().h >= 28.0,
        "row should grow to tallest child"
    );
    assert_eq!(tree.get(short).unwrap().frame().h, 28.0);
    assert_eq!(tree.get(tall).unwrap().frame().h, 28.0);
}

#[test]
fn flex_layout_uses_child_align_self_over_container_align() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(100.0, 50.0)));
    let mut child_style = Style::default();
    child_style.align_self = Some(crate::ui::layout::AlignItems::Start);
    let child = tree.add_child(
        root,
        Box::new(Container::new().style(child_style).size(20.0, 10.0)),
    );

    tree.layout();

    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(0.0, 0.0, 20.0, 10.0)
    );
}

#[test]
fn overflow_layout_uses_child_align_self_over_container_align() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Container::new().size(100.0, 50.0).overflow_content(),
    ));
    let mut child_style = Style::default();
    child_style.align_self = Some(crate::ui::layout::AlignItems::Start);
    let child = tree.add_child(
        root,
        Box::new(Container::new().style(child_style).size(20.0, 10.0)),
    );

    tree.layout();

    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(0.0, 0.0, 20.0, 10.0)
    );
}

#[test]
fn grid_layout_measures_children_with_content_constraints() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Grid::new()
            .columns(vec![crate::ui::layout::GridTrack::Px(100.0)])
            .rows(vec![crate::ui::layout::GridTrack::Px(60.0)])
            .size(40.0, 24.0)
            .align(crate::ui::layout::AlignItems::Start),
    ));
    let child = tree.add_child(root, Box::new(SpyWidget::new(120.0, 80.0)));

    tree.layout();

    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(0.0, 0.0, 40.0, 24.0)
    );
}

#[test]
fn grid_layout_uses_style_box_model_content_rect() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Grid::new()
            .columns(vec![crate::ui::layout::GridTrack::Fr(1.0)])
            .rows(vec![crate::ui::layout::GridTrack::Fr(1.0)])
            .pad(EdgeInsets::uniform(4.0))
            .border(Color::black(), 2.0)
            .size(120.0, 80.0)
            .justify(crate::ui::layout::JustifyContent::Stretch),
    ));
    let child = tree.add_child(root, Box::new(SpyWidget::new(120.0, 80.0)));

    tree.layout();

    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(6.0, 6.0, 108.0, 68.0)
    );
}

#[test]
fn layout_margin_offsets_child_frame() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(200.0, 100.0)));
    let child = tree.add_child(
        root,
        Box::new(
            Container::new()
                .size(20.0, 10.0)
                .margin(EdgeInsets::new(4.0, 3.0, 8.0, 6.0)),
        ),
    );

    tree.layout();

    assert_eq!(tree.get(child).unwrap().frame().x, 4.0);
    assert_eq!(tree.get(child).unwrap().frame().y, 3.0);
}

#[test]
fn layout_margin_occupies_space_between_siblings() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Container::new()
            .size(200.0, 100.0)
            .dir(crate::ui::layout::FlexDirection::Row),
    ));
    let first = tree.add_child(
        root,
        Box::new(
            Container::new()
                .size(20.0, 10.0)
                .margin(EdgeInsets::new(0.0, 0.0, 7.0, 0.0)),
        ),
    );
    let second = tree.add_child(root, Box::new(Container::new().size(20.0, 10.0)));

    tree.layout();

    assert_eq!(tree.get(first).unwrap().frame().x, 0.0);
    assert_eq!(tree.get(second).unwrap().frame().x, 27.0);
}

#[test]
fn layout_margin_occupies_space_in_column_direction() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Container::new()
            .size(200.0, 100.0)
            .direction(crate::ui::style::FlexDirection::Column),
    ));
    let first = tree.add_child(
        root,
        Box::new(
            Container::new()
                .size(20.0, 10.0)
                .margin(EdgeInsets::new(0.0, 6.0, 0.0, 0.0)),
        ),
    );
    let second = tree.add_child(root, Box::new(Container::new().size(20.0, 10.0)));

    tree.layout();

    assert_eq!(tree.get(first).unwrap().frame().y, 6.0);
    assert_eq!(tree.get(second).unwrap().frame().y, 16.0);
}

#[test]
fn layout_margin_reduces_stretched_cross_axis_size() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Container::new()
            .size(200.0, 100.0)
            .dir(crate::ui::layout::FlexDirection::Row),
    ));
    let child = tree.add_child(
        root,
        Box::new(
            Container::new()
                .size(20.0, 10.0)
                .margin(EdgeInsets::new(0.0, 3.0, 0.0, 7.0)),
        ),
    );

    tree.layout();

    let frame = tree.get(child).unwrap().frame();
    assert_eq!(frame.y, 3.0);
    assert_eq!(frame.h, 90.0);
}

#[test]
fn overflow_layout_margin_occupies_space_between_siblings() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Container::new()
            .size(200.0, 100.0)
            .dir(crate::ui::layout::FlexDirection::Row)
            .overflow_content(),
    ));
    tree.add_child(
        root,
        Box::new(
            Container::new()
                .size(20.0, 10.0)
                .margin(EdgeInsets::new(0.0, 0.0, 7.0, 0.0)),
        ),
    );
    let second = tree.add_child(root, Box::new(Container::new().size(20.0, 10.0)));

    tree.layout();

    assert_eq!(tree.get(second).unwrap().frame().x, 27.0);
}

#[test]
fn wrapped_layout_uses_margin_for_line_breaks() {
    let mut tree = WidgetTree::new();
    let root_style = Style::container()
        .with_wrap(true)
        .with_align(crate::ui::style::AlignItems::Start)
        .with_width(50.0)
        .with_height(100.0);
    let root = tree.set_root(Box::new(Container::new().style(root_style)));
    tree.add_child(
        root,
        Box::new(
            Container::new()
                .size(30.0, 10.0)
                .margin(EdgeInsets::new(0.0, 0.0, 25.0, 0.0)),
        ),
    );
    let second = tree.add_child(root, Box::new(Container::new().size(10.0, 10.0)));

    tree.layout();

    assert_eq!(tree.get(second).unwrap().frame().x, 0.0);
    assert_eq!(tree.get(second).unwrap().frame().y, 10.0);
}

#[test]
fn grid_layout_applies_child_margin_inside_cell() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Grid::new()
            .columns(vec![crate::ui::layout::GridTrack::Px(50.0)])
            .rows(vec![crate::ui::layout::GridTrack::Px(40.0)])
            .size(50.0, 40.0)
            .justify(crate::ui::layout::JustifyContent::Stretch),
    ));
    let child = tree.add_child(
        root,
        Box::new(
            Container::new()
                .size(10.0, 10.0)
                .margin(EdgeInsets::new(4.0, 3.0, 6.0, 7.0)),
        ),
    );

    tree.layout();

    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(4.0, 3.0, 40.0, 30.0)
    );
}

#[test]
fn grid_layout_honors_explicit_cell_zero() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Grid::new()
            .columns(vec![
                crate::ui::layout::GridTrack::Px(50.0),
                crate::ui::layout::GridTrack::Px(50.0),
            ])
            .rows(vec![crate::ui::layout::GridTrack::Px(40.0)])
            .size(100.0, 40.0)
            .align(crate::ui::layout::AlignItems::Start),
    ));
    let automatic = tree.add_child(root, Box::new(Container::new().size(10.0, 10.0)));
    let explicit = tree.add_child(
        root,
        Box::new(
            Container::new()
                .style(Style::default().with_grid_cell(0))
                .size(10.0, 10.0),
        ),
    );

    tree.layout();

    assert_eq!(
        tree.get(explicit).unwrap().frame(),
        Rect::new(0.0, 0.0, 10.0, 10.0)
    );
    assert_eq!(
        tree.get(automatic).unwrap().frame(),
        Rect::new(50.0, 0.0, 10.0, 10.0)
    );
}

#[test]
fn grid_layout_uses_child_grid_span() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Grid::new()
            .columns(vec![
                crate::ui::layout::GridTrack::Px(50.0),
                crate::ui::layout::GridTrack::Px(50.0),
            ])
            .rows(vec![crate::ui::layout::GridTrack::Px(40.0)])
            .size(100.0, 40.0)
            .justify(crate::ui::layout::JustifyContent::Stretch),
    ));
    let child = tree.add_child(
        root,
        Box::new(
            Container::new()
                .style(Style::default().with_grid_cell(0).with_grid_column_span(2))
                .size(10.0, 10.0),
        ),
    );

    tree.layout();

    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(0.0, 0.0, 100.0, 40.0)
    );
}

#[test]
fn tree_add_child_links_parent() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let cid = tree.add_child(root, Box::new(SpyWidget::new(80.0, 40.0)));
    assert!(tree.get(cid).is_some());
    assert_eq!(tree.get(root).unwrap().children(), &[cid]);
    assert_eq!(tree.get(cid).unwrap().parent(), Some(root));
}

#[test]
fn tree_traverse_preorder() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let a = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
    let b = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
    let c = tree.add_child(a, Box::new(SpyWidget::new(25.0, 15.0)));
    let first = tree.traverse();
    assert_eq!(&*first, &[root, a, c, b]);
    let cached_address = first.as_ptr();
    drop(first);
    assert_eq!(tree.traverse().as_ptr(), cached_address);
}

#[test]
fn layout_traverse_keeps_ancestor_path_and_dirty_subtree() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let branch = tree.add_child(
        root,
        Box::new(PassThroughContainer::new(80.0, 40.0, vec![])),
    );
    let leaf = tree.add_child(branch, Box::new(SpyWidget::new(30.0, 20.0)));
    let sibling = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
    tree.layout();
    tree.reset_invalidation();

    tree.push_layout_invalidation(branch);

    assert_eq!(tree.layout_traverse(), vec![root, branch, leaf]);
    assert!(!tree.layout_traverse().contains(&sibling));
}

#[test]
fn tree_remove_cascades_to_children() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let a = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
    let b = tree.add_child(a, Box::new(SpyWidget::new(25.0, 15.0)));
    tree.remove(a);
    assert!(tree.get(a).is_none());
    assert!(tree.get(b).is_none());
    assert_eq!(tree.get(root).unwrap().children().len(), 0);
}

#[test]
fn removed_widget_id_does_not_match_reused_slot() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let first = tree.add_child(root, Box::new(Label::new("first")));

    tree.remove(first);
    let second = tree.add_child(root, Box::new(Label::new("second")));

    assert_eq!(first.slot(), second.slot());
    assert_ne!(first.generation(), second.generation());
    assert!(tree.get(first).is_none());
    assert!(tree.get(second).is_some());
}

#[test]
fn replacing_root_invalidates_previous_root_id() {
    let mut tree = WidgetTree::new();
    let first = tree.set_root(Box::new(Label::new("first")));

    let second = tree.set_root(Box::new(Label::new("second")));

    assert_eq!(first.slot(), second.slot());
    assert_ne!(first.generation(), second.generation());
    assert!(tree.get(first).is_none());
    assert!(tree.get(second).is_some());
}

#[test]
fn lifecycle_active_inactive_follow_clip_intersection() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(ClipContainer::new(
        100.0,
        100.0,
        120.0,
        vec![Box::new(LifecycleProbe::new(20.0, 20.0, events.clone()))],
    )));

    tree.layout();
    assert_eq!(events.borrow().clone(), vec!["init", "attach", "mount"]);

    tree.find_by_type_and_modify::<ClipContainer>(|container| {
        container.child_y = 10.0;
    });
    tree.push_layout_invalidation(root);
    tree.layout();
    assert_eq!(
        events.borrow().clone(),
        vec!["init", "attach", "mount", "active"]
    );

    tree.find_by_type_and_modify::<ClipContainer>(|container| {
        container.child_y = 150.0;
    });
    tree.push_layout_invalidation(root);
    tree.layout();
    assert_eq!(
        events.borrow().clone(),
        vec!["init", "attach", "mount", "active", "inactive"]
    );
}

#[test]
fn lifecycle_active_follows_scroll_viewport_offset() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(ScrollClipContainer::new(
        100.0,
        100.0,
        150.0,
        0.0,
        vec![Box::new(LifecycleProbe::new(20.0, 20.0, events.clone()))],
    )));

    tree.layout();
    assert_eq!(events.borrow().clone(), vec!["init", "attach", "mount"]);

    tree.find_by_type_and_modify::<ScrollClipContainer>(|container| {
        container.scroll_y = 100.0;
    });
    tree.push_layout_invalidation(root);
    tree.layout();
    assert_eq!(
        events.borrow().clone(),
        vec!["init", "attach", "mount", "active"]
    );

    tree.find_by_type_and_modify::<ScrollClipContainer>(|container| {
        container.scroll_y = 180.0;
    });
    tree.push_layout_invalidation(root);
    tree.layout();
    assert_eq!(
        events.borrow().clone(),
        vec!["init", "attach", "mount", "active", "inactive"]
    );
}

#[test]
fn lifecycle_focus_keeps_clipped_widget_active() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(ClipContainer::new(
        100.0,
        100.0,
        150.0,
        vec![Box::new(LifecycleProbe::new(20.0, 20.0, events.clone()))],
    )));
    tree.layout();
    assert_eq!(events.borrow().clone(), vec!["init", "attach", "mount"]);

    let probe_id = tree.find_by_type::<LifecycleProbe>().unwrap();
    tree.get_mut(probe_id).unwrap().set_tab_index(1);
    assert_eq!(tree.focus_by_type::<LifecycleProbe>(), Some(probe_id));
    assert_eq!(
        events.borrow().clone(),
        vec!["init", "attach", "mount", "active"]
    );

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(200.0, 200.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(
        events.borrow().clone(),
        vec!["init", "attach", "mount", "active", "inactive"]
    );
}

#[test]
fn lifecycle_unmount_inactivates_active_subtree() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(ClipContainer::new(
        100.0,
        100.0,
        10.0,
        vec![Box::new(LifecycleProbe::new(20.0, 20.0, events.clone()))],
    )));
    tree.layout();

    tree.set_root(Box::new(SpyWidget::new(20.0, 20.0)));

    assert_eq!(
        events.borrow().clone(),
        vec!["init", "attach", "mount", "active", "inactive", "unmount", "detach", "destroy"]
    );
}

#[test]
fn app_state_registers_mounted_components_and_unregisters_removed_components() {
    let app_state = AppState::new();
    let mut tree = WidgetTree::new();
    tree.set_app_state(app_state.clone());
    let root = tree.set_root(Box::new(PassThroughContainer::new(
        100.0,
        50.0,
        vec![Box::new(Label::new("child"))],
    )));

    assert!(app_state.is_empty());

    tree.layout();
    let child = tree.find_by_type::<Label>().unwrap();
    assert_eq!(app_state.len(), 2);
    assert_eq!(
        app_state.get_handle(child).unwrap().text().as_deref(),
        Some("child")
    );

    tree.remove(child);
    assert!(app_state.get_handle(child).is_none());
    assert!(app_state.get_handle(root).is_some());
}

#[test]
fn app_state_lookup_handle_reads_typed_snapshot_for_component_builtin() {
    let app_state = AppState::new();
    let mut tree = WidgetTree::new();
    tree.set_app_state(app_state.clone());
    let root = tree.set_root(Box::new(QRCode::new("uix").size(96.0).error_level(2)));

    tree.layout();

    assert_eq!(
        app_state.get_handle(root).unwrap().snapshot_fields(),
        Some(SnapshotFields::QRCode {
            value: "uix".to_string(),
            size: 96.0,
            error_level: 2,
            module_count: 21,
            encoding_error: None,
        })
    );
}

#[test]
fn app_state_lookup_handle_invalidate_marks_narrow_paint() {
    let app_state = AppState::new();
    let mut tree = WidgetTree::new();
    tree.set_app_state(app_state.clone());
    let root = tree.set_root(Box::new(Label::new("root")));
    tree.layout();
    tree.reset_invalidation();

    app_state.get_handle(root).unwrap().invalidate();

    let invalidation = tree.invalidation.lock().unwrap_or_else(|e| e.into_inner());
    assert!(invalidation.node_needs_paint(root));
    assert!(!invalidation.has_layout());
    drop(invalidation);
    assert!(tree.has_render_work());
    assert!(!tree.dirty_region().full_frame);
}

#[test]
fn app_state_clears_stale_snapshots_when_root_is_replaced() {
    let app_state = AppState::new();
    let mut tree = WidgetTree::new();
    tree.set_app_state(app_state.clone());
    let first = tree.set_root(Box::new(Label::new("first")));
    tree.layout();
    assert_eq!(
        app_state.get_handle(first).unwrap().text().as_deref(),
        Some("first")
    );

    tree.set_root(Box::new(Label::new("second")));
    assert!(app_state.get_handle(first).is_none());

    let second = tree.root_id().unwrap();
    tree.layout();
    assert_eq!(
        app_state.get_handle(second).unwrap().text().as_deref(),
        Some("second")
    );
}

#[test]
fn widget_tree_injects_tree_level_managers() {
    let mut tree = WidgetTree::new();

    tree.managers_mut().text.set_text("tree default");
    tree.managers_mut().focus.set_focusable(true);

    assert_eq!(tree.managers().text.text(), "tree default");
    assert!(tree.managers().focus.is_focusable());
}

#[test]
fn widget_tree_manager_overrides_are_per_component() {
    let mut tree = WidgetTree::new();
    tree.managers_mut().text.set_text("tree default");
    tree.managers_mut().style.set_bg(Color::from_rgb(1, 2, 3));
    let root = tree.set_root(Box::new(Label::new("root")));
    let child = tree.add_child(root, Box::new(Label::new("child")));

    let mut text = TextManager::new();
    text.set_text("root override");
    tree.managers_mut().override_text(root, text);
    let mut style = StyleManager::new();
    style.set_bg(Color::from_rgb(4, 5, 6));
    tree.managers_mut().override_style(root, style);

    assert_eq!(tree.managers().text_for(root).text(), "root override");
    assert_eq!(tree.managers().text_for(child).text(), "tree default");
    assert_eq!(
        tree.managers().style_for(root).bg(),
        Some(Color::from_rgb(4, 5, 6))
    );
    assert_eq!(
        tree.managers().style_for(child).bg(),
        Some(Color::from_rgb(1, 2, 3))
    );
}

#[test]
fn widget_tree_remove_overrides_restores_style_default() {
    let mut tree = WidgetTree::new();
    tree.managers_mut().style.set_bg(Color::from_rgb(1, 2, 3));
    let root = tree.set_root(Box::new(Label::new("root")));
    let child = tree.add_child(root, Box::new(Label::new("child")));

    let mut root_style = StyleManager::new();
    root_style.set_bg(Color::from_rgb(4, 5, 6));
    tree.managers_mut().override_style(root, root_style);

    let mut child_style = StyleManager::new();
    child_style.set_bg(Color::from_rgb(7, 8, 9));
    tree.managers_mut().override_style(child, child_style);

    tree.managers_mut().remove_overrides(root);

    assert_eq!(
        tree.managers().style_for(root).bg(),
        Some(Color::from_rgb(1, 2, 3))
    );
    assert_eq!(
        tree.managers().style_for(child).bg(),
        Some(Color::from_rgb(7, 8, 9))
    );
}

#[test]
fn widget_tree_clear_overrides_restores_all_style_defaults() {
    let mut tree = WidgetTree::new();
    tree.managers_mut().style.set_bg(Color::from_rgb(1, 2, 3));
    let root = tree.set_root(Box::new(Label::new("root")));
    let child = tree.add_child(root, Box::new(Label::new("child")));

    let mut root_style = StyleManager::new();
    root_style.set_bg(Color::from_rgb(4, 5, 6));
    tree.managers_mut().override_style(root, root_style);

    let mut child_style = StyleManager::new();
    child_style.set_bg(Color::from_rgb(7, 8, 9));
    tree.managers_mut().override_style(child, child_style);

    tree.managers_mut().clear_overrides();

    assert_eq!(
        tree.managers().style_for(root).bg(),
        Some(Color::from_rgb(1, 2, 3))
    );
    assert_eq!(
        tree.managers().style_for(child).bg(),
        Some(Color::from_rgb(1, 2, 3))
    );
}

#[test]
fn widget_tree_removes_manager_overrides_with_removed_nodes() {
    let mut tree = WidgetTree::new();
    tree.managers_mut().text.set_text("tree default");
    let root = tree.set_root(Box::new(PassThroughContainer::new(100.0, 50.0, vec![])));
    let child = tree.add_child(root, Box::new(Label::new("child")));

    let mut text = TextManager::new();
    text.set_text("child override");
    tree.managers_mut().override_text(child, text);
    assert_eq!(tree.managers().text_for(child).text(), "child override");

    tree.remove(child);

    assert_eq!(tree.managers().text_for(child).text(), "tree default");
}

#[test]
fn widget_tree_clears_manager_overrides_when_root_is_replaced() {
    let mut tree = WidgetTree::new();
    tree.managers_mut().text.set_text("tree default");
    let first = tree.set_root(Box::new(Label::new("first")));

    let mut text = TextManager::new();
    text.set_text("stale override");
    tree.managers_mut().override_text(first, text);
    assert_eq!(tree.managers().text_for(first).text(), "stale override");

    let second = tree.set_root(Box::new(Label::new("second")));

    assert_eq!(second.slot(), first.slot());
    assert_ne!(second.generation(), first.generation());
    assert_eq!(tree.managers().text_for(first).text(), "tree default");
    assert_eq!(tree.managers().text_for(second).text(), "tree default");
}

#[test]
fn focus_manager_tracks_direct_tree_tab_index_defaults() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let second = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)));
    let first = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));

    assert_eq!(tree.managers().focus.focusable_order(), vec![first, second]);
    assert_eq!(tree.collect_focusable(), vec![first, second]);
}

#[test]
fn focus_manager_tracks_pointer_focus_changes() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 100.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 20.0, 20.0));

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 10.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(tree.managers().focus.focused_component(), Some(child));

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(300.0, 300.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(tree.managers().focus.focused_component(), None);
}

#[test]
fn focus_manager_drives_tab_navigation() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let first = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    let second = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(first));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(second));
}

#[test]
fn focus_manager_shift_tab_starts_at_last_candidate() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let _first = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    let last = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::SHIFT,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(last));
}

#[test]
fn tab_navigation_skips_focusable_descendants_of_hidden_ancestors() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let hidden_parent = tree.add_child(
        root,
        Box::new(PassThroughContainer::new(100.0, 50.0, vec![])),
    );
    let hidden_child = tree.add_child(
        hidden_parent,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)),
    );
    let visible_child =
        tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)));
    tree.get_mut(hidden_parent).unwrap().set_visible(false);

    assert_eq!(tree.collect_focusable(), vec![visible_child]);
    assert!(!tree.collect_focusable().contains(&hidden_child));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.managers().focus.focused_component(),
        Some(visible_child)
    );
}

#[test]
fn hiding_focused_subtree_dispatches_focus_out_and_blocks_stale_key_routing() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let parent = tree.add_child(
        root,
        Box::new(PassThroughContainer::new(100.0, 50.0, vec![])),
    );
    let child = tree.add_child(
        parent,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)),
    );
    tree.set_focus(Some(child));
    tree.get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow_mut()
        .clear();

    tree.set_visible(parent, false);

    assert!(tree.managers().focus.focused_component().is_none());
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    let events = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], SystemEvent::FocusOut));
}

#[test]
fn dispatch_clears_focus_hidden_through_low_level_visibility_mutation() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    tree.set_focus(Some(child));
    tree.get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow_mut()
        .clear();
    tree.get_mut(child).unwrap().set_visible(false);

    assert_eq!(
        tree.dispatch_event(&SystemEvent::TextInput {
            text: "blocked".to_string(),
        }),
        EventResult::NotHandled
    );
    assert!(tree.managers().focus.focused_component().is_none());
    let events = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], SystemEvent::FocusOut));
}

#[test]
fn focus_by_type_uses_the_shared_focus_transition_path() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let first = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    let second = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)));
    tree.set_focus(Some(second));
    for id in [first, second] {
        tree.get(id)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<SpyWidget>()
            .unwrap()
            .events
            .borrow_mut()
            .clear();
    }

    assert_eq!(tree.focus_by_type::<SpyWidget>(), Some(first));

    let first_events = tree
        .get(first)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow();
    let second_events = tree
        .get(second)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow();
    assert_eq!(first_events.len(), 1);
    assert!(matches!(first_events[0], SystemEvent::FocusIn));
    assert_eq!(second_events.len(), 1);
    assert!(matches!(second_events[0], SystemEvent::FocusOut));
}

#[test]
fn focus_trap_tab_navigation_stays_inside_top_overlay_owner_subtree() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(300.0, 200.0, vec![])));
    let outside = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    let modal = tree.add_child(
        root,
        Box::new(Modal::new("Dialog").visible(true).overlay(true)),
    );
    let first = tree.add_child(
        modal,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)),
    );
    let second = tree.add_child(
        modal,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(3)),
    );
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    tree.layout();

    tree.managers_mut()
        .focus
        .set_focused_component(Some(outside));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(first));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(second));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(first));
}

#[test]
fn focus_trap_shift_tab_wraps_inside_top_overlay_owner_subtree() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(300.0, 200.0, vec![])));
    let _outside = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    let modal = tree.add_child(
        root,
        Box::new(Modal::new("Dialog").visible(true).overlay(true)),
    );
    let first = tree.add_child(
        modal,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)),
    );
    let second = tree.add_child(
        modal,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(3)),
    );
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    tree.layout();
    tree.managers_mut().focus.set_focused_component(Some(first));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::SHIFT,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(second));
}

#[test]
fn focus_trap_without_focusable_children_does_not_escape_to_main_tree() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(300.0, 200.0, vec![])));
    let outside = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    let owner = tree.add_child(
        root,
        Box::new(PassThroughContainer::new(100.0, 80.0, vec![])),
    );
    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(owner, OverlayKind::Modal)
            .bounds(Rect::new(50.0, 30.0, 120.0, 100.0))
            .z_index(1000),
    );
    tree.managers_mut()
        .focus
        .set_focused_component(Some(outside));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(outside));
}

#[test]
fn drawer_focus_trap_tab_navigation_stays_inside_drawer_subtree() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(300.0, 200.0, vec![])));
    let outside = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    let drawer = tree.add_child(root, Box::new(Drawer::new("Drawer").show()));
    let first = tree.add_child(
        drawer,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)),
    );
    let second = tree.add_child(
        drawer,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(3)),
    );
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    tree.layout();
    tree.managers_mut()
        .focus
        .set_focused_component(Some(outside));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(first));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(second));
}

#[test]
fn focus_manager_clears_removed_focused_component() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    tree.managers_mut().focus.set_focused_component(Some(child));

    tree.remove(child);

    assert_eq!(tree.managers().focus.focused_component(), None);
}

#[test]
fn focus_manager_is_single_source_for_key_dispatch() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    tree.managers_mut().focus.set_focused_component(Some(child));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );

    let events = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow();
    assert!(events.iter().any(|event| matches!(
        event,
        SystemEvent::KeyDown {
            key: KeyCode::Enter,
            ..
        }
    )));
}

#[test]
fn interaction_manager_tracks_hover_changes() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 100.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 20.0, 20.0));

    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(10.0, 10.0),
        mods: KeyMod::NONE,
    });

    assert_eq!(tree.managers().interaction.hovered_component(), Some(child));

    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(150.0, 50.0),
        mods: KeyMod::NONE,
    });

    assert_eq!(tree.managers().interaction.hovered_component(), Some(root));
}

#[test]
fn hover_over_label_does_not_invalidate_paint() {
    // 侧栏 Label 无 hover 视觉态；enter/leave NotHandled 时不得窄标脏，
    // 否则局部清屏会挖掉父 Container 背景，表现为悬停时文字/图标消失。
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(220.0, 400.0)));
    let label = tree.add_child(root, Box::new(Label::new("导航")));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 220.0, 400.0));
    tree.get_mut(label)
        .unwrap()
        .set_frame(Rect::new(12.0, 80.0, 180.0, 36.0));
    tree.reset_invalidation();

    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(40.0, 90.0),
        mods: KeyMod::NONE,
    });

    assert_eq!(tree.managers().interaction.hovered_component(), Some(label));
    assert!(
        !tree.has_render_work(),
        "Label hover must not enqueue paint invalidation"
    );
}

#[test]
fn hover_over_button_still_invalidates_paint() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(220.0, 100.0)));
    let button = tree.add_child(root, Box::new(Button::new("OK")));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 220.0, 100.0));
    tree.get_mut(button)
        .unwrap()
        .set_frame(Rect::new(10.0, 10.0, 80.0, 32.0));
    tree.reset_invalidation();

    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(20.0, 20.0),
        mods: KeyMod::NONE,
    });

    assert_eq!(
        tree.managers().interaction.hovered_component(),
        Some(button)
    );
    assert!(
        tree.has_render_work(),
        "Button hover must invalidate paint for hover style"
    );
}

#[test]
fn interaction_manager_tracks_pressed_changes() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 100.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 20.0, 20.0));

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 10.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(tree.managers().interaction.pressed_component(), Some(child));

    tree.dispatch_event(&SystemEvent::PointerUp {
        pos: Point::new(10.0, 10.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(tree.managers().interaction.pressed_component(), None);
}

#[test]
fn interaction_manager_hover_drives_timer_target() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0)));

    tree.managers_mut()
        .interaction
        .set_hovered_component(Some(child));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Timer { id: 7 }),
        EventResult::Handled
    );
    let events = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow();
    assert!(events
        .iter()
        .any(|event| matches!(event, SystemEvent::Timer { id: 7 })));
}

#[test]
fn interaction_manager_clears_removed_component_state() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0)));

    tree.managers_mut()
        .interaction
        .set_hovered_component(Some(child));
    tree.managers_mut()
        .interaction
        .set_pressed_component(Some(child));

    tree.remove(child);

    assert_eq!(tree.managers().interaction.hovered_component(), None);
    assert_eq!(tree.managers().interaction.pressed_component(), None);
}

#[test]
fn drag_manager_tracks_pointer_drag_gesture() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(30.0, 30.0)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 100.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 30.0, 30.0));

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 10.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert!(tree.managers().drag.is_potential());
    assert!(!tree.managers().drag.is_dragging());
    assert_eq!(tree.managers().drag.target(), Some(child));

    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(20.0, 10.0),
        mods: KeyMod::NONE,
    });

    assert!(tree.managers().drag.is_dragging());
    assert!(!tree.managers().drag.is_potential());
    assert_eq!(tree.managers().drag.last_pos(), Point::new(20.0, 10.0));
    assert_eq!(tree.managers().drag.drag_offset(), Point::new(10.0, 0.0));

    tree.dispatch_event(&SystemEvent::PointerUp {
        pos: Point::new(20.0, 10.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert!(!tree.managers().drag.is_dragging());
    assert!(!tree.managers().drag.is_potential());
    assert_eq!(tree.managers().drag.target(), None);

    let events = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow();
    assert!(events
        .iter()
        .any(|event| matches!(event, SystemEvent::DragStart { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, SystemEvent::DragMove { delta, .. } if *delta == Point::new(10.0, 0.0))));
    assert!(events
        .iter()
        .any(|event| matches!(event, SystemEvent::DragEnd { .. })));
}

#[test]
fn drag_manager_target_drives_drag_events() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(30.0, 30.0)));

    tree.managers_mut().drag.begin_gesture(
        Some(child),
        Point::new(10.0, 10.0),
        MouseButton::Left,
        KeyMod::NONE,
    );

    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(20.0, 10.0),
        mods: KeyMod::NONE,
    });

    let events = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow();
    assert!(events
        .iter()
        .any(|event| matches!(event, SystemEvent::DragStart { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event, SystemEvent::DragMove { .. })));
}

#[test]
fn drag_manager_clears_removed_component_state() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0)));

    tree.managers_mut().drag.begin_gesture(
        Some(child),
        Point::new(1.0, 1.0),
        MouseButton::Left,
        KeyMod::NONE,
    );
    tree.managers_mut().drag.activate_gesture();

    tree.remove(child);

    assert!(!tree.managers().drag.is_dragging());
    assert!(!tree.managers().drag.is_potential());
    assert_eq!(tree.managers().drag.target(), None);
}

#[test]
fn lifecycle_theme_changed_notifies_and_invalidates_palette_only() {
    let palette_events = Rc::new(RefCell::new(Vec::new()));
    let static_events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(ClipContainer::new(
        100.0,
        100.0,
        10.0,
        vec![
            Box::new(LifecycleProbe::new(20.0, 20.0, palette_events.clone())),
            Box::new(LifecycleProbe::new(20.0, 20.0, static_events.clone()).static_colors()),
        ],
    )));
    tree.layout();
    tree.reset_invalidation();

    let probes = tree.find_all_by_type::<LifecycleProbe>();
    let palette_id = probes
        .iter()
        .find(|(_, probe)| probe.uses_palette)
        .map(|(id, _)| *id)
        .unwrap();
    let static_id = probes
        .iter()
        .find(|(_, probe)| !probe.uses_palette)
        .map(|(id, _)| *id)
        .unwrap();

    tree.dispatch_event(&SystemEvent::ThemeChanged { is_dark: true });

    assert_eq!(
        palette_events.borrow().clone(),
        vec!["init", "attach", "mount", "active", "theme"]
    );
    assert_eq!(
        static_events.borrow().clone(),
        vec!["init", "attach", "mount", "active", "theme"]
    );

    {
        let invalidation = tree.invalidation.lock().unwrap_or_else(|e| e.into_inner());
        assert!(invalidation.node_needs_paint(palette_id));
        assert!(!invalidation.node_needs_paint(static_id));
    }
    assert!(tree.has_render_work());
    assert!(!tree.dirty_region().full_frame);
}

#[test]
fn theme_changed_invalidates_palette_widget_inside_overlay_subtree() {
    let palette_events = Rc::new(RefCell::new(Vec::new()));
    let static_events = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(240.0, 180.0, vec![])));
    let static_id = tree.add_child(
        root_id,
        Box::new(LifecycleProbe::new(20.0, 20.0, static_events.clone()).static_colors()),
    );
    let modal = tree.add_child(
        root_id,
        Box::new(Modal::new("Dialog").visible(true).overlay(true)),
    );
    let palette_id = tree.add_child(
        modal,
        Box::new(LifecycleProbe::new(20.0, 20.0, palette_events.clone())),
    );
    tree.layout();
    tree.reset_invalidation();

    assert!(tree.overlay_stack().top().is_some());

    tree.dispatch_event(&SystemEvent::ThemeChanged { is_dark: true });

    assert_eq!(palette_events.borrow().last(), Some(&"theme"));
    assert_eq!(static_events.borrow().last(), Some(&"theme"));

    let invalidation = tree.invalidation.lock().unwrap_or_else(|e| e.into_inner());
    assert!(invalidation.node_needs_paint(palette_id));
    assert!(!invalidation.node_needs_paint(static_id));
}

#[test]
fn locale_changed_dispatches_to_root_and_invalidates_layout() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
    tree.layout();
    tree.reset_invalidation();

    tree.dispatch_event(&SystemEvent::LocaleChanged {
        locale: "zh-CN".to_string(),
    });

    assert!(matches!(
        tree.get(root)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<SpyWidget>()
            .unwrap()
            .last_event
            .borrow()
            .as_ref(),
        Some(SystemEvent::LocaleChanged { locale }) if locale == "zh-CN"
    ));
    let invalidation = tree.invalidation.lock().unwrap_or_else(|e| e.into_inner());
    assert!(invalidation.has_layout());
    assert!(invalidation.node_needs_paint(root));
}

#[test]
fn nested_invalidation_batch_flushes_at_outer_boundary() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
    tree.layout();
    tree.reset_invalidation();

    tree.begin_invalidation_batch();
    tree.begin_invalidation_batch();
    tree.push_layout_invalidation(root);
    tree.push_paint_invalidation(root, Some(Rect::new(0.0, 0.0, 10.0, 10.0)));
    tree.finish_invalidation_batch();

    {
        let invalidation = tree
            .invalidation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        assert!(invalidation.is_empty());
    }

    tree.finish_invalidation_batch();
    let invalidation = tree
        .invalidation
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    assert!(invalidation.has_layout());
    assert!(invalidation.node_needs_paint(root));
}

#[test]
fn hit_test_root_contains() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
    tree.layout();
    assert!(tree.hit_test(Point::new(50.0, 25.0)).is_some());
}

#[test]
fn hit_test_outside_returns_none() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
    tree.layout();
    assert!(tree.hit_test(Point::new(200.0, 200.0)).is_none());
    assert!(tree.hit_test(Point::new(-1.0, 25.0)).is_none());
}

#[test]
fn hit_test_returns_deepest_child() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(child));
}

#[test]
fn hit_test_skips_invisible() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    tree.get_mut(child).unwrap().set_visible(false);
    assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(root));
}

#[test]
fn hit_test_skips_children_outside_parent_clip() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(ClipContainer::new(
        100.0,
        100.0,
        120.0,
        vec![Box::new(SpyWidget::new(80.0, 40.0))],
    )));
    tree.layout();

    let child = tree.get(root).unwrap().children()[0];
    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(0.0, 120.0, 80.0, 40.0)
    );
    assert_eq!(tree.hit_test(Point::new(40.0, 130.0)), None);
}

#[test]
fn hit_test_keeps_children_inside_parent_clip() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(ClipContainer::new(
        100.0,
        100.0,
        60.0,
        vec![Box::new(SpyWidget::new(80.0, 40.0))],
    )));
    tree.layout();

    let child = tree.get(root).unwrap().children()[0];
    assert_eq!(tree.hit_test(Point::new(40.0, 80.0)), Some(child));
}

#[test]
fn dispatch_pointer_down_focuses_target() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let btn = tree.add_child(root_id, Box::new(SpyWidget::new(80.0, 40.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(btn)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 80.0, 40.0));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(40.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
}

#[test]
fn dispatch_pointer_down_targets_overlay_owner() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let owner = tree.add_child(root_id, Box::new(SpyWidget::new(20.0, 20.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(owner)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 20.0, 20.0));

    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(owner, OverlayKind::Popover)
            .bounds(Rect::new(50.0, 50.0, 60.0, 40.0))
            .z_index(10),
    );

    let result = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(70.0, 70.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    let events = tree
        .get(owner)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow()
        .clone();

    assert_eq!(result, EventResult::Handled);
    assert!(events
        .iter()
        .any(|event| matches!(event, SystemEvent::PointerDown { .. })));
    assert_eq!(tree.managers().focus.focused_component(), Some(owner));
}

#[test]
fn dispatch_wheel_targets_overlay_owner_before_main_tree() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let underlying = tree.add_child(root_id, Box::new(SpyWidget::new(200.0, 200.0)));
    let owner = tree.add_child(root_id, Box::new(SpyWidget::new(20.0, 20.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(underlying)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(owner)
        .unwrap()
        .set_frame(Rect::new(150.0, 150.0, 20.0, 20.0));

    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(owner, OverlayKind::Popover)
            .bounds(Rect::new(50.0, 50.0, 60.0, 40.0))
            .z_index(10),
    );

    let result = tree.dispatch_event(&SystemEvent::Wheel {
        pos: Point::new(70.0, 70.0),
        delta: Point::new(0.0, -1.0),
    });

    let overlay_events = tree
        .get(owner)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow()
        .clone();
    let underlying_events = tree
        .get(underlying)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow()
        .clone();

    assert_eq!(result, EventResult::Handled);
    assert!(overlay_events
        .iter()
        .any(|event| matches!(event, SystemEvent::Wheel { .. })));
    assert!(underlying_events.is_empty());
}

#[test]
fn scroll_composite_viewport_is_clipped_to_component_frame() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(ScrollCompositeViewportProbe {
        pending_delta: Cell::new(None),
        viewport: Rect::new(-20.0, 20.0, 80.0, 200.0),
    }));
    tree.get_mut(id)
        .expect("scroll probe")
        .set_frame(Rect::new(10.0, 10.0, 100.0, 100.0));
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: Point::new(20.0, 30.0),
            delta: Point::new(0.0, -1.0),
        }),
        EventResult::Handled
    );

    assert_eq!(
        tree.scroll_region_moves(),
        Some(vec![(Rect::new(10.0, 20.0, 50.0, 90.0), 0.0, 10.0)])
    );
}

#[test]
fn dispatch_pointer_down_outside_modal_overlay_dismisses_and_blocks_underlying() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let underlying = tree.add_child(root_id, Box::new(SpyWidget::new(200.0, 200.0)));
    let owner = tree.add_child(root_id, Box::new(SpyWidget::new(20.0, 20.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(underlying)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(owner)
        .unwrap()
        .set_frame(Rect::new(150.0, 150.0, 20.0, 20.0));

    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(owner, OverlayKind::Modal)
            .bounds(Rect::new(50.0, 50.0, 60.0, 40.0))
            .z_index(100),
    );

    let result = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(20.0, 20.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    let underlying_event = tree
        .get(underlying)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .last_event
        .borrow()
        .clone();

    assert_eq!(result, EventResult::Handled);
    assert!(tree.overlay_stack().is_empty());
    assert!(underlying_event.is_none());
    assert!(tree.managers().focus.focused_component().is_none());
}

#[test]
fn dismissing_focus_trap_restores_focus_before_trap() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(240.0, 160.0, vec![])));
    let outside = tree.add_child(
        root_id,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)),
    );
    let owner = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(100.0, 80.0, vec![])),
    );
    let inside = tree.add_child(
        owner,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)),
    );
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 240.0, 160.0));
    tree.get_mut(outside)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 20.0, 20.0));
    tree.get_mut(owner)
        .unwrap()
        .set_frame(Rect::new(60.0, 40.0, 100.0, 80.0));
    tree.get_mut(inside)
        .unwrap()
        .set_frame(Rect::new(60.0, 40.0, 20.0, 20.0));
    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(owner, OverlayKind::Modal)
            .bounds(Rect::new(50.0, 30.0, 120.0, 100.0))
            .z_index(1000),
    );
    tree.managers_mut()
        .focus
        .set_focused_component(Some(outside));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(inside));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(20.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );

    assert!(tree.overlay_stack().is_empty());
    assert_eq!(tree.managers().focus.focused_component(), Some(outside));
}

#[test]
fn dismissing_focus_trap_drops_stale_restore_target() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(240.0, 160.0, vec![])));
    let outside = tree.add_child(
        root_id,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)),
    );
    let owner = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(100.0, 80.0, vec![])),
    );
    let inside = tree.add_child(
        owner,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)),
    );
    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(owner, OverlayKind::Modal)
            .bounds(Rect::new(50.0, 30.0, 120.0, 100.0))
            .z_index(1000),
    );
    tree.managers_mut()
        .focus
        .set_focused_component(Some(outside));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(inside));

    tree.remove(outside);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(20.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );

    assert!(tree.managers().focus.focused_component().is_none());
}

#[test]
fn rebuilding_removed_focus_trap_restores_focus_before_trap() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(240.0, 160.0, vec![])));
    let outside = tree.add_child(
        root_id,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)),
    );
    let owner = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(100.0, 80.0, vec![])),
    );
    let inside = tree.add_child(
        owner,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)),
    );
    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(owner, OverlayKind::Modal)
            .bounds(Rect::new(50.0, 30.0, 120.0, 100.0))
            .z_index(1000),
    );
    tree.managers_mut()
        .focus
        .set_focused_component(Some(outside));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(inside));

    tree.rebuild_widget_overlays();

    assert!(tree.overlay_stack().is_empty());
    assert_eq!(tree.managers().focus.focused_component(), Some(outside));
}

#[test]
fn rebuilding_removed_focus_trap_drops_stale_restore_target() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(240.0, 160.0, vec![])));
    let outside = tree.add_child(
        root_id,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)),
    );
    let owner = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(100.0, 80.0, vec![])),
    );
    let inside = tree.add_child(
        owner,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)),
    );
    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(owner, OverlayKind::Modal)
            .bounds(Rect::new(50.0, 30.0, 120.0, 100.0))
            .z_index(1000),
    );
    tree.managers_mut()
        .focus
        .set_focused_component(Some(outside));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(inside));

    tree.remove(outside);
    tree.rebuild_widget_overlays();

    assert!(tree.overlay_stack().is_empty());
    assert!(tree.managers().focus.focused_component().is_none());
}

#[test]
fn nested_focus_traps_restore_focus_in_lifo_order() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(260.0, 180.0, vec![])));
    let outside = tree.add_child(
        root_id,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)),
    );
    let first_owner = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(180.0, 130.0, vec![])),
    );
    let first_inside = tree.add_child(
        first_owner,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)),
    );
    let second_owner = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(100.0, 80.0, vec![])),
    );
    let second_inside = tree.add_child(
        second_owner,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(3)),
    );
    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(first_owner, OverlayKind::Modal)
            .bounds(Rect::new(0.0, 0.0, 200.0, 150.0))
            .z_index(1000),
    );
    tree.managers_mut()
        .focus
        .set_focused_component(Some(outside));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.managers().focus.focused_component(),
        Some(first_inside)
    );

    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(second_owner, OverlayKind::Modal)
            .bounds(Rect::new(50.0, 50.0, 100.0, 80.0))
            .z_index(1100),
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.managers().focus.focused_component(),
        Some(second_inside)
    );

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(20.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.managers().focus.focused_component(),
        Some(first_inside)
    );
    assert_eq!(tree.overlay_stack().len(), 1);

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(220.0, 160.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(outside));
    assert!(tree.overlay_stack().is_empty());
}

#[test]
fn removing_focus_trap_owner_restores_focus_before_trap() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(240.0, 160.0, vec![])));
    let outside = tree.add_child(
        root_id,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)),
    );
    let owner = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(100.0, 80.0, vec![])),
    );
    let inside = tree.add_child(
        owner,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)),
    );
    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(owner, OverlayKind::Modal)
            .bounds(Rect::new(50.0, 30.0, 120.0, 100.0))
            .z_index(1000),
    );
    tree.managers_mut()
        .focus
        .set_focused_component(Some(outside));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(inside));

    tree.remove(owner);

    assert!(tree.overlay_stack().is_empty());
    assert_eq!(tree.managers().focus.focused_component(), Some(outside));
}

#[test]
fn removing_lower_focus_trap_owner_does_not_steal_focus_from_top_trap() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(260.0, 180.0, vec![])));
    let outside = tree.add_child(
        root_id,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)),
    );
    let lower_owner = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(180.0, 130.0, vec![])),
    );
    let lower_inside = tree.add_child(
        lower_owner,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(2)),
    );
    let top_owner = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(100.0, 80.0, vec![])),
    );
    let top_inside = tree.add_child(
        top_owner,
        Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(3)),
    );
    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(lower_owner, OverlayKind::Modal)
            .bounds(Rect::new(0.0, 0.0, 200.0, 150.0))
            .z_index(1000),
    );
    tree.managers_mut()
        .focus
        .set_focused_component(Some(outside));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.managers().focus.focused_component(),
        Some(lower_inside)
    );

    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(top_owner, OverlayKind::Modal)
            .bounds(Rect::new(50.0, 50.0, 100.0, 80.0))
            .z_index(1100),
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Tab,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(top_inside));

    tree.remove(lower_owner);

    assert_eq!(tree.overlay_stack().len(), 1);
    assert_eq!(
        tree.overlay_stack().top().map(|entry| entry.owner()),
        Some(top_owner)
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(top_inside));
}

#[test]
fn layout_registers_visible_modal_overlay() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let modal = tree.add_child(
        root_id,
        Box::new(Modal::new("Dialog").visible(true).overlay(true)),
    );
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    tree.layout();

    let top = tree.overlay_stack().top().unwrap();
    assert_eq!(top.owner(), modal);
    assert_eq!(top.kind(), OverlayKind::Modal);
    assert!(top.is_modal());
    assert!(top.traps_focus());
    assert!(!top.is_managed());
}

#[test]
fn layout_rebuilds_widget_overlay_without_duplicates() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let modal = tree.add_child(
        root_id,
        Box::new(Modal::new("Dialog").visible(true).overlay(true)),
    );
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    tree.layout();
    tree.layout();

    let modal_entries = tree
        .overlay_stack()
        .iter()
        .filter(|entry| entry.owner() == modal && entry.kind() == OverlayKind::Modal)
        .count();
    assert_eq!(modal_entries, 1);
}

#[test]
fn layout_preserves_managed_overlay_entries_between_rebuilds() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let modal = tree.add_child(
        root_id,
        Box::new(Modal::new("Dialog").visible(true).overlay(true)),
    );
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    let managed_id = tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(root_id, OverlayKind::ContextMenu)
            .bounds(Rect::new(10.0, 10.0, 80.0, 80.0))
            .z_index(1200)
            .managed(true),
    );

    tree.layout();
    tree.layout();

    assert!(tree
        .overlay_stack()
        .iter()
        .any(|entry| entry.id() == managed_id && entry.is_managed()));
    let modal_entries = tree
        .overlay_stack()
        .iter()
        .filter(|entry| entry.owner() == modal && entry.kind() == OverlayKind::Modal)
        .count();
    assert_eq!(modal_entries, 1);
}

#[test]
fn layout_discards_unmanaged_stale_overlay_entries() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(root_id, OverlayKind::Popover)
            .bounds(Rect::new(10.0, 10.0, 80.0, 80.0))
            .z_index(900),
    );

    tree.layout();

    assert!(tree.overlay_stack().is_empty());
}

#[test]
fn delayed_tooltip_waits_for_timer_before_overlay() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let tooltip = tree.add_child(
        root_id,
        Box::new(Tooltip::new("Help").delay_ms(300).timer_id(42)),
    );
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(tooltip)
        .unwrap()
        .set_frame(Rect::new(10.0, 10.0, 80.0, 20.0));

    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(20.0, 15.0),
        mods: KeyMod::NONE,
    });

    assert_eq!(
        tree.active_timers(),
        vec![(
            WidgetTree::timer_work_key(tooltip, 42),
            std::time::Duration::from_millis(300)
        )]
    );

    assert!(tree
        .overlay_stack()
        .iter()
        .all(|entry| entry.kind() != OverlayKind::Tooltip));

    tree.dispatch_event(&SystemEvent::Timer { id: 41 });
    assert!(tree
        .overlay_stack()
        .iter()
        .all(|entry| entry.kind() != OverlayKind::Tooltip));

    tree.dispatch_event(&SystemEvent::Timer { id: 42 });

    let top = tree.overlay_stack().top().unwrap();
    assert_eq!(top.owner(), tooltip);
    assert_eq!(top.kind(), OverlayKind::Tooltip);
}

#[test]
fn delayed_tooltips_use_widget_scoped_timer_keys_by_default() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let first = tree.add_child(root_id, Box::new(Tooltip::new("One").delay_ms(300)));
    let second = tree.add_child(root_id, Box::new(Tooltip::new("Two").delay_ms(300)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(first)
        .unwrap()
        .set_frame(Rect::new(10.0, 10.0, 80.0, 20.0));
    tree.get_mut(second)
        .unwrap()
        .set_frame(Rect::new(10.0, 40.0, 80.0, 20.0));

    assert_eq!(
        tree.dispatch_to(first, &SystemEvent::PointerEnter),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_to(second, &SystemEvent::PointerEnter),
        EventResult::Handled
    );

    let timers = tree.active_timers();
    assert_eq!(timers.len(), 2);
    assert!(timers.contains(&(
        WidgetTree::timer_work_key(first, 1),
        std::time::Duration::from_millis(300)
    )));
    assert!(timers.contains(&(
        WidgetTree::timer_work_key(second, 1),
        std::time::Duration::from_millis(300)
    )));
    assert_ne!(timers[0].0, timers[1].0);
}

#[test]
fn dispatch_pointer_down_empty_space_clears_focus() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.layout();
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(300.0, 300.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
}

#[test]
fn dispatch_key_to_focused_component() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.layout();
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
}

#[test]
fn enter_and_space_emit_click_for_focused_component() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(SpyWidget::new(200.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(10.0, 20.0, 200.0, 100.0));
    tree.managers_mut()
        .focus
        .set_focused_component(Some(root_id));

    let clicks = Rc::new(RefCell::new(Vec::new()));
    let clicks_for_handler = clicks.clone();
    tree.handler_table()
        .on(root_id, crate::ui::SemanticKind::Click, move |event| {
            if let Some(click) = event.click_payload() {
                clicks_for_handler.borrow_mut().push(*click);
            }
        });

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Space,
            mods: KeyMod::SHIFT,
        }),
        EventResult::Handled
    );

    let clicks = clicks.borrow();
    assert_eq!(clicks.len(), 2);
    assert_eq!(clicks[0].button, MouseButton::Left);
    assert_eq!(clicks[0].pos, Point::new(110.0, 70.0));
    assert_eq!(clicks[0].modifiers, KeyMod::NONE);
    assert_eq!(clicks[1].button, MouseButton::Left);
    assert_eq!(clicks[1].pos, Point::new(110.0, 70.0));
    assert_eq!(clicks[1].modifiers, KeyMod::SHIFT);
}

#[test]
fn dispatch_pointer_move_triggers_hover_enter_leave() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(50.0, 50.0),
        mods: KeyMod::NONE,
    });
}

#[test]
fn pointer_move_inside_same_hover_skips_dispatch_without_opt_in() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerMove {
            pos: Point::new(50.0, 50.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let event_count = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow()
        .len();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerMove {
            pos: Point::new(60.0, 60.0),
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );

    let events = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow();
    assert_eq!(events.len(), event_count);
}

#[test]
fn continuous_pointer_move_opt_in_receives_same_hover_moves() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(ContinuousSpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(50.0, 50.0),
        mods: KeyMod::NONE,
    });
    let event_count = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ContinuousSpyWidget>()
        .unwrap()
        .0
        .events
        .borrow()
        .len();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerMove {
            pos: Point::new(60.0, 60.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );

    let events = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ContinuousSpyWidget>()
        .unwrap()
        .0
        .events
        .borrow();
    let new_events = &events[event_count..];
    assert!(new_events
        .iter()
        .any(|event| matches!(event, SystemEvent::PointerMove { .. })));
}

#[test]
fn pointer_down_target_receives_move_even_after_leaving_hover_frame() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let event_count = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow()
        .len();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerMove {
            pos: Point::new(150.0, 150.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );

    let events = tree
        .get(child)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap()
        .events
        .borrow();
    let new_events = &events[event_count..];
    assert!(new_events
        .iter()
        .any(|event| matches!(event, SystemEvent::PointerMove { .. })));
}

#[test]
fn dispatch_resize_goes_to_root() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    let child = tree.add_child(root, Box::new(SpyWidget::new(50.0, 40.0)));
    tree.layout();
    tree.reset_invalidation();
    assert!(tree.layout_traverse().is_empty());

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Resize {
            width: 400.0,
            height: 300.0
        }),
        EventResult::Handled
    );

    let invalidation = tree.invalidation.lock().unwrap_or_else(|e| e.into_inner());
    assert!(invalidation.has_layout());
    assert!(invalidation.layout_roots().contains(&root));
    assert!(invalidation.node_needs_paint(root));
    drop(invalidation);

    assert_eq!(
        tree.get(root).unwrap().frame(),
        Rect::new(0.0, 0.0, 400.0, 300.0)
    );
    assert_eq!(tree.layout_traverse(), vec![root, child]);
}

/// Resize 后 layout 不得把根 frame 收缩回内容固有高度（窗口客户区是权威尺寸）。
#[test]
fn resize_root_survives_layout_without_engine_sync() {
    let root_view = column([
        row([label("nav").width(200.0), label("content").flex_grow(1.0)]).flex_grow(1.0),
        label("status"),
    ])
    .flex_grow(1.0);
    let mut tree = ViewAdapter::build(root_view);
    let rid = tree.root_id().expect("root");
    tree.get_mut(rid)
        .expect("root mut")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.layout();

    tree.dispatch_event(&SystemEvent::Resize {
        width: 1000.0,
        height: 800.0,
    });
    tree.layout();

    let root_frame = tree.get(rid).expect("root").frame();
    assert!(
        (root_frame.w - 1000.0).abs() < 0.5 && (root_frame.h - 800.0).abs() < 0.5,
        "root should stay at window size after layout, got {}x{}",
        root_frame.w,
        root_frame.h
    );

    let main = tree.get(rid).expect("root").children()[0];
    let content = tree.get(main).expect("main").children()[1];
    let content_w = tree.get(content).expect("content").frame().w;
    assert!(
        content_w > 700.0,
        "flex content should grow with window, got {content_w}"
    );
}

/// 复现 demo 卡顿：row Stretch 把 column_fit 侧栏拉到客户区高后，
/// Phase 4 不得再按内容缩回（否则 Phase 1 拉满 ↔ Phase 4 收缩空转）。
#[test]
fn stretch_sidebar_does_not_phase4_thrash_against_parent_allocation() {
    let shell = column([
        row([
            column_fit([
                label("brand"),
                label("nav-a"),
                label("nav-b"),
                label("nav-c"),
                label("").flex_grow(1.0),
                label("footer"),
            ])
            .width(220.0),
            column([
                label("header").height(48.0),
                scroll(column_fit([
                    label("block-1").height(120.0),
                    label("block-2").height(120.0),
                    label("block-3").height(120.0),
                ]))
                .flex_grow(1.0)
                .into(),
            ])
            .flex_grow(1.0),
        ])
        .flex_grow(1.0),
        label("status").height(28.0),
    ])
    .flex_grow(1.0);

    let mut tree = ViewAdapter::build(shell);
    let rid = tree.root_id().expect("root");
    tree.get_mut(rid)
        .expect("root mut")
        .set_frame(Rect::new(0.0, 0.0, 1200.0, 800.0));
    let _ = tree.take_layout_frame_writes();
    let _ = tree.take_layout_shrink_ops();

    tree.layout();
    let shrink_ops = tree.take_layout_shrink_ops();
    let first_writes = tree.take_layout_frame_writes();
    assert!(
        first_writes > 0,
        "initial layout should place frames, got writes={first_writes}"
    );

    let main = tree.get(rid).expect("root").children()[0];
    let sidebar = tree.get(main).expect("main row").children()[0];
    let sidebar_h = tree.get(sidebar).expect("sidebar").frame().h;
    let row_h = tree.get(main).expect("main row").frame().h;
    assert!(
        (sidebar_h - row_h).abs() < 0.5,
        "stretch sidebar must keep parent allocation, sidebar={sidebar_h} row={row_h}"
    );
    assert!(
        shrink_ops == 0,
        "Phase 4 must not shrink stretch-allocated sidebar (thrash fuel), got {shrink_ops} ops"
    );

    // 再次强制 layout：结果不变时不得再写 frame / Phase 4
    let frames_before: Vec<_> = tree
        .traverse()
        .iter()
        .copied()
        .map(|id| (id, tree.get(id).expect("node").frame()))
        .collect();
    let _ = tree.take_layout_frame_writes();
    let _ = tree.take_layout_shrink_ops();
    tree.push_layout_invalidation(rid);
    tree.layout();
    let second_writes = tree.take_layout_frame_writes();
    let second_shrink = tree.take_layout_shrink_ops();
    assert_eq!(
        second_writes, 0,
        "stable re-layout must not rewrite frames (got {second_writes})"
    );
    assert_eq!(
        second_shrink, 0,
        "stable re-layout must not Phase 4 shrink (got {second_shrink})"
    );
    for (id, before) in frames_before {
        let after = tree.get(id).expect("node").frame();
        assert_eq!(before, after, "frame changed on stable re-layout for {id}");
    }
}

/// 复现 demo 卡顿：ScrollView 内 wrap Space（height=下限）内容撑开后，
/// 不得与 Phase 1/sibling re-layout 在 120↔124 间空转至 max converge。
#[test]
fn viewport_wrap_space_does_not_phase2_viewport_thrash() {
    use crate::ui::layout::{AlignItems, FlexDirection};
    use crate::ui::widgets::general::space::SpaceSize;
    use crate::ui::widgets::Space;

    // min_h=120；窄宽下三块 200×64 换行 → 交叉轴约 64+8+64=136 > 120
    let wrap_row = Space::new()
        .height(120.0)
        .direction(FlexDirection::Row)
        .wrap(true)
        .size(SpaceSize::Small)
        .flex_grow(1.0)
        .align(AlignItems::Start)
        .child(Label::new("A").size(200.0, 64.0))
        .child(Label::new("B").size(200.0, 64.0))
        .child(Label::new("C").size(200.0, 64.0));

    let shell = column([
        scroll(column_fit([
            embed(wrap_row),
            label("tall-tail").height(900.0),
        ]))
        .flex_grow(1.0)
        .into(),
        label("status").height(28.0),
    ])
    .flex_grow(1.0);

    let mut tree = ViewAdapter::build(shell);
    let rid = tree.root_id().expect("root");
    // 宽约 420：两列换行，触发交叉轴超过 min_h
    tree.get_mut(rid)
        .expect("root mut")
        .set_frame(Rect::new(0.0, 0.0, 420.0, 600.0));
    tree.push_layout_invalidation(rid);
    let _ = tree.take_layout_frame_writes();
    let _ = tree.take_layout_converge_passes();
    let _ = tree.take_layout_expand_ops();

    tree.layout();
    let first_passes = tree.take_layout_converge_passes();
    let first_expands = tree.take_layout_expand_ops();
    assert!(
        first_passes < 10,
        "must converge before max iterations, got passes={first_passes} expands={first_expands}"
    );
    assert!(
        first_passes <= 4,
        "viewport+wrap should stabilize quickly, got passes={first_passes}"
    );

    let scroll_id = tree
        .find_all_by_type::<crate::ui::widgets::ScrollView>()
        .into_iter()
        .next()
        .map(|(id, _)| id)
        .expect("ScrollView");
    let content = tree.get(scroll_id).expect("scroll").children()[0];
    let wrap_space = tree.get(content).expect("content col").children()[0];
    let wrap_h = tree.get(wrap_space).expect("wrap space").frame().h;
    assert!(
        wrap_h > 120.5,
        "wrap Space must keep expanded cross size, got h={wrap_h}"
    );

    // 稳定后再 layout：零 frame 写、收敛 1 遍、无 Phase 2 扩展
    let _ = tree.take_layout_frame_writes();
    let _ = tree.take_layout_converge_passes();
    let _ = tree.take_layout_expand_ops();
    tree.push_layout_invalidation(rid);
    tree.layout();
    assert_eq!(
        tree.take_layout_frame_writes(),
        0,
        "stable viewport layout must not rewrite frames"
    );
    assert_eq!(
        tree.take_layout_expand_ops(),
        0,
        "stable layout must not Phase 2 expand again"
    );
    assert_eq!(
        tree.take_layout_converge_passes(),
        1,
        "stable layout should finish on first converge pass"
    );

    // resize 宽度后同样不得打满 converge
    tree.dispatch_event(&SystemEvent::Resize {
        width: 380.0,
        height: 640.0,
    });
    let _ = tree.take_layout_converge_passes();
    let _ = tree.take_layout_expand_ops();
    tree.layout();
    let resize_passes = tree.take_layout_converge_passes();
    assert!(
        resize_passes < 10,
        "resize must not thrash to max passes, got {resize_passes}"
    );
    assert!(
        resize_passes <= 4,
        "resize should re-converge quickly, got {resize_passes}"
    );
}

/// 复现 demo 首页卡顿：ScrollView 内定高 Card（Column）里的 wrap Space，
/// 内容交叉轴超过 Card content 高（约 106→121）时，不得与 Phase 1 钳制振荡至 max。
#[test]
fn fixed_height_card_wrap_inside_viewport_does_not_phase2_thrash() {
    use crate::ui::layout::{AlignItems, FlexDirection};
    use crate::ui::widgets::display::tag::Tag;
    use crate::ui::widgets::general::space::SpaceSize;
    use crate::ui::widgets::Space;

    // 对齐 demo_card(220×140)：padding 16 → content ≈ 188×108
    let card = column_fit([
        label("覆盖范围").font_size(14.0),
        space(10.0),
        column_fit([
            label("11").font_size(36.0),
            space(4.0),
            label("个演示页").font_size(13.0),
            space(10.0),
            embed(
                Space::new()
                    .size(SpaceSize::Small)
                    .direction(FlexDirection::Row)
                    .wrap(true)
                    .align(AlignItems::Start)
                    .child(Tag::new("组件"))
                    .child(Tag::new("运行时"))
                    .child(Tag::new("主题")),
            ),
        ]),
    ])
    .width(220.0)
    .height(140.0)
    .padding(16.0);

    let shell = column([
        scroll(
            column_fit([
                // 与 demo 首页一致：卡片在 row 内保持 220，不致被 ScrollView 拉满宽
                row([embed(card)]).align(AlignItems::Start),
                label("tall-tail").height(900.0),
            ])
            .overflow_content(),
        )
        .both()
        .flex_grow(1.0)
        .into(),
        label("status").height(28.0),
    ])
    .flex_grow(1.0);

    let mut tree = ViewAdapter::build(shell);
    let rid = tree.root_id().expect("root");
    tree.get_mut(rid)
        .expect("root mut")
        .set_frame(Rect::new(0.0, 0.0, 1200.0, 800.0));
    tree.push_layout_invalidation(rid);
    let _ = tree.take_layout_frame_writes();
    let _ = tree.take_layout_converge_passes();
    let _ = tree.take_layout_expand_ops();

    tree.layout();
    let first_passes = tree.take_layout_converge_passes();
    let first_expands = tree.take_layout_expand_ops();
    assert!(
        first_passes < 10,
        "must converge before max iterations, got passes={first_passes} expands={first_expands}"
    );
    assert!(
        first_passes <= 4,
        "fixed card + wrap in viewport should stabilize quickly, got passes={first_passes}"
    );

    // 定位含 3 个 Tag 的 wrap Space（demo 卡片内）
    let wrap_id = tree
        .traverse()
        .iter()
        .copied()
        .find(|&id| {
            let kids = tree
                .get(id)
                .map(|n| n.children().to_vec())
                .unwrap_or_default();
            kids.len() == 3
                && kids.iter().all(|&cid| {
                    tree.get(cid)
                        .and_then(|n| n.component().as_any().downcast_ref::<Tag>())
                        .is_some()
                })
        })
        .expect("wrap Space with 3 Tags");
    let wrap_before = tree.get(wrap_id).expect("wrap").frame();
    assert!(
        wrap_before.w > 100.0 && wrap_before.w < 250.0,
        "wrap Space should stay near card content width (~188), got w={}",
        wrap_before.w
    );

    // 稳定后再 layout：不得再 Phase 2 扩（106↔121 空转）
    let _ = tree.take_layout_frame_writes();
    let _ = tree.take_layout_converge_passes();
    let _ = tree.take_layout_expand_ops();
    tree.push_layout_invalidation(rid);
    tree.layout();
    assert_eq!(
        tree.take_layout_expand_ops(),
        0,
        "stable layout must not Phase 2 expand again (106↔121 thrash)"
    );
    assert_eq!(
        tree.take_layout_converge_passes(),
        1,
        "stable layout should finish on first converge pass"
    );
    assert_eq!(
        tree.take_layout_frame_writes(),
        0,
        "stable layout must not rewrite frames"
    );
    let wrap_after = tree.get(wrap_id).expect("wrap").frame();
    assert_eq!(
        wrap_before, wrap_after,
        "wrap Space frame must not oscillate on stable re-layout"
    );
}

/// 真因回归：ScrollView 下定高中间层仍须 parent_cap。
/// 旧逻辑用 nearest_viewport_overflow_axes 放开视口下所有节点的 cap，
/// Phase 2 把定高 Card 子树撑过父槽，下一轮 Phase 1 写回 → 高度振荡打满 converge。
#[test]
fn scrollview_fixed_intermediate_keeps_parent_cap_no_phase_oscillation() {
    use crate::ui::widgets::containers::Container;

    // ScrollView → overflow 内容列 → 定高 100 的中间层 → 固有高 150 的叶子
    let shell = scroll(
        column_fit([
            column_fit([label("tall-leaf").width(180.0).height(150.0)])
                .width(200.0)
                .height(100.0),
            label("tail").height(400.0),
        ])
        .overflow_content(),
    )
    .size(220.0, 160.0);

    let mut tree = ViewAdapter::build(shell);
    let rid = tree.root_id().expect("root");
    tree.get_mut(rid)
        .expect("root mut")
        .set_frame(Rect::new(0.0, 0.0, 220.0, 160.0));
    tree.push_layout_invalidation(rid);
    let _ = tree.take_layout_frame_trace();
    let _ = tree.take_layout_converge_passes();
    let _ = tree.take_layout_expand_ops();

    tree.layout();
    let passes = tree.take_layout_converge_passes();
    let expands = tree.take_layout_expand_ops();
    let trace = tree.take_layout_frame_trace();
    assert!(
        passes < 10,
        "must not thrash to max converge, passes={passes} expands={expands} trace={trace:?}"
    );
    assert!(
        passes <= 4,
        "fixed intermediate under scroll should stabilize quickly, got {passes}"
    );

    // 定高中间层 frame.h 必须停在 100，不得被 Phase 2 撑到 150
    let fixed_id = tree
        .traverse()
        .iter()
        .copied()
        .find(|&id| {
            tree.get(id)
                .and_then(|n| n.component().as_any().downcast_ref::<Container>())
                .is_some_and(|c| c.style.height == Some(100.0) && c.style.width == Some(200.0))
        })
        .expect("fixed-height intermediate Container");
    let fixed_frame = tree.get(fixed_id).expect("fixed").frame();
    assert!(
        (fixed_frame.h - 100.0).abs() < 1.0,
        "fixed intermediate must keep h=100 under scroll parent_cap, got {}",
        fixed_frame.h
    );

    // 证据：同一节点不得出现 Phase1 写矮高 + Phase2 写回更高的振荡对
    let mut by_id: std::collections::HashMap<ComponentId, Vec<(u8, i32, i32)>> =
        std::collections::HashMap::new();
    for (phase, id, before, after) in &trace {
        if before != after {
            by_id
                .entry(*id)
                .or_default()
                .push((*phase, *before, *after));
        }
    }
    for (id, writes) in &by_id {
        let has_p1_short = writes.iter().any(|(p, b, a)| *p == 1 && a < b);
        let has_p2_grow = writes.iter().any(|(p, b, a)| *p == 2 && a > b);
        assert!(
            !(has_p1_short && has_p2_grow),
            "id={id:?} oscillated Phase1 shrink + Phase2 grow: {writes:?}"
        );
    }

    // 稳定再 layout：不得再 Phase 2 扩展
    let _ = tree.take_layout_expand_ops();
    let _ = tree.take_layout_converge_passes();
    tree.push_layout_invalidation(rid);
    tree.layout();
    assert_eq!(
        tree.take_layout_expand_ops(),
        0,
        "stable re-layout must not Phase2-expand"
    );
    assert_eq!(tree.take_layout_converge_passes(), 1);

    // 滚动内容根仍可高于视口（parent 是 viewport 时放开 cap）
    let sv = tree
        .traverse()
        .iter()
        .copied()
        .find(|&id| {
            tree.get(id)
                .and_then(|n| {
                    n.component()
                        .as_any()
                        .downcast_ref::<crate::ui::widgets::ScrollView>()
                })
                .is_some()
        })
        .expect("scrollview");
    let content_id = tree.get(sv).expect("sv").children()[0];
    let content_h = tree.get(content_id).expect("content").frame().h;
    assert!(
        content_h > 160.0,
        "scroll content root must still grow past viewport, got {content_h}"
    );
}

/// Container 定高 Column 不得把子项 measure 钳回 content_rect.h（intrinsic 内容更高时）。
#[test]
fn container_measure_does_not_clamp_child_below_intrinsic_on_main_axis() {
    use crate::core::{Constraints, EdgeInsets, Size};
    use crate::ui::traits::WidgetLayout;
    use crate::ui::widgets::containers::Container;
    use crate::ui::widgets::Space;

    let space = Space::new().height(106.0);
    space.cached_content_size.set(Size::new(188.0, 121.0));
    // 模拟定高 Card content 区：max_h=106 曾把 121 压回 → Phase 2 振荡
    let tight = Constraints::loose(Size::new(188.0, 106.0));
    let measured = space.measure(tight);
    assert!(
        measured.h >= 120.5,
        "Space measure must resist parent max clamp below cached content, got {}",
        measured.h
    );

    let col = Container::new().h(140.0).padding(EdgeInsets::uniform(16.0));
    col.cached_content_size.set(Size::new(188.0, 121.0));
    let measured_col = col.measure(Constraints::loose(Size::new(220.0, 140.0)));
    assert!(
        measured_col.h >= 140.0,
        "Container fixed height is a floor at content cache"
    );
}

/// Space.height 是下限：cached 内容更高时 measure 须回报内容高（否则 Phase 1 写回矮值）。
#[test]
fn space_measure_floors_fixed_height_at_cached_content() {
    use crate::core::{Constraints, Size};
    use crate::ui::traits::WidgetLayout;
    use crate::ui::widgets::Space;

    let space = Space::new().height(120.0);
    space.cached_content_size.set(Size::new(400.0, 136.0));
    let measured = space.measure(Constraints::unconstrained());
    assert_eq!(
        measured.h, 136.0,
        "fixed height must not under-report wrapped content cache"
    );
    assert!(measured.w >= 400.0);
}

/// intrinsic column_fit（无 Stretch 拉满）布局后应稳定：二次 layout 零 frame 写、零 Phase 4。
#[test]
fn intrinsic_column_fit_layout_converges_without_empty_phase4() {
    let view = column_fit([
        label("title").height(24.0),
        label("body").height(80.0),
        label("foot").height(20.0),
    ])
    .width(240.0)
    .padding(8.0);

    let mut tree = ViewAdapter::build(view);
    let rid = tree.root_id().expect("root");
    tree.get_mut(rid)
        .expect("root mut")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 1.0));
    tree.push_layout_invalidation(rid);
    let _ = tree.take_layout_frame_writes();
    let _ = tree.take_layout_shrink_ops();

    tree.layout();
    let h1 = tree.get(rid).expect("root").frame().h;
    assert!(
        h1 > 100.0,
        "intrinsic column should expand to content, got h={h1}"
    );
    let _ = tree.take_layout_frame_writes();
    let _ = tree.take_layout_shrink_ops();

    tree.push_layout_invalidation(rid);
    tree.layout();
    assert_eq!(
        tree.take_layout_frame_writes(),
        0,
        "stable intrinsic layout must not rewrite frames"
    );
    assert_eq!(
        tree.take_layout_shrink_ops(),
        0,
        "stable intrinsic layout must not empty-Phase-4"
    );
    assert!(
        (tree.get(rid).expect("root").frame().h - h1).abs() < 0.5,
        "intrinsic height must stay converged"
    );
}

// Capture phase tests.

/// Capture phase: opt-in root handles the event before the child.
#[test]
fn capture_phase_root_handles_before_child() {
    let mut tree = WidgetTree::new();
    // Tree: CaptureSpyWidget(root, Handled) -> PassThroughContainer -> SpyWidget(child).
    // CaptureSpyWidget root opts into capture and handles before the child.
    let root_id = tree.set_root(Box::new(CaptureSpyWidget::new(300.0, 300.0)));
    let container = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(300.0, 300.0, vec![])),
    );
    let child = tree.add_child(container, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));
    tree.get_mut(container)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    let result = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    // SpyWidget(root) returns Handled during capture, so dispatch is handled.
    assert_eq!(result, EventResult::Handled);
    // Capture handled the event before bubbling, so focus is not moved to child.
    assert!(tree.managers().focus.focused_component().is_none());
}

#[test]
fn capture_phase_requires_explicit_opt_in() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(SpyWidget::new(300.0, 300.0)));
    let container = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(300.0, 300.0, vec![])),
    );
    let child = tree.add_child(container, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));
    tree.get_mut(container)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    let result = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(result, EventResult::Handled);
    assert_eq!(tree.managers().focus.focused_component(), Some(child));
    let root = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SpyWidget>()
        .unwrap();
    assert!(root.events.borrow().is_empty());
}

/// Capture phase falls through to bubble phase when not intercepted.
#[test]
fn capture_phase_not_intercepted_proceeds_to_bubble() {
    let mut tree = WidgetTree::new();
    // Tree: PassThroughContainer(root) -> SpyWidget(child).
    // PassThroughContainer returns NotHandled and does not intercept.
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    let result = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    // Capture does not intercept, then child handles the event during bubble.
    assert_eq!(result, EventResult::Handled);
    // Bubble dispatch sets focus to the child.
    assert_eq!(tree.managers().focus.focused_component(), Some(child));
}

/// Capture phase: wheel events can be intercepted before the target.
#[test]
fn capture_phase_wheel_intercepted() {
    let mut tree = WidgetTree::new();
    // Tree: CaptureSpyWidget(root, Handled) -> PassThroughContainer -> SpyWidget(child).
    let root_id = tree.set_root(Box::new(CaptureSpyWidget::new(300.0, 300.0)));
    let container = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(300.0, 300.0, vec![])),
    );
    let child = tree.add_child(container, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));
    tree.get_mut(container)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    let result = tree.dispatch_event(&SystemEvent::Wheel {
        pos: Point::new(50.0, 50.0),
        delta: Point::new(0.0, -10.0),
    });
    assert_eq!(result, EventResult::Handled);
}

/// Capture phase: key down events can be intercepted globally.
#[test]
fn capture_phase_key_down_intercepted() {
    let mut tree = WidgetTree::new();
    // Tree: CaptureSpyWidget(root, Handled) -> PassThroughContainer -> SpyWidget(child).
    let root_id = tree.set_root(Box::new(CaptureSpyWidget::new(300.0, 300.0)));
    let container = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(300.0, 300.0, vec![])),
    );
    let _child = tree.add_child(container, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));

    // Establish focus first so the key event has a target.
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    let result = tree.dispatch_event(&SystemEvent::KeyDown {
        key: KeyCode::Escape,
        mods: KeyMod::NONE,
    });
    // CaptureSpyWidget(root) handles during capture.
    assert_eq!(result, EventResult::Handled);
}

#[test]
fn right_pointer_up_emits_context_menu_semantic_event() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    let called = Rc::new(RefCell::new(false));
    let called_for_handler = called.clone();
    tree.handler_table().on(
        root_id,
        crate::ui::SemanticKind::ContextMenu,
        move |event| {
            if event.click_payload().is_some() {
                *called_for_handler.borrow_mut() = true;
            }
        },
    );

    let pos = Point::new(50.0, 50.0);
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });

    assert!(*called.borrow());
}

#[test]
fn secondary_pointer_context_menu_preserves_existing_focus() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new().size(240.0, 80.0)));
    let focused = tree.add_child(root, Box::new(Button::new("Focused")));
    let context_target = tree.add_child(root, Box::new(Button::new("Context")));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 240.0, 80.0));
    tree.get_mut(focused)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 32.0));
    tree.get_mut(context_target)
        .unwrap()
        .set_frame(Rect::new(120.0, 0.0, 100.0, 32.0));
    tree.set_focus(Some(focused));

    let pos = Point::new(140.0, 16.0);
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });

    assert_eq!(tree.managers().focus.focused_component(), Some(focused));
    assert_eq!(
        tree.overlay_stack().top().map(|entry| entry.kind()),
        Some(OverlayKind::ContextMenu)
    );
}

#[test]
fn pointer_click_waits_for_matching_release_button() {
    let mut tree = WidgetTree::new();
    let target = tree.set_root(Box::new(Button::new("Save")));
    tree.get_mut(target)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 32.0));
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let clicks_for_handler = clicks.clone();
    tree.handler_table()
        .on(target, SemanticKind::Click, move |event| {
            if let Some(click) = event.click_payload() {
                clicks_for_handler.borrow_mut().push(click.button);
            }
        });
    let pos = Point::new(20.0, 16.0);

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });

    assert!(clicks.borrow().is_empty());
    assert_eq!(
        tree.managers().interaction.pressed_component(),
        Some(target)
    );
    assert_eq!(
        tree.managers().interaction.pressed_button(),
        Some(MouseButton::Left)
    );
    assert!(tree
        .overlay_stack()
        .iter()
        .all(|entry| entry.kind() != OverlayKind::ContextMenu));

    tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(&*clicks.borrow(), &[MouseButton::Left]);
    assert_eq!(tree.managers().interaction.pressed_component(), None);
    assert_eq!(tree.managers().interaction.pressed_button(), None);
}

#[test]
fn secondary_press_cannot_be_completed_by_primary_release() {
    let mut tree = WidgetTree::new();
    let target = tree.set_root(Box::new(Button::new("Context")));
    tree.get_mut(target)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 32.0));
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let clicks_for_handler = clicks.clone();
    tree.handler_table()
        .on(target, SemanticKind::Click, move |event| {
            if let Some(click) = event.click_payload() {
                clicks_for_handler.borrow_mut().push(click.button);
            }
        });
    let pos = Point::new(20.0, 16.0);

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert!(clicks.borrow().is_empty());
    assert_eq!(
        tree.managers().interaction.pressed_button(),
        Some(MouseButton::Right)
    );

    tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });

    assert_eq!(&*clicks.borrow(), &[MouseButton::Right]);
    assert_eq!(
        tree.overlay_stack().top().map(|entry| entry.kind()),
        Some(OverlayKind::ContextMenu)
    );
}

#[test]
fn right_pointer_up_opens_context_menu_overlay_by_default() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    let pos = Point::new(50.0, 50.0);
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });

    let top = tree.overlay_stack().top().unwrap();
    assert_eq!(top.owner(), root_id);
    assert_eq!(top.kind(), OverlayKind::ContextMenu);
    assert!(top.dismisses_on_outside());
    assert!(top.is_managed());
    assert_eq!(
        top.bounds_rect(),
        Some(Rect::new(pos.x, pos.y, 160.0, 160.0))
    );
}

#[test]
fn right_pointer_up_replaces_existing_context_menu_overlay() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    for pos in [Point::new(30.0, 30.0), Point::new(70.0, 80.0)] {
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos,
            button: MouseButton::Right,
            mods: KeyMod::NONE,
        });
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos,
            button: MouseButton::Right,
            mods: KeyMod::NONE,
        });
    }

    let context_menus: Vec<_> = tree
        .overlay_stack()
        .iter()
        .filter(|entry| entry.kind() == OverlayKind::ContextMenu)
        .collect();

    assert_eq!(context_menus.len(), 1);
    assert_eq!(
        context_menus[0].bounds_rect(),
        Some(Rect::new(70.0, 80.0, 160.0, 160.0))
    );
}

#[test]
fn context_menu_prevent_default_skips_overlay() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    tree.handler_table()
        .on(root_id, crate::ui::SemanticKind::ContextMenu, |event| {
            event.prevent_default();
        });

    let pos = Point::new(50.0, 50.0);
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    });

    assert!(tree
        .overlay_stack()
        .iter()
        .all(|entry| entry.kind() != OverlayKind::ContextMenu));
}

#[test]
fn text_input_emits_semantic_event_for_focused_target() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    let text = Rc::new(RefCell::new(String::new()));
    let text_for_handler = text.clone();
    tree.handler_table()
        .on(root_id, crate::ui::SemanticKind::TextInput, move |event| {
            if let Some(value) = event.text_payload() {
                *text_for_handler.borrow_mut() = value.to_string();
            }
        });

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 10.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::TextInput {
        text: "hello".to_string(),
    });

    assert_eq!(&*text.borrow(), "hello");
}

#[test]
fn ime_composition_events_emit_semantic_events_for_focused_target() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    let started = Rc::new(RefCell::new(false));
    let started_for_handler = started.clone();
    tree.handler_table()
        .on_ime_composition_start(root_id, move || {
            *started_for_handler.borrow_mut() = true;
        });

    let update = Rc::new(RefCell::new(String::new()));
    let update_for_handler = update.clone();
    tree.handler_table()
        .on_ime_composition_update(root_id, move |text| {
            *update_for_handler.borrow_mut() = text.to_string();
        });

    let end = Rc::new(RefCell::new(String::new()));
    let end_for_handler = end.clone();
    tree.handler_table()
        .on_ime_composition_end(root_id, move |text| {
            *end_for_handler.borrow_mut() = text.to_string();
        });

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 10.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(
        tree.dispatch_event(&SystemEvent::ImeCompositionStart),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::ImeCompositionUpdate {
            text: "zh".to_string(),
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::ImeCompositionEnd {
            text: "中".to_string(),
        }),
        EventResult::Handled
    );

    assert!(*started.borrow());
    assert_eq!(&*update.borrow(), "zh");
    assert_eq!(&*end.borrow(), "中");
}

#[test]
fn clipboard_events_emit_semantic_events_for_focused_target() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    let copied = Rc::new(RefCell::new(false));
    let copied_for_handler = copied.clone();
    tree.handler_table()
        .on_copy(root_id, move || *copied_for_handler.borrow_mut() = true);

    let cut = Rc::new(RefCell::new(false));
    let cut_for_handler = cut.clone();
    tree.handler_table()
        .on_cut(root_id, move || *cut_for_handler.borrow_mut() = true);

    let pasted = Rc::new(RefCell::new(String::new()));
    let pasted_for_handler = pasted.clone();
    tree.handler_table().on_paste(root_id, move |text| {
        *pasted_for_handler.borrow_mut() = text.to_string()
    });

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 10.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Copy),
        EventResult::Handled
    );
    assert_eq!(tree.dispatch_event(&SystemEvent::Cut), EventResult::Handled);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::Paste {
            text: "from clipboard".to_string(),
        }),
        EventResult::Handled
    );

    assert!(*copied.borrow());
    assert!(*cut.borrow());
    assert_eq!(&*pasted.borrow(), "from clipboard");
}

#[test]
fn file_drop_emits_semantic_event_for_hit_target() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    let files = Rc::new(RefCell::new(Vec::<String>::new()));
    let files_for_handler = files.clone();
    tree.handler_table()
        .on(root_id, crate::ui::SemanticKind::FileDrop, move |event| {
            if let Some((payload, _position)) = event.file_drop_payload() {
                *files_for_handler.borrow_mut() = payload.to_vec();
            }
        });

    tree.dispatch_event(&SystemEvent::FileDrop {
        files: vec!["a.txt".to_string(), "b.txt".to_string()],
        position: Point::new(20.0, 20.0),
    });

    assert_eq!(
        &*files.borrow(),
        &vec!["a.txt".to_string(), "b.txt".to_string()]
    );
}

#[test]
fn file_drop_targets_overlay_owner_before_main_tree() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let owner = tree.add_child(root_id, Box::new(SpyWidget::new(20.0, 20.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(owner)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 20.0, 20.0));
    tree.overlay_stack_mut().push_entry(
        OverlayEntry::new(owner, OverlayKind::Popover)
            .bounds(Rect::new(50.0, 50.0, 80.0, 80.0))
            .z_index(900)
            .managed(true),
    );

    let files = Rc::new(RefCell::new(Vec::<String>::new()));
    let files_for_handler = files.clone();
    tree.handler_table()
        .on(owner, crate::ui::SemanticKind::FileDrop, move |event| {
            if let Some((payload, _position)) = event.file_drop_payload() {
                *files_for_handler.borrow_mut() = payload.to_vec();
            }
        });

    tree.dispatch_event(&SystemEvent::FileDrop {
        files: vec!["overlay.txt".to_string()],
        position: Point::new(70.0, 70.0),
    });

    assert_eq!(&*files.borrow(), &vec!["overlay.txt".to_string()]);
}

#[test]
fn typography_drag_selection_extends_to_sibling_above() {
    use crate::ui::Typography;

    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(400.0, 200.0, vec![])));
    let h2 = tree.add_child(
        root,
        Box::new(Typography::heading("Heading 2 — 二级标题", 2)),
    );
    let h3 = tree.add_child(
        root,
        Box::new(Typography::heading("Heading 3 — 三级标题", 3)),
    );
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 400.0, 200.0));
    tree.get_mut(h2)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 400.0, 40.0));
    tree.get_mut(h3)
        .unwrap()
        .set_frame(Rect::new(0.0, 40.0, 400.0, 40.0));

    // 在 Heading 3 按下开始拖选
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(20.0, 55.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().interaction.pressed_component(), Some(h3));
    assert!(tree
        .get(h3)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Typography>()
        .unwrap()
        .is_cross_text_dragging());

    // 不松手，拖到上方 Heading 2
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerMove {
            pos: Point::new(20.0, 20.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );

    let h2_sel = tree
        .get(h2)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Typography>()
        .unwrap()
        .selected_text();
    assert!(
        h2_sel.is_some(),
        "拖到上方行后 Heading 2 应进入选区，got {h2_sel:?}"
    );
    assert!(h2_sel.as_deref().unwrap().contains("Heading 2"));
}

#[test]
fn typography_drag_selection_extends_through_middle_sibling() {
    use crate::ui::Typography;

    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(400.0, 240.0, vec![])));
    let h1 = tree.add_child(root, Box::new(Typography::heading("Heading 1", 1)));
    let h2 = tree.add_child(root, Box::new(Typography::heading("Heading 2", 2)));
    let h3 = tree.add_child(root, Box::new(Typography::heading("Heading 3", 3)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 400.0, 240.0));
    tree.get_mut(h1)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 400.0, 40.0));
    tree.get_mut(h2)
        .unwrap()
        .set_frame(Rect::new(0.0, 40.0, 400.0, 40.0));
    tree.get_mut(h3)
        .unwrap()
        .set_frame(Rect::new(0.0, 80.0, 400.0, 40.0));

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 95.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(10.0, 15.0),
        mods: KeyMod::NONE,
    });

    let selected = |id| {
        tree.get(id)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<Typography>()
            .unwrap()
            .selected_text()
    };
    assert!(selected(h1).is_some(), "最上行应在选区");
    assert!(selected(h2).is_some(), "中间行应整行选中");
    assert_eq!(selected(h2).as_deref(), Some("Heading 2"));
}

#[test]
fn typography_cross_selection_copy_aggregates_sibling_lines() {
    use crate::native::test_harness::FakeClipboard;
    use crate::ui::clipboard;
    use crate::ui::Typography;

    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(400.0, 240.0, vec![])));
    let h1 = tree.add_child(
        root,
        Box::new(Typography::heading("Heading 1 — 一级标题", 1)),
    );
    let h2 = tree.add_child(
        root,
        Box::new(Typography::heading("Heading 2 — 二级标题", 2)),
    );
    let h3 = tree.add_child(
        root,
        Box::new(Typography::heading("Heading 3 — 三级标题", 3)),
    );
    let body = tree.add_child(
        root,
        Box::new(Typography::paragraph(
            "正文段落：UIX Rust 原生 UI — 响应式布局、主题",
        )),
    );
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 400.0, 240.0));
    tree.get_mut(h1)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 400.0, 40.0));
    tree.get_mut(h2)
        .unwrap()
        .set_frame(Rect::new(0.0, 40.0, 400.0, 40.0));
    tree.get_mut(h3)
        .unwrap()
        .set_frame(Rect::new(0.0, 80.0, 400.0, 40.0));
    tree.get_mut(body)
        .unwrap()
        .set_frame(Rect::new(0.0, 120.0, 400.0, 40.0));

    // 在正文按下并拖到 Heading 1：焦点留在起点，上行进入跨节点选区
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 135.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(10.0, 15.0),
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::PointerUp {
        pos: Point::new(10.0, 15.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(tree.managers().focus.focused_component(), Some(body));

    // 无 glyph 缓存时 hit 落在 char 0，锚点行可能未入选；补齐与 GUI 一致的整行选区。
    let select_all = |id: ComponentId| {
        let node = tree.get(id).unwrap();
        let t = node
            .component()
            .as_any()
            .downcast_ref::<Typography>()
            .unwrap();
        let len = t.cross_text_len();
        t.set_cross_text_range(Some((0, len)));
    };
    select_all(h1);
    select_all(h2);
    select_all(h3);
    select_all(body);

    let aggregated = tree
        .aggregate_cross_text_selection(body)
        .expect("跨节点选区应可聚合");
    assert_eq!(
        aggregated,
        "Heading 1 — 一级标题\nHeading 2 — 二级标题\nHeading 3 — 三级标题\n正文段落：UIX Rust 原生 UI — 响应式布局、主题"
    );

    // 焦点在起点（正文）时 Ctrl+C 须写入聚合文本，而非仅一行
    let mut clipboard = FakeClipboard::new();
    let copy_result = clipboard::with_clipboard(&mut clipboard, || {
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::C,
            mods: KeyMod::CTRL,
        })
    });
    assert_eq!(copy_result, EventResult::Handled);
    assert_eq!(clipboard.last_set_text(), Some(aggregated.as_str()));

    // 菜单/系统 Copy 事件同样走聚合路径
    let mut clipboard2 = FakeClipboard::new();
    let copy_result =
        clipboard::with_clipboard(&mut clipboard2, || tree.dispatch_event(&SystemEvent::Copy));
    assert_eq!(copy_result, EventResult::Handled);
    assert_eq!(clipboard2.last_set_text(), Some(aggregated.as_str()));
}

#[test]
fn rendered_typography_drag_selection_copies_the_actual_cross_node_range() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
    use crate::draw::painting::PaintContext;
    use crate::draw::spatial::Orientation;
    use crate::native::test_harness::FakeClipboard;
    use crate::ui::clipboard;
    use crate::ui::Typography;

    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(400.0, 160.0, vec![])));
    let h1 = tree.add_child(root, Box::new(Typography::heading("Heading 1", 1)));
    let h2 = tree.add_child(root, Box::new(Typography::heading("Heading 2", 2)));
    let h3 = tree.add_child(root, Box::new(Typography::heading("Heading 3", 3)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 400.0, 160.0));
    for (id, y) in [(h1, 0.0), (h2, 40.0), (h3, 80.0)] {
        tree.get_mut(id)
            .unwrap()
            .set_frame(Rect::new(0.0, y, 400.0, 40.0));
    }

    // Populate the same glyph hit-test cache used by the live renderer before
    // sending pointer events; this makes the selection range fully observable.
    let mut canvas = SharedRasterizer::new(PixelSurface::new(400, 160));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            font,
            &fonts,
            &images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            400,
            160,
        );
        for id in [h1, h2, h3] {
            let node = tree.get(id).expect("Typography node");
            let typography = node
                .component()
                .as_any()
                .downcast_ref::<Typography>()
                .expect("Typography component");
            WidgetRender::render(typography, node.frame(), &mut ctx, &tree);
        }
    }

    // Drag from after Heading 3 to just left of Heading 1. The tree's
    // nearest-line fallback clamps that endpoint to Heading 1 char 0.
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(399.0, 100.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerMove {
            pos: Point::new(-1.0, 20.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: Point::new(0.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );

    let selected = |id| {
        tree.get(id)
            .expect("Typography node")
            .component()
            .as_any()
            .downcast_ref::<Typography>()
            .expect("Typography component")
            .selected_text()
    };
    assert_eq!(selected(h1).as_deref(), Some("Heading 1"));
    assert_eq!(selected(h2).as_deref(), Some("Heading 2"));
    assert_eq!(selected(h3).as_deref(), Some("Heading 3"));

    let mut clipboard = FakeClipboard::new();
    let copy_result = clipboard::with_clipboard(&mut clipboard, || {
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::C,
            mods: KeyMod::CTRL,
        })
    });
    assert_eq!(copy_result, EventResult::Handled);
    assert_eq!(
        clipboard.last_set_text(),
        Some("Heading 1\nHeading 2\nHeading 3")
    );
}
