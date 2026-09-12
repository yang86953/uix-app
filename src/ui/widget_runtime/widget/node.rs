use super::*;

/// 尚未挂载到 [`WidgetTree`] 的组件、子树与声明期元数据。
pub struct WidgetNode {
    /// 此节点拥有的组件实例。
    pub widget: Box<dyn Widget>,
    /// 按声明顺序排列的直接子节点。
    pub children: Vec<WidgetNode>,
    pub(crate) provider_context: ProviderContext,
    pub(crate) visible: bool,
    pub(crate) visual_transform: ViewTransform,
    // 保存待挂载节点的完整定位声明。
    pub(crate) position: crate::ui::position::PositionedLayout,
    // 保存声明节点样式中的 min/max 尺寸约束，供父布局统一消费。
    pub(crate) size_constraints: crate::ui::theme::style::SizeConstraints,
    // 保存待挂载节点声明的文字选择策略。
    pub(crate) user_select: crate::ui::UserSelect,
    // 保存待挂载节点显式声明的指针光标；None 表示继承。
    pub(crate) cursor: Option<crate::platform::windowing::CursorType>,
    pub(crate) enter_animation: Option<crate::ui::animation::AnimationConfig>,
    pub(crate) enter_deadline: Option<std::time::Instant>,
    pub(crate) leave_animation: Option<crate::ui::animation::AnimationConfig>,
    // 保存声明节点自身的视觉透明度；默认 1.0 表示不额外衰减。
    pub(crate) declared_opacity: f32,
    /// 此节点在同级绘制和命中顺序中的层级值。
    pub z_index: i32,
    /// 用于协调同级声明节点身份的稳定 key。
    pub key: Option<Box<str>>,
    /// 暴露给自动化和检查工具的稳定身份。
    pub automation_id: Option<Box<str>>,
    /// 此节点参与 Tab 导航时使用的非负顺序索引。
    pub tab_idx: i32,
    pub(crate) tab_index_override: Option<i32>,
    pub(crate) focus_handle: Option<FocusHandle>,
    // 无障碍覆盖是稀疏元数据，沿协调流水线移动同一按需分配对象。
    pub(crate) accessibility_override: Option<Box<AccessibilityOverride>>,
    /// 此节点登记的语义事件处理器。
    pub handlers: Vec<HandlerRegistration>,
    pub(crate) system_event_handlers: Vec<SystemEventHandlerRegistration>,
    pub(crate) render_handlers: Vec<RenderHandlerRegistration>,
    // 保存捕获阶段交接的结构性 State 绑定。
    pub(crate) captured_state_binds:
        Vec<std::sync::Arc<dyn crate::ui::reactive::state::StatePaintBind>>,
    // 子树作用域重建工厂；存在时 captured_state_binds 安装为节点作用域失效。
    pub(crate) scoped_rebuild: Option<std::sync::Arc<dyn Fn() -> crate::ui::view::ViewNode>>,
    // 保存捕获阶段交接的节点私有 Effect。
    pub(crate) captured_effects: Vec<crate::ui::reactive::state::Effect>,
    // 保留内联组件的非视觉状态作用域标记。
    pub(crate) uix_widget_scopes: Vec<crate::ui::widget_state::UixWidgetScopeMarker>,
    // 保存声明节点捕获、待实际节点身份建立后转交树的动画源。
    pub(crate) animated_sources: Vec<std::sync::Arc<dyn crate::ui::animation::AnimatedSource>>,
}

