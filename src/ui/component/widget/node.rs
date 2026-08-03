use super::*;

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
    pub(crate) tab_index_override: Option<i32>,
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
            tab_index_override: None,
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
            tab_index_override: None,
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
        let idx = idx.max(0);
        self.tab_idx = idx;
        self.tab_index_override = Some(idx);
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
        state: &crate::ui::reactive::state::State<T>,
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
        computed: &crate::ui::reactive::state::Computed<T>,
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

