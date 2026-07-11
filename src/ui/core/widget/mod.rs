pub use crate::core::ComponentId;
pub use crate::core::Point;
use crate::core::{Constraints, Rect, Size};
use crate::draw::compositor::PicturePolicy;
use crate::draw::spatial::{Ray3D, SpatialContext};
pub use crate::native::traits::input::{KeyCode, KeyMod, MouseButton};
use crate::ui::event::{HandlerRegistration, HandlerSignature};
pub use crate::ui::event::{SystemEvent, SystemEventKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    Handled,
    NotHandled,
    Bubbled,
}

pub(crate) type WidgetId = ComponentId;

// 重新导出 api 中的 trait 定义
pub use crate::ui::traits::{
    EventHandler, IntoWidgetNode, WidgetCapabilities, WidgetComponent, WidgetLayout,
    WidgetLifecycle, WidgetRender, WidgetTextInput,
};

pub struct WidgetNode {
    pub widget: Box<dyn WidgetComponent>,
    pub children: Vec<WidgetNode>,
    pub z_index: i32,
    pub key: Option<Box<str>>,
    pub tab_idx: i32,
    pub handlers: Vec<HandlerRegistration>,
}

impl WidgetNode {
    pub fn new(widget: Box<dyn WidgetComponent>, children: Vec<WidgetNode>) -> Self {
        Self {
            widget,
            children,
            z_index: 0,
            key: None,
            tab_idx: 0,
            handlers: Vec::new(),
        }
    }
    pub fn key(mut self, k: &str) -> Self {
        self.key = Some(k.into());
        self
    }
    pub fn leaf(widget: Box<dyn WidgetComponent>) -> Self {
        Self {
            widget,
            children: vec![],
            z_index: 0,
            key: None,
            tab_idx: 0,
            handlers: Vec::new(),
        }
    }
    pub fn z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }
    /// 设置 Tab 键导航顺序索引（> 0 表示可通过 Tab 获取焦点）。
    pub fn tab_index(mut self, idx: i32) -> Self {
        self.tab_idx = idx;
        self
    }
    pub fn on_semantic(mut self, registration: HandlerRegistration) -> Self {
        self.handlers.push(registration);
        self
    }
    pub fn on_semantic_capture<T>(
        mut self,
        mut registration: HandlerRegistration,
        state: &crate::ui::state::State<T>,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        registration = registration.with_state_capture(state);
        self.handlers.push(registration);
        self
    }
    pub fn on_semantic_computed_capture<T>(
        mut self,
        mut registration: HandlerRegistration,
        computed: &crate::ui::state::Computed<T>,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        registration = registration.with_computed_capture(computed);
        self.handlers.push(registration);
        self
    }
    pub fn on_semantic_window_capture(
        mut self,
        mut registration: HandlerRegistration,
        window_id: crate::core::WindowId,
    ) -> Self {
        registration = registration.with_window_capture(window_id);
        self.handlers.push(registration);
        self
    }
    pub fn with_handlers(mut self, handlers: Vec<HandlerRegistration>) -> Self {
        self.handlers = handlers;
        self
    }
}

pub trait WidgetCore {
    fn id(&self) -> ComponentId;
    fn set_id(&mut self, id: ComponentId);
    fn parent(&self) -> Option<ComponentId>;
    fn set_parent(&mut self, id: Option<ComponentId>);
    fn children(&self) -> &[ComponentId];
    fn children_mut(&mut self) -> &mut Vec<ComponentId>;
    fn frame(&self) -> Rect;
    fn set_frame(&mut self, rect: Rect);
    fn visible(&self) -> bool;
    fn set_visible(&mut self, v: bool);
    fn opacity(&self) -> f32;
    fn set_opacity(&mut self, v: f32);
    fn z_index(&self) -> i32;
    fn set_z_index(&mut self, v: i32);
    /// Tab 键导航顺序索引。0 = 不可通过 Tab 导航获取焦点，> 0 = 可聚焦。
    fn tab_index(&self) -> i32;
    fn set_tab_index(&mut self, v: i32);
}

pub struct BoxedWidget {
    component: Box<dyn WidgetComponent>,
    caps: WidgetCapabilities,
    id: WidgetId,
    parent: Option<WidgetId>,
    children: Vec<WidgetId>,
    key: Option<Box<str>>,
    frame: Rect,
    visible: bool,
    attached: bool,
    mounted: bool,
    active: bool,
    destroyed: bool,
    widget_opacity: f32,
    z: i32,
    /// Tab 键导航顺序（0=不可通过 Tab 导航聚焦）。
    tab_idx: i32,
    handler_signatures: Vec<HandlerSignature>,
}

