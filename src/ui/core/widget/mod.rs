pub use crate::core::ComponentId;
pub use crate::core::Point;
use crate::core::{Constraints, Rect, Size};
use crate::draw::compositor::PicturePolicy;
use crate::draw::spatial::{Ray3D, SpatialContext};
pub use crate::native::traits::input::{KeyCode, KeyMod, MouseButton};
use crate::ui::accessibility_override::AccessibilityOverride;
use crate::ui::component_snapshot::{
    AccessibilitySnapshot, ComponentConfigSnapshot, SnapshotFields,
};
pub use crate::ui::event::SystemEvent;
use crate::ui::event::{HandlerRegistration, HandlerSignature};
use crate::ui::focus_handle::FocusHandle;
use crate::ui::foundation::provider_context::{
    current_provider_context, with_provider_context, ProviderContext,
};
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::system_event_handler::SystemEventHandlerRegistration;
use crate::ui::view_transform::ViewTransform;

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
    pub(crate) provider_context: ProviderContext,
    pub(crate) visible: bool,
    pub(crate) visual_transform: ViewTransform,
    pub(crate) enter_animation: Option<crate::ui::animation::AnimationConfig>,
    pub(crate) enter_deadline: Option<std::time::Instant>,
    pub(crate) leave_animation: Option<crate::ui::animation::AnimationConfig>,
    pub z_index: i32,
    pub key: Option<Box<str>>,
    pub automation_id: Option<Box<str>>,
    pub tab_idx: i32,
    pub(crate) focus_handle: Option<FocusHandle>,
    pub(crate) accessibility_override: Option<AccessibilityOverride>,
    pub handlers: Vec<HandlerRegistration>,
    pub(crate) system_event_handlers: Vec<SystemEventHandlerRegistration>,
    pub(crate) render_handlers: Vec<RenderHandlerRegistration>,
}

impl WidgetNode {
    pub fn new(widget: Box<dyn WidgetComponent>, children: Vec<WidgetNode>) -> Self {
        Self {
            widget,
            children,
            provider_context: current_provider_context(),
            visible: true,
            visual_transform: ViewTransform::default(),
            enter_animation: None,
            enter_deadline: None,
            leave_animation: None,
            z_index: 0,
            key: None,
            automation_id: None,
            tab_idx: 0,
            focus_handle: None,
            accessibility_override: None,
            handlers: Vec::new(),
            system_event_handlers: Vec::new(),
            render_handlers: Vec::new(),
        }
    }
    pub fn key(mut self, k: &str) -> Self {
        self.key = Some(k.into());
        self
    }
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(id.into());
        self
    }
    pub fn leaf(widget: Box<dyn WidgetComponent>) -> Self {
        Self {
            widget,
            children: vec![],
            provider_context: current_provider_context(),
            visible: true,
            visual_transform: ViewTransform::default(),
            enter_animation: None,
            enter_deadline: None,
            leave_animation: None,
            z_index: 0,
            key: None,
            automation_id: None,
            tab_idx: 0,
            focus_handle: None,
            accessibility_override: None,
            handlers: Vec::new(),
            system_event_handlers: Vec::new(),
            render_handlers: Vec::new(),
        }
    }
    pub fn z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }
    pub(crate) fn with_visibility(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
    pub(crate) fn with_visual_transform(mut self, transform: ViewTransform) -> Self {
        self.visual_transform = transform;
        self
    }
    pub(crate) fn with_enter_animation(
        mut self,
        animation: crate::ui::animation::AnimationConfig,
        deadline: Option<std::time::Instant>,
    ) -> Self {
        self.enter_animation = Some(animation);
        self.enter_deadline = deadline;
        self
    }
    pub(crate) fn with_leave_animation(
        mut self,
        animation: crate::ui::animation::AnimationConfig,
    ) -> Self {
        self.leave_animation = Some(animation);
        self
    }
    /// 设置 Tab 键导航顺序索引（> 0 表示可通过 Tab 获取焦点）。
    pub fn tab_index(mut self, idx: i32) -> Self {
        self.tab_idx = idx;
        self
    }
    pub(crate) fn with_focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus_handle = Some(handle);
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
    pub(crate) fn with_accessibility_override(
        mut self,
        accessibility_override: AccessibilityOverride,
    ) -> Self {
        self.accessibility_override = Some(accessibility_override);
        self
    }
    pub(crate) fn with_system_event_handlers(
        mut self,
        handlers: Vec<SystemEventHandlerRegistration>,
    ) -> Self {
        self.system_event_handlers = handlers;
        self
    }
    pub(crate) fn with_render_handlers(mut self, handlers: Vec<RenderHandlerRegistration>) -> Self {
        self.render_handlers = handlers;
        self
    }

    pub(crate) fn with_provider_context(mut self, context: ProviderContext) -> Self {
        self.provider_context = context;
        self
    }
}

