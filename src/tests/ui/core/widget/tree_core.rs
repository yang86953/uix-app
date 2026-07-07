// WidgetTree unit tests.
// Split out of `tree_core.rs` to keep implementation files manageable.
use crate::core::{ComponentId, Constraints, EdgeInsets, Point, Size};
use crate::draw::Color;
use crate::native::traits::input::{KeyCode, KeyMod, MouseButton};
use crate::ui::core::widget::tree_core::*;
use crate::ui::layout::engine::{child_from_tree, child_from_tree_with_constraints};
use crate::ui::managers::StyleManager;
use crate::ui::{
    AppState, Button, Container, Drawer, Grid, Label, Modal, OverlayEntry, OverlayKind, QRCode,
    SnapshotFields, Style, TextManager, Tooltip,
};
use std::cell::RefCell;
use std::rc::Rc;

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

    fn layout_children(
        &self,
        frame: Rect,
        children: &[ComponentId],
        tree: &WidgetTree,
    ) -> Vec<(ComponentId, Rect)> {
        children
            .iter()
            .copied()
            .map(|id| {
                let size = tree
                    .get(id)
                    .map(|node| node.measure(Constraints::unconstrained()))
                    .unwrap_or_default();
                (
                    id,
                    Rect::new(frame.x, frame.y + self.child_y, size.w, size.h),
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

    fn layout_children(
        &self,
        frame: Rect,
        children: &[ComponentId],
        tree: &WidgetTree,
    ) -> Vec<(ComponentId, Rect)> {
        children
            .iter()
            .copied()
            .map(|id| {
                let size = tree
                    .get(id)
                    .map(|node| node.measure(Constraints::unconstrained()))
                    .unwrap_or_default();
                (
                    id,
                    Rect::new(frame.x, frame.y + self.child_y, size.w, size.h),
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
        Rect::new(0.0, 0.0, 40.0, 24.0)
    );
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
    let root = tree.set_root(Box::new(Container::new().size(200.0, 100.0)));
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
    let root = tree.set_root(Box::new(Container::new().size(200.0, 100.0)));
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
        Container::new().size(200.0, 100.0).overflow_content(),
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
    tree.add_child(root, Box::new(Container::new().size(10.0, 10.0)));
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
    assert_eq!(tree.traverse(), vec![root, a, c, b]);
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
    tree.reset_dirty();

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
fn focus_trap_tab_navigation_stays_inside_top_overlay_owner_subtree() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(300.0, 200.0, vec![])));
    let outside = tree.add_child(root, Box::new(SpyWidget::new(20.0, 20.0).with_tab_index(1)));
    let modal = tree.add_child(root, Box::new(Modal::new("Dialog").show().overlay(true)));
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
    let modal = tree.add_child(root, Box::new(Modal::new("Dialog").show().overlay(true)));
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
    tree.reset_dirty();

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
    let modal = tree.add_child(root_id, Box::new(Modal::new("Dialog").show().overlay(true)));
    let palette_id = tree.add_child(
        modal,
        Box::new(LifecycleProbe::new(20.0, 20.0, palette_events.clone())),
    );
    tree.layout();
    tree.reset_dirty();

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
    tree.reset_dirty();

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
    let modal = tree.add_child(root_id, Box::new(Modal::new("Dialog").show().overlay(true)));
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
    let modal = tree.add_child(root_id, Box::new(Modal::new("Dialog").show().overlay(true)));
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
    let modal = tree.add_child(root_id, Box::new(Modal::new("Dialog").show().overlay(true)));
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
    tree.reset_dirty();
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