impl BoxedWidget {
    pub fn new(mut component: Box<dyn WidgetComponent>) -> Self {
        let caps = component.capabilities();
        if let Some(lifecycle) = component.as_lifecycle_mut() {
            lifecycle.on_init();
        }
        Self {
            component,
            caps,
            id: WidgetId::default(),
            parent: None,
            children: Vec::new(),
            key: None,
            frame: Rect::zero(),
            visible: true,
            attached: false,
            mounted: false,
            active: false,
            destroyed: false,
            widget_opacity: 1.0,
            z: 0,
            tab_idx: 0,
            handler_signatures: Vec::new(),
        }
    }
    pub fn component(&self) -> &dyn WidgetComponent {
        &*self.component
    }
    pub fn component_mut(&mut self) -> &mut dyn WidgetComponent {
        &mut *self.component
    }
    pub(crate) fn replace_component(&mut self, mut component: Box<dyn WidgetComponent>) {
        let was_attached = self.attached;
        let was_mounted = self.mounted;
        let was_active = self.active;

        if was_active {
            self.on_inactive();
        }
        if was_mounted {
            self.on_unmount();
        }
        if was_attached {
            self.on_detach();
        }
        if !self.destroyed {
            self.on_destroy();
        }

        if let Some(lifecycle) = component.as_lifecycle_mut() {
            lifecycle.on_init();
        }
        self.caps = component.capabilities();
        self.component = component;
        self.destroyed = false;

        if was_attached {
            self.on_attach();
        }
        if was_mounted {
            self.on_mount();
        }
        if was_active {
            self.on_active();
        }
    }
    pub fn capabilities(&self) -> WidgetCapabilities {
        self.caps
    }
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }
    pub(crate) fn set_key(&mut self, key: Option<Box<str>>) {
        self.key = key;
    }
    pub(crate) fn handler_signatures(&self) -> &[HandlerSignature] {
        &self.handler_signatures
    }
    pub(crate) fn set_handler_signatures(&mut self, signatures: Vec<HandlerSignature>) {
        self.handler_signatures = signatures;
    }

    pub fn as_render(&self) -> Option<&dyn WidgetRender> {
        self.component.as_render()
    }
    pub fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        self.component.as_render_mut()
    }
    pub fn as_event(&self) -> Option<&dyn EventHandler> {
        self.component.as_event()
    }
    pub fn as_event_mut(&mut self) -> Option<&mut dyn EventHandler> {
        self.component.as_event_mut()
    }
    pub fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        self.component.as_layout()
    }

    pub fn as_text_input(&self) -> Option<&dyn WidgetTextInput> {
        self.component.as_text_input()
    }

    pub fn as_lifecycle(&self) -> Option<&dyn WidgetLifecycle> {
        self.component.as_lifecycle()
    }

    pub fn as_lifecycle_mut(&mut self) -> Option<&mut dyn WidgetLifecycle> {
        self.component.as_lifecycle_mut()
    }

    // ═══ 便捷分发方法 ═══

    pub fn measure(&self, constraints: Constraints) -> Size {
        self.component()
            .as_layout()
            .map(|l| l.measure(constraints))
            .unwrap_or_default()
    }

    pub fn flex_grow(&self) -> f32 {
        self.component()
            .as_layout()
            .map(|l| l.flex_grow())
            .unwrap_or(0.0)
    }
    pub fn flex_shrink(&self) -> f32 {
        self.component()
            .as_layout()
            .map(|l| l.flex_shrink())
            .unwrap_or(0.0)
    }
    pub fn layout_children(
        &self,
        frame: Rect,
        children: &[ComponentId],
        tree: &WidgetTree,
    ) -> Vec<(ComponentId, Rect)> {
        self.component()
            .as_layout()
            .map(|layout| {
                let measured = layout.measure_children(frame, children, tree);
                layout.layout_children(frame, &measured, tree)
            })
            .unwrap_or_default()
    }
    pub fn children_clip(&self, frame: Rect) -> Option<Rect> {
        self.component()
            .as_render()
            .and_then(|r| r.children_clip(frame))
    }
    pub fn dirty_rect(&self, frame: Rect) -> Rect {
        self.component()
            .as_render()
            .map(|r| r.dirty_rect(frame))
            .unwrap_or(frame)
    }
    pub fn overlay_entry(&self, id: ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.component()
            .as_render()
            .and_then(|r| r.overlay_entry(id, frame))
    }
    pub fn scroll_delta(&self, frame: Rect) -> Option<(f32, f32)> {
        self.component()
            .as_event()
            .and_then(|e| e.scroll_delta(frame))
    }
    pub fn scroll_delta_for_dirty(&self) -> Option<(f32, f32)> {
        self.component()
            .as_event()
            .and_then(|e| e.scroll_delta_for_dirty())
    }
    pub fn viewport_scroll_offset(&self) -> Option<(f32, f32)> {
        self.component()
            .as_event()
            .and_then(|e| e.viewport_scroll_offset())
    }
    pub fn active_timer(&self) -> Option<(u64, std::time::Duration)> {
        self.component().as_event().and_then(|e| e.active_timer())
    }
    pub fn wants_capture_phase(&self) -> bool {
        self.component()
            .as_event()
            .is_some_and(|e| e.wants_capture_phase())
    }
    pub fn wants_continuous_pointer_move(&self) -> bool {
        self.component()
            .as_event()
            .is_some_and(|e| e.wants_continuous_pointer_move())
    }
    pub fn hit_test_frame(&self, actual_frame: Rect) -> Rect {
        self.component()
            .as_event()
            .map(|e| e.hit_test_frame(actual_frame))
            .unwrap_or(actual_frame)
    }
    pub fn hit_test_3d(&self, ray: &Ray3D, spatial: &SpatialContext, frame: Rect) -> bool {
        self.component()
            .as_event()
            .map(|e| e.hit_test_3d(ray, spatial, frame))
            .unwrap_or(false)
    }
    pub fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        self.component_mut()
            .as_event_mut()
            .map(|e| e.on_event(event))
            .unwrap_or(EventResult::NotHandled)
    }
    pub fn semantic_event(
        &self,
        id: ComponentId,
        event: &SystemEvent,
    ) -> Option<crate::ui::event::SemanticEvent> {
        self.component()
            .as_event()
            .and_then(|e| e.semantic_event(id, event))
    }
    pub fn is_focusable(&self) -> bool {
        self.tab_idx > 0 && self.visible && self.component.as_event().is_some()
    }

    pub fn attached(&self) -> bool {
        self.attached
    }
    pub(crate) fn set_attached(&mut self, attached: bool) {
        self.attached = attached;
    }
    pub fn mounted(&self) -> bool {
        self.mounted
    }
    pub(crate) fn set_mounted(&mut self, mounted: bool) {
        self.mounted = mounted;
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub(crate) fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    pub fn destroyed(&self) -> bool {
        self.destroyed
    }
    pub(crate) fn set_destroyed(&mut self, destroyed: bool) {
        self.destroyed = destroyed;
    }
    pub fn uses_palette(&self) -> bool {
        self.component()
            .as_render()
            .is_some_and(|render| render.uses_palette())
    }
    pub fn picture_policy(&self) -> PicturePolicy {
        self.component().picture_policy()
    }
    pub fn has_dynamic_content(&self) -> bool {
        self.component().has_dynamic_content()
    }
    pub fn has_interactive_state(&self) -> bool {
        self.caps.contains(WidgetCapabilities::EVENT)
    }
    pub fn on_attach(&mut self) {
        if let Some(l) = self.component_mut().as_lifecycle_mut() {
            l.on_attach();
        }
    }
    pub fn on_mount(&mut self) {
        if let Some(l) = self.component_mut().as_lifecycle_mut() {
            l.on_mount();
        }
    }
    pub fn on_active(&mut self) {
        if let Some(l) = self.component_mut().as_lifecycle_mut() {
            l.on_active();
        }
    }
    pub fn on_inactive(&mut self) {
        if let Some(l) = self.component_mut().as_lifecycle_mut() {
            l.on_inactive();
        }
    }
    pub fn on_theme_changed(&mut self) {
        if let Some(l) = self.component_mut().as_lifecycle_mut() {
            l.on_theme_changed();
        }
    }
    pub fn on_unmount(&mut self) {
        if let Some(l) = self.component_mut().as_lifecycle_mut() {
            l.on_unmount();
        }
    }
    pub fn on_detach(&mut self) {
        if let Some(l) = self.component_mut().as_lifecycle_mut() {
            l.on_detach();
        }
    }
    pub fn on_destroy(&mut self) {
        if let Some(l) = self.component_mut().as_lifecycle_mut() {
            l.on_destroy();
        }
    }
    pub fn render(
        &self,
        frame: Rect,
        ctx: &mut crate::draw::painting::PaintContext,
        tree: &WidgetTree,
    ) {
        if let Some(r) = self.component().as_render() {
            r.render(frame, ctx, tree);
        }
    }
}