pub trait WidgetCore {
    #[cfg(any(test, feature = "test-harness"))]
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
    fn z_index(&self) -> i32;
    fn set_z_index(&mut self, v: i32);
    /// Tab 键导航顺序索引。0 = 不可通过 Tab 导航获取焦点，> 0 = 可聚焦。
    fn tab_index(&self) -> i32;
    fn set_tab_index(&mut self, v: i32);
}

pub struct BoxedWidget {
    component: Box<dyn WidgetComponent>,
    caps: WidgetCapabilities,
    provider_context: ProviderContext,
    id: WidgetId,
    parent: Option<WidgetId>,
    children: Vec<WidgetId>,
    key: Option<Box<str>>,
    automation_id: Option<Box<str>>,
    frame: Rect,
    visible: bool,
    visual_transform: ViewTransform,
    view_transition: Option<crate::ui::animation::TransitionPlayer>,
    view_transition_deadline: Option<std::time::Instant>,
    leave_animation: Option<crate::ui::animation::AnimationConfig>,
    pending_removal: bool,
    attached: bool,
    mounted: bool,
    active: bool,
    destroyed: bool,
    z: i32,
    /// Tab 键导航顺序（0=不可通过 Tab 导航聚焦）。
    tab_idx: i32,
    handler_signatures: Vec<HandlerSignature>,
    system_event_handlers: Vec<SystemEventHandlerRegistration>,
    accessibility_override: Option<AccessibilityOverride>,
}

impl BoxedWidget {
    pub fn new(component: Box<dyn WidgetComponent>) -> Self {
        Self::new_with_context(component, current_provider_context())
    }

    pub(crate) fn new_with_context(
        mut component: Box<dyn WidgetComponent>,
        provider_context: ProviderContext,
    ) -> Self {
        let caps = with_provider_context(&provider_context, || {
            let caps = component.capabilities();
            if let Some(lifecycle) = component.as_lifecycle_mut() {
                lifecycle.on_init();
            }
            caps
        });
        Self {
            component,
            caps,
            provider_context,
            id: WidgetId::default(),
            parent: None,
            children: Vec::new(),
            key: None,
            automation_id: None,
            frame: Rect::zero(),
            visible: true,
            visual_transform: ViewTransform::default(),
            view_transition: None,
            view_transition_deadline: None,
            leave_animation: None,
            pending_removal: false,
            attached: false,
            mounted: false,
            active: false,
            destroyed: false,
            z: 0,
            tab_idx: 0,
            handler_signatures: Vec::new(),
            system_event_handlers: Vec::new(),
            accessibility_override: None,
        }
    }
    pub fn component(&self) -> &dyn WidgetComponent {
        &*self.component
    }
    pub fn component_mut(&mut self) -> &mut dyn WidgetComponent {
        &mut *self.component
    }

    pub(crate) fn provider_context(&self) -> &ProviderContext {
        &self.provider_context
    }

    pub(crate) fn set_provider_context(&mut self, provider_context: ProviderContext) {
        self.provider_context = provider_context;
    }

    fn with_component_context<T>(&self, f: impl FnOnce(&dyn WidgetComponent) -> T) -> T {
        with_provider_context(&self.provider_context, || f(&*self.component))
    }