impl WidgetNode {
    /// 使用给定组件和直接子节点创建声明节点。
    pub fn new(widget: Box<dyn Widget>, children: Vec<WidgetNode>) -> Self {
        Self {
            widget,
            children,
            provider_context: current_provider_context(),
            visible: true,
            visual_transform: ViewTransform::default(),
            // 命令式节点默认参与正常布局流。
            position: crate::ui::position::PositionedLayout::default(),
            // 命令式节点默认不施加尺寸约束。
            size_constraints: crate::ui::theme::style::SizeConstraints::NONE,
            // 命令式节点默认保留组件自身选择能力。
            user_select: crate::ui::UserSelect::Auto,
            // 命令式节点默认不覆盖父节点光标。
            cursor: None,
            enter_animation: None,
            enter_deadline: None,
            leave_animation: None,
            declared_opacity: 1.0,
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
            // 新建命令式节点默认没有捕获的 State 绑定。
            captured_state_binds: Vec::new(),
            // 新建命令式节点默认不携带作用域重建工厂。
            scoped_rebuild: None,
            // 新建命令式节点默认没有捕获的 Effect。
            captured_effects: Vec::new(),
            // 新建命令式节点默认没有捕获的动画源。
            animated_sources: Vec::new(),
            uix_widget_scopes: Vec::new(),
        }
    }
    /// 设置用于同级协调的稳定 key。
    pub fn key(mut self, k: &str) -> Self {
        self.key = Some(k.into());
        self
    }
    /// 设置暴露给自动化和检查工具的稳定身份。
    pub fn automation_id(mut self, id: &str) -> Self {
        self.automation_id = Some(id.into());
        self
    }
    /// 创建没有直接子节点的声明节点。
    pub fn leaf(widget: Box<dyn Widget>) -> Self {
        Self {
            widget,
            children: vec![],
            provider_context: current_provider_context(),
            visible: true,
            visual_transform: ViewTransform::default(),
            // 命令式叶节点默认参与正常布局流。
            position: crate::ui::position::PositionedLayout::default(),
            // 命令式叶节点默认不施加尺寸约束。
            size_constraints: crate::ui::theme::style::SizeConstraints::NONE,
            // 命令式叶节点默认保留组件自身选择能力。
            user_select: crate::ui::UserSelect::Auto,
            // 命令式叶节点默认不覆盖父节点光标。
            cursor: None,
            enter_animation: None,
            enter_deadline: None,
            leave_animation: None,
            declared_opacity: 1.0,
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
            // 叶节点默认没有捕获的 State 绑定。
            captured_state_binds: Vec::new(),
            // 叶节点默认不携带作用域重建工厂。
            scoped_rebuild: None,
            // 叶节点默认没有捕获的 Effect。
            captured_effects: Vec::new(),
            // 叶节点默认没有捕获的动画源。
            animated_sources: Vec::new(),
            uix_widget_scopes: Vec::new(),
        }
    }
    /// 设置此节点在同级中的绘制和命中层级。
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
    // 设置待挂载节点的完整定位声明。
    pub(crate) fn with_position(mut self, position: crate::ui::position::PositionedLayout) -> Self {
        // 保留模式与四边值供组件树统一求解。
        self.position = position;
        // 返回可继续组装的节点。
        self
    }
    // 设置待挂载节点声明的 min/max 尺寸约束。
    pub(crate) fn with_size_constraints(
        mut self,
        constraints: crate::ui::theme::style::SizeConstraints,
    ) -> Self {
        self.size_constraints = constraints;
        self
    }
    // 设置待挂载节点声明的文字选择策略。
    pub(crate) fn with_user_select(mut self, value: crate::ui::UserSelect) -> Self {
        // 保留显式策略供实际树结合父级解析。
        self.user_select = value;
        // 返回可继续组装的节点。
        self
    }
    // 设置待挂载节点显式声明的指针光标。
    pub(crate) fn with_cursor(mut self, cursor: crate::platform::windowing::CursorType) -> Self {
        // Some(Arrow) 保留“覆盖继承为默认箭头”的语义。
        self.cursor = Some(cursor);
        // 返回可继续组装的声明节点。
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
    // 设置待挂载节点声明的视觉透明度；调用方传入非法值时按无效果归一。
    pub(crate) fn with_declared_opacity(mut self, opacity: f32) -> Self {
        self.declared_opacity = normalize_declared_opacity(opacity);
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
    /// 追加一个语义事件处理器注册。
    pub fn on_semantic(mut self, registration: HandlerRegistration) -> Self {
        self.handlers.push(registration);
        self
    }
    /// 追加在调用时可读取给定响应式状态快照的语义处理器。
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
    /// 追加在调用时可读取给定派生状态快照的语义处理器。
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
    /// 追加捕获目标窗口身份的语义事件处理器。
    pub fn on_semantic_window_capture(
        mut self,
        mut registration: HandlerRegistration,
        window_id: crate::core::WindowId,
    ) -> Self {
        registration = registration.with_window_capture(window_id);
        self.handlers.push(registration);
        self
    }
    /// 替换此节点的全部语义事件处理器。
    pub fn with_handlers(mut self, handlers: Vec<HandlerRegistration>) -> Self {
        self.handlers = handlers;
        self
    }
    pub(crate) fn with_accessibility_override(
        mut self,
        accessibility_override: Box<AccessibilityOverride>,
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

    // 把 View 捕获输出的 State 绑定交给将来拥有节点的 WidgetTree。
    pub(crate) fn with_captured_state_binds(
        mut self,
        state_binds: Vec<std::sync::Arc<dyn crate::ui::reactive::state::StatePaintBind>>,
    ) -> Self {
        // 保留声明节点本次捕获的全部结构性依赖。
        self.captured_state_binds = state_binds;
        // 返回仍可继续配置的节点。
        self
    }

    // 把子树作用域重建工厂交给将来拥有节点的 WidgetTree。
    pub(crate) fn with_scoped_rebuild(
        mut self,
        rebuild: Option<std::sync::Arc<dyn Fn() -> crate::ui::view::ViewNode>>,
    ) -> Self {
        // 保存作用域闭包；结构性 State 绑定随后仍走既有交接通道。
        self.scoped_rebuild = rebuild;
        // 返回仍可继续配置的节点。
        self
    }

    // 把 View 捕获输出的 Effect 交给将来拥有节点的 BoxedWidget。
    pub(crate) fn with_captured_effects(
        mut self,
        effects: Vec<crate::ui::reactive::state::Effect>,
    ) -> Self {
        // 保留节点生命周期拥有的全部 Effect。
        self.captured_effects = effects;
        // 返回仍可继续配置的节点。
        self
    }

    // 把声明节点捕获的动画源交给将来拥有该节点的 WidgetTree。
    pub(crate) fn with_animated_sources(
        mut self,
        sources: Vec<std::sync::Arc<dyn crate::ui::animation::AnimatedSource>>,
    ) -> Self {
        // 保留节点生命周期拥有的全部动画源。
        self.animated_sources = sources;
        // 返回带有动画生命周期元数据的节点。
        self
    }

    // 把声明节点承载的全部内联组件作用域传递到树节点。
    pub(crate) fn with_uix_widget_scopes(
        mut self,
        scopes: Vec<crate::ui::widget_state::UixWidgetScopeMarker>,
    ) -> Self {
        // 保留原始顺序，使嵌套展开身份可精确比较。
        self.uix_widget_scopes = scopes;
        // 返回带有生命周期元数据的节点。
        self
    }

    pub(crate) fn with_provider_context(mut self, context: ProviderContext) -> Self {
        self.provider_context = context;
        self
    }
}

// 把声明透明度归一为可绘制值；非有限输入按无衰减处理。
pub(crate) fn normalize_declared_opacity(opacity: f32) -> f32 {
    if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        1.0
    }
}