impl WidgetCore for BoxedWidget {
    fn id(&self) -> ComponentId {
        self.id
    }
    fn set_id(&mut self, id: ComponentId) {
        self.id = id;
    }
    fn parent(&self) -> Option<ComponentId> {
        self.parent
    }
    fn set_parent(&mut self, id: Option<ComponentId>) {
        self.parent = id;
    }
    fn children(&self) -> &[ComponentId] {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<ComponentId> {
        &mut self.children
    }
    fn frame(&self) -> Rect {
        self.frame
    }
    fn set_frame(&mut self, rect: Rect) {
        self.frame = rect;
    }
    fn visible(&self) -> bool {
        self.visible
    }
    fn set_visible(&mut self, v: bool) {
        self.visible = v;
    }
    fn opacity(&self) -> f32 {
        self.widget_opacity
    }
    fn set_opacity(&mut self, v: f32) {
        self.widget_opacity = v;
    }
    fn z_index(&self) -> i32 {
        self.z
    }
    fn set_z_index(&mut self, v: i32) {
        self.z = v;
    }
    fn tab_index(&self) -> i32 {
        self.tab_idx
    }
    fn set_tab_index(&mut self, v: i32) {
        self.tab_idx = v;
    }
}

mod text_selection;
mod tree_core;
mod tree_dirty;
mod tree_events;
pub use tree_core::WidgetTree;