    fn with_component_context_mut<T>(
        &mut self,
        f: impl FnOnce(&mut dyn WidgetComponent) -> T,
    ) -> T {
        let provider_context = self.provider_context.clone();
        with_provider_context(&provider_context, || f(&mut *self.component))
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

        self.caps = with_provider_context(&self.provider_context, || {
            let caps = component.capabilities();
            if let Some(lifecycle) = component.as_lifecycle_mut() {
                lifecycle.on_init();
            }
            caps
        });
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
    pub fn automation_id(&self) -> Option<&str> {
        self.automation_id.as_deref()
    }
    pub(crate) fn set_automation_id(&mut self, automation_id: Option<Box<str>>) {
        self.automation_id = automation_id;
    }
    pub(crate) fn handler_signatures(&self) -> &[HandlerSignature] {
        &self.handler_signatures
    }
    pub(crate) fn set_handler_signatures(&mut self, signatures: Vec<HandlerSignature>) {
        self.handler_signatures = signatures;
    }

    pub(crate) fn replace_system_event_handlers(
        &mut self,
        handlers: Vec<SystemEventHandlerRegistration>,
    ) {
        self.system_event_handlers = handlers;
    }

    pub(crate) fn set_accessibility_override(
        &mut self,
        accessibility_override: Option<AccessibilityOverride>,
    ) {
        self.accessibility_override = accessibility_override;
    }

    pub(crate) fn visual_transform(&self) -> ViewTransform {
        self.visual_transform
    }

    pub(crate) fn set_visual_transform(&mut self, transform: ViewTransform) {
        self.visual_transform = transform;
    }

    pub(crate) fn visual_transform_matrix(&self) -> crate::draw::Transform {
        let transition = self
            .view_transition
            .as_ref()
            .map(|player| ViewTransform {
                offset: player.offset,
                scale: player.scale,
            })
            .unwrap_or_default();
        self.visual_transform
            .combined(transition)
            .matrix(self.frame)
    }

    pub(crate) fn has_effective_visual_transform(&self) -> bool {
        !self.visual_transform_matrix().is_identity()
    }

    pub(crate) fn set_enter_animation(
        &mut self,
        animation: Option<crate::ui::animation::AnimationConfig>,
        deadline: Option<std::time::Instant>,
    ) {
        self.view_transition_deadline = None;
        self.view_transition = animation.and_then(|animation| {
            let mut player = crate::ui::animation::TransitionPlayer::new(animation);
            if animation.duration() <= 0.0 && deadline.is_none() {
                player.update(0.0);
            }
            if animation.duration() <= 0.0 && deadline.is_some() {
                player.hold_at_start();
            }
            self.view_transition_deadline = (!player.finished).then_some(deadline).flatten();
            (!player.finished).then_some(player)
        });
    }

    pub(crate) fn set_leave_animation(
        &mut self,
        animation: Option<crate::ui::animation::AnimationConfig>,
    ) {
        self.leave_animation = animation;
    }

    pub(crate) fn start_leave_transition(&mut self) -> bool {
        if self.pending_removal {
            return self.view_transition_active();
        }
        let Some(animation) = self.leave_animation else {
            return false;
        };
        self.view_transition_deadline = None;
        let (opacity, offset, scale) = self
            .view_transition
            .as_ref()
            .map_or((1.0, Point::new(0.0, 0.0), 1.0), |player| {
                (player.opacity_progress, player.offset, player.scale)
            });
        let mut player = crate::ui::animation::TransitionPlayer::new_from_current(
            animation, opacity, offset, scale,
        );
        if animation.duration() <= 0.0 {
            player.update(0.0);
        }
        if player.finished {
            return false;
        }
        self.view_transition = Some(player);
        self.pending_removal = true;
        true
    }

    pub(crate) fn cancel_pending_removal(&mut self) -> bool {
        if !self.pending_removal {
            return false;
        }
        self.pending_removal = false;
        self.view_transition = None;
        self.view_transition_deadline = None;
        true
    }

    pub(crate) fn pending_removal(&self) -> bool {
        self.pending_removal
    }

    pub(crate) fn view_transition_active(&self) -> bool {
        self.view_transition
            .as_ref()
            .is_some_and(|player| !player.finished)
    }

    pub(crate) fn view_transition_deadline(&self) -> Option<std::time::Instant> {
        self.view_transition_active()
            .then_some(self.view_transition_deadline)
            .flatten()
    }

    pub(crate) fn view_transition_opacity(&self) -> f32 {
        self.view_transition
            .as_ref()
            .map_or(1.0, |player| player.opacity_progress.clamp(0.0, 1.0))
    }

    pub(crate) fn advance_view_transition(
        &mut self,
        now: std::time::Instant,
        mut dt: f64,
    ) -> (bool, bool) {
        let Some(player) = self.view_transition.as_mut() else {
            return (false, false);
        };
        if let Some(deadline) = self.view_transition_deadline {
            if deadline > now {
                return (false, false);
            }
            self.view_transition_deadline = None;
            dt = now.saturating_duration_since(deadline).as_secs_f64();
        }
        player.update(dt);
        let still_active = !player.finished;
        let remove_now = !still_active && self.pending_removal;
        if !still_active {
            self.view_transition = None;
            self.view_transition_deadline = None;
        }
        (still_active, remove_now)
    }

    pub(crate) fn accessibility(&self) -> AccessibilitySnapshot {
        let base = self.with_component_context(|component| {
            let fields = component.snapshot_fields();
            Self::component_accessibility(component, &fields)
        });
        self.apply_accessibility_override(base)
    }

    fn component_accessibility(
        component: &dyn WidgetComponent,
        fields: &SnapshotFields,
    ) -> AccessibilitySnapshot {
        let mut accessibility = fields.accessibility();
        if let Some(label) = component
            .as_any()
            .downcast_ref::<crate::ui::view::combinators::DynamicLabel>()
        {
            let text = label.semantic_text();
            accessibility.role = crate::ui::AccessibilityRole::Text;
            accessibility.name = (!text.is_empty()).then_some(text);
        }
        accessibility
    }

    fn apply_accessibility_override(&self, base: AccessibilitySnapshot) -> AccessibilitySnapshot {
        self.accessibility_override
            .as_ref()
            .map(|accessibility_override| accessibility_override.apply(base.clone()))
            .unwrap_or(base)
    }

    pub(crate) fn component_snapshot(&self, id: ComponentId) -> ComponentConfigSnapshot {
        self.with_component_context(|component| {
            let fields = component.snapshot_fields();
            let accessibility = self
                .apply_accessibility_override(Self::component_accessibility(component, &fields));
            ComponentConfigSnapshot::from_component_fields(id, component, fields)
                .with_accessibility(accessibility)
        })
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
        self.with_component_context(|component| {
            component
                .as_layout()
                .map(|layout| layout.measure(constraints))
                .unwrap_or_default()
        })
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
    pub fn child_overflow_expands_parent(&self) -> bool {
        self.component()
            .as_layout()
            .map(|layout| layout.child_overflow_expands_parent())
            .unwrap_or(true)
    }
    pub fn layout_children(
        &self,
        frame: Rect,
        children: &[ComponentId],
        tree: &WidgetTree,
    ) -> Vec<(ComponentId, Rect)> {
        self.with_component_context(|component| {
            component
                .as_layout()
                .map(|layout| {
                    let measured = layout.measure_children(frame, children, tree);
                    layout.layout_children(frame, &measured, tree)
                })
                .unwrap_or_default()
        })
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
    pub fn scroll_composite_viewport(&self, frame: Rect) -> Rect {
        self.component()
            .as_event()
            .and_then(|e| e.scroll_composite_viewport(frame))
            .and_then(|viewport| viewport.intersect(&frame))
            .unwrap_or(frame)
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
        self.system_event_handlers
            .iter()
            .any(SystemEventHandlerRegistration::wants_capture_phase)
            || self
                .component()
                .as_event()
                .is_some_and(|e| e.wants_capture_phase())
    }
    pub fn wants_continuous_pointer_move(&self) -> bool {
        self.system_event_handlers
            .iter()
            .any(SystemEventHandlerRegistration::wants_continuous_pointer_move)
            || self
                .component()
                .as_event()
                .is_some_and(|e| e.wants_continuous_pointer_move())
    }
    pub fn hit_test_frame(&self, actual_frame: Rect) -> Rect {
        self.component()
            .as_event()
            .map(|e| e.hit_test_frame(actual_frame))
            .unwrap_or(actual_frame)
    }
    pub fn hit_test_children(&self) -> bool {
        self.component()
            .as_event()
            .is_none_or(|event| event.hit_test_children())
    }
    pub fn hit_test_3d(&self, ray: &Ray3D, spatial: &SpatialContext, frame: Rect) -> bool {
        self.component()
            .as_event()
            .map(|e| e.hit_test_3d(ray, spatial, frame))
            .unwrap_or(false)
    }
    pub fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        let mut bubbled = false;
        for handler in &mut self.system_event_handlers {
            match handler.handle(event) {
                None => {}
                Some(EventResult::Handled) => return EventResult::Handled,
                Some(EventResult::Bubbled) => bubbled = true,
                Some(EventResult::NotHandled) => {}
            }
        }
        let component_result = self.with_component_context_mut(|component| {
            component
                .as_event_mut()
                .map(|handler| handler.on_event(event))
                .unwrap_or(EventResult::NotHandled)
        });
        if component_result == EventResult::NotHandled && bubbled {
            EventResult::Bubbled
        } else {
            component_result
        }
    }
    pub(crate) fn on_focus_within(&mut self, focused: bool) -> EventResult {
        self.with_component_context_mut(|component| {
            component
                .as_event_mut()
                .map(|handler| handler.on_focus_within(focused))
                .unwrap_or(EventResult::NotHandled)
        })
    }
    pub(crate) fn take_window_action(&mut self) -> Option<crate::ui::event::WindowAction> {
        self.with_component_context_mut(|component| {
            component
                .as_event_mut()
                .and_then(|event| event.take_window_action())
        })
    }
    pub(crate) fn take_layout_request(&mut self) -> bool {
        self.with_component_context_mut(|component| {
            component
                .as_event_mut()
                .is_some_and(|event| event.take_layout_request())
        })
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
        (self.tab_idx > 0 || self.component().tab_index() > 0)
            && self.visible()
            && self.accepts_events()
            && self.is_interaction_enabled()
    }

    pub(crate) fn visibility_gate(&self) -> bool {
        self.visible
    }

    pub(crate) fn accepts_events(&self) -> bool {
        self.component.as_event().is_some() || !self.system_event_handlers.is_empty()
    }

    pub(crate) fn is_interaction_enabled(&self) -> bool {
        !self.accessibility().state.disabled
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
        self.caps.contains(WidgetCapabilities::EVENT) || !self.system_event_handlers.is_empty()
    }
    pub fn on_attach(&mut self) {
        self.with_component_context_mut(|component| {
            if let Some(lifecycle) = component.as_lifecycle_mut() {
                lifecycle.on_attach();
            }
        });
    }
    pub fn on_mount(&mut self) {
        self.with_component_context_mut(|component| {
            if let Some(lifecycle) = component.as_lifecycle_mut() {
                lifecycle.on_mount();
            }
        });
    }
    pub fn on_active(&mut self) {
        self.with_component_context_mut(|component| {
            if let Some(lifecycle) = component.as_lifecycle_mut() {
                lifecycle.on_active();
            }
        });
    }
    pub fn on_inactive(&mut self) {
        self.with_component_context_mut(|component| {
            if let Some(lifecycle) = component.as_lifecycle_mut() {
                lifecycle.on_inactive();
            }
        });
    }
    pub fn on_theme_changed(&mut self) {
        self.with_component_context_mut(|component| {
            if let Some(lifecycle) = component.as_lifecycle_mut() {
                lifecycle.on_theme_changed();
            }
        });
    }
    pub fn on_unmount(&mut self) {
        self.with_component_context_mut(|component| {
            if let Some(lifecycle) = component.as_lifecycle_mut() {
                lifecycle.on_unmount();
            }
        });
    }
    pub fn on_detach(&mut self) {
        self.with_component_context_mut(|component| {
            if let Some(lifecycle) = component.as_lifecycle_mut() {
                lifecycle.on_detach();
            }
        });
    }
    pub fn on_destroy(&mut self) {
        self.with_component_context_mut(|component| {
            if let Some(lifecycle) = component.as_lifecycle_mut() {
                lifecycle.on_destroy();
            }
        });
    }
    pub fn render(
        &self,
        frame: Rect,
        ctx: &mut crate::draw::painting::PaintContext,
        tree: &WidgetTree,
    ) {
        let config = &self.provider_context.config;
        let theme = config.theme.as_ref().map(|theme| theme.tokens_arc());
        let patch = config
            .component_tokens
            .get(self.component.as_any().type_id());
        let previous_scope = ctx.replace_token_scope(theme, patch);
        self.with_component_context(|component| {
            if let Some(render) = component.as_render() {
                render.render(frame, ctx, tree);
            }
        });
        ctx.restore_token_scope(previous_scope);
    }
}

impl WidgetCore for BoxedWidget {
    #[cfg(any(test, feature = "test-harness"))]
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
        self.visible && self.with_component_context(WidgetComponent::visible)
    }
    fn set_visible(&mut self, v: bool) {
        self.visible = v;
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

pub(crate) mod text_selection;
pub(crate) mod tree_core;
pub(crate) mod tree_dirty;
pub(crate) mod tree_dynamic;
pub(crate) mod tree_events;
mod tree_semantics;
mod tree_transform;
mod tree_transition;
pub use tree_core::WidgetTree;
