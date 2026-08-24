use super::*;
// 拆分节点 Effect 生命周期辅助方法，保持 BoxedWidget 主文件低于规模上限。
#[path = "effects.rs"]
// 编译节点 Effect 生命周期的私有实现模块。
mod captured_effects;
// 拆分节点 State 租约辅助方法，保持主体低于规模上限。
#[path = "state_binds.rs"]
// 编译节点 State 租约生命周期的私有实现模块。
mod captured_state_binds;
// 拆分透明包装节点的同轮子测量分发，保持主文件低于规模上限。
#[path = "layout_measure.rs"]
mod layout_measure;
// 拆分定位元数据与子布局求解，保持主体低于规模上限。
#[path = "position_layout.rs"]
mod position_layout;
// 测试只读取布局过滤分配计数，不扩大生产模块公开面。
#[cfg(test)]
pub(crate) use position_layout::{
    filtered_child_allocation_count, reset_filtered_child_allocation_count,
};
// 拆分视觉变换与光标元数据访问，保持主体文件低于规模上限。
#[path = "visual_metadata.rs"]
// 编译运行时节点视觉元数据的私有实现模块。
mod visual_metadata;
// 拆分文字选择元数据访问，保持主体文件低于规模上限。
#[path = "user_select_metadata.rs"]
// 编译声明值与 used-value 的节点私有存储入口。
mod user_select_metadata;

// Table 最多按中、左、右三个固定列区声明父级可见片段。
struct ParentClipRegions {
    regions: [Rect; 3],
    len: usize,
    enabled: bool,
}

impl Default for ParentClipRegions {
    fn default() -> Self {
        Self {
            regions: [Rect::zero(); 3],
            len: 0,
            enabled: false,
        }
    }
}

impl ParentClipRegions {
    // 覆盖内联片段并保留固定容量，不产生第二次堆申请。
    #[cfg(feature = "table")]
    fn set(&mut self, regions: impl IntoIterator<Item = Rect>) {
        let mut next = [Rect::zero(); 3];
        let mut len = 0;
        for region in regions {
            assert!(len < next.len(), "父布局片段不得超过三个固定列区");
            next[len] = region;
            len += 1;
        }
        self.regions = next;
        self.len = len;
        self.enabled = true;
    }

    fn as_slice(&self) -> &[Rect] {
        &self.regions[..self.len]
    }
}

pub struct BoxedWidget {
    widget: Box<dyn Widget>,
    caps: WidgetCapabilities,
    provider_context: ProviderContext,
    id: WidgetId,
    parent: Option<WidgetId>,
    children: Vec<WidgetId>,
    key: Option<Box<str>>,
    automation_id: Option<Box<str>>,
    frame: Rect,
    // 保存父布局为当前节点子树声明的不连续可见片段。
    parent_clip_regions: std::cell::RefCell<Option<Box<ParentClipRegions>>>,
    visible: bool,
    parent_visible: bool,
    visual_transform: ViewTransform,
    // 保存当前运行时节点的定位模式与四边值。
    position: crate::ui::position::PositionedLayout,
    // 保存当前节点自己的文字选择声明。
    declared_user_select: crate::ui::UserSelect,
    // 保存结合祖先边界解析后的最终文字选择策略。
    effective_user_select: crate::ui::UserSelect,
    // 保存当前运行时节点显式覆盖的指针光标；None 表示沿父链继承。
    cursor: Option<crate::platform::windowing::CursorType>,
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
    tab_index_override: Option<i32>,
    // 协调只整体替换签名快照，盒装切片不保留无用容量字段。
    handler_signatures: Box<[HandlerSignature]>,
    // 系统处理器同样只整批替换，保留可变迭代而无需 Vec 容量。
    system_event_handlers: Box<[SystemEventHandlerRegistration]>,
    // 绝大多数实际节点无覆盖，按需分配避免每节点内联大对象。
    accessibility_override: Option<Box<AccessibilityOverride>>,
    // 保存该实际节点持有的结构性 State 订阅租约。
    reconcile_state_binds: Vec<crate::ui::reactive::state::ReconcileBindLease>,
    // 保存该实际节点持有且可在只读绘制后替换的 Paint 订阅租约。
    paint_state_binds: std::cell::RefCell<Vec<crate::ui::reactive::state::PaintBindLease>>,
    // 由此实际节点拥有并随真实移除释放的捕获 Effect。
    effects: Box<[crate::ui::reactive::state::Effect]>,
    // 保存实际挂载节点承载的全部内联组件状态作用域。
    uix_widget_scopes: Box<[crate::ui::widget_state::UixWidgetScopeMarker]>,
}

// 在可能展开 panic 的操作之后恢复调用方提供的可变状态。
fn run_with_unwind_restore<T, R>(
    // 接收需要在操作结束后恢复的可变状态。
    target: &mut T,
    // 接收可能正常返回或触发 panic 的主体操作。
    operation: impl FnOnce(&mut T) -> R,
    // 接收无论主体结果如何都必须执行的恢复操作。
    restore: impl FnOnce(&mut T),
    // 返回主体操作的原始结果类型。
) -> R {
    // 捕获展开过程，以便先恢复状态再继续传播 panic。
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(target)));
    // 在正常返回和 panic 展开两条路径上都执行恢复。
    restore(target);
    // 保持调用方可观察到的返回值或 panic 载荷不变。
    match result {
        // 正常路径直接返回主体结果。
        Ok(value) => value,
        // 异常路径原样恢复展开，避免吞掉组件 panic。
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

impl BoxedWidget {
    pub fn new(widget: Box<dyn Widget>) -> Self {
        Self::new_with_context(widget, current_provider_context())
    }

    pub(crate) fn new_with_context(
        mut widget: Box<dyn Widget>,
        provider_context: ProviderContext,
    ) -> Self {
        let caps = with_provider_context(&provider_context, || {
            let caps = widget.capabilities();
            if let Some(lifecycle) = widget.as_lifecycle_mut() {
                lifecycle.on_init();
            }
            caps
        });
        Self {
            widget,
            caps,
            provider_context,
            id: WidgetId::default(),
            parent: None,
            children: Vec::new(),
            key: None,
            automation_id: None,
            frame: Rect::zero(),
            // 普通节点默认不受父级片段裁剪限制。
            parent_clip_regions: std::cell::RefCell::new(None),
            visible: true,
            parent_visible: true,
            visual_transform: ViewTransform::default(),
            // 新节点默认参与正常布局流。
            position: crate::ui::position::PositionedLayout::default(),
            // 新节点默认没有选择策略覆盖。
            declared_user_select: crate::ui::UserSelect::Auto,
            // 没有父节点时 auto 保持组件默认能力。
            effective_user_select: crate::ui::UserSelect::Auto,
            // 新运行时节点默认继承父节点光标。
            cursor: None,
            view_transition: None,
            view_transition_deadline: None,
            leave_animation: None,
            pending_removal: false,
            attached: false,
            mounted: false,
            active: false,
            destroyed: false,
            z: 0,
            tab_index_override: None,
            handler_signatures: Box::default(),
            system_event_handlers: Box::default(),
            accessibility_override: None,
            // 新节点在接收声明输出前没有结构性 State 租约。
            reconcile_state_binds: Vec::new(),
            // 新节点在首次布局或绘制捕获前没有 Paint 租约。
            paint_state_binds: std::cell::RefCell::new(Vec::new()),
            // 新节点在接收 View 捕获输出前不拥有 Effect。
            effects: Box::default(),
            uix_widget_scopes: Box::default(),
        }
    }
    pub fn widget(&self) -> &dyn Widget {
        &*self.widget
    }
    pub fn widget_mut(&mut self) -> &mut dyn Widget {
        &mut *self.widget
    }

    pub(crate) fn provider_context(&self) -> &ProviderContext {
        &self.provider_context
    }

    pub(crate) fn set_provider_context(&mut self, provider_context: ProviderContext) {
        self.provider_context = provider_context;
    }

    fn with_widget_context<T>(&self, f: impl FnOnce(&dyn Widget) -> T) -> T {
        with_provider_context(&self.provider_context, || f(&*self.widget))
    }

    fn with_widget_context_mut<T>(&mut self, f: impl FnOnce(&mut dyn Widget) -> T) -> T {
        let provider_context = self.provider_context.clone();
        with_provider_context(&provider_context, || f(&mut *self.widget))
    }

    /// 通知组件其直接子节点集合已经完成一次结构变更。
    pub(crate) fn notify_children_changed(&mut self) {
        // 先读取稳定的子节点数量，避免组件回调与节点向量产生重叠借用。
        let child_count = self.children.len();
        // 在节点自己的 ProviderContext 中同步组件派生运行态。
        self.with_widget_context_mut(|widget| widget.on_children_changed(child_count));
    }

    /// 通知组件其后代可见成员已变化，旧自然尺寸不再可作为布局下限。
    pub(crate) fn notify_child_visibility_changed(&mut self) {
        self.with_widget_context_mut(|widget| widget.on_child_visibility_changed());
    }

    pub(crate) fn replace_widget(&mut self, mut widget: Box<dyn Widget>) {
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
            let caps = widget.capabilities();
            if let Some(lifecycle) = widget.as_lifecycle_mut() {
                lifecycle.on_init();
            }
            caps
        });
        self.widget = widget;
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
    pub(crate) fn debug_type_name(&self) -> &'static str {
        self.with_widget_context(|widget| widget.debug_type_name())
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
        self.handler_signatures = signatures.into_boxed_slice();
    }

    pub(crate) fn replace_system_event_handlers(
        &mut self,
        handlers: Vec<SystemEventHandlerRegistration>,
    ) {
        self.system_event_handlers = handlers.into_boxed_slice();
    }

    pub(crate) fn set_accessibility_override(
        &mut self,
        accessibility_override: Option<Box<AccessibilityOverride>>,
    ) {
        self.accessibility_override = accessibility_override;
    }

    // 返回节点身份与状态清理所需的完整内联组件作用域列表。
    pub(crate) fn uix_widget_scopes(&self) -> &[crate::ui::widget_state::UixWidgetScopeMarker] {
        // 借用不参与渲染的元数据。
        &self.uix_widget_scopes
    }

    // 在构建或协调时替换节点的内联组件作用域元数据。
    pub(crate) fn set_uix_widget_scopes(
        &mut self,
        scopes: Vec<crate::ui::widget_state::UixWidgetScopeMarker>,
    ) {
        // 整体替换以保持声明树和挂载树一致。
        self.uix_widget_scopes = scopes.into_boxed_slice();
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
        let base = self.with_widget_context(|widget| {
            let fields = widget.snapshot_fields();
            Self::widget_accessibility(widget, &fields)
        });
        self.apply_accessibility_override(base)
    }

    fn widget_accessibility(widget: &dyn Widget, fields: &SnapshotFields) -> AccessibilitySnapshot {
        let mut accessibility = fields.accessibility();
        if let Some(label) = widget
            .as_any()
            .downcast_ref::<crate::ui::widget_runtime::dynamic_label::DynamicLabel>()
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

    pub(crate) fn widget_snapshot(&self, id: WidgetId) -> WidgetConfigSnapshot {
        self.with_widget_context(|widget| {
            let fields = widget.snapshot_fields();
            let accessibility =
                self.apply_accessibility_override(Self::widget_accessibility(widget, &fields));
            WidgetConfigSnapshot::from_widget_fields(id, widget, fields)
                .with_accessibility(accessibility)
        })
    }

    pub fn as_render(&self) -> Option<&dyn WidgetRender> {
        self.widget.as_render()
    }
    pub fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        self.widget.as_render_mut()
    }
    pub fn as_event(&self) -> Option<&dyn EventHandler> {
        self.widget.as_event()
    }
    pub fn as_event_mut(&mut self) -> Option<&mut dyn EventHandler> {
        self.widget.as_event_mut()
    }
    pub fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        self.widget.as_layout()
    }

    pub fn as_text_input(&self) -> Option<&dyn WidgetTextInput> {
        self.widget.as_text_input()
    }

    pub fn as_lifecycle(&self) -> Option<&dyn WidgetLifecycle> {
        self.widget.as_lifecycle()
    }

    pub fn as_lifecycle_mut(&mut self) -> Option<&mut dyn WidgetLifecycle> {
        self.widget.as_lifecycle_mut()
    }

    // ═══ 便捷分发方法 ═══

    pub fn measure(&self, constraints: Constraints) -> Size {
        // 先在组件上下文中执行组件自己的测量逻辑。
        let measured = self.with_widget_context(|widget| {
            widget
                .as_layout()
                .map(|layout| layout.measure(constraints))
                .unwrap_or_default()
        });
        // 在统一组件边界清除无界哨兵、非有限值和负尺寸。
        crate::ui::layout::engine::normalize_layout_size(measured)
    }

    // 在固有尺寸容器内读取组件不受 Flex basis 归零影响的自然内容尺寸。
    pub(crate) fn measure_natural(&self, constraints: Constraints) -> Size {
        // 在组件 ProviderContext 中调用布局能力的自然测量窄契约。
        let measured = self.with_widget_context(|widget| {
            // 无布局能力的节点继续返回稳定零尺寸。
            widget
                // 通过能力上转型隔离具体组件类型。
                .as_layout()
                // 自然测量只改变尺寸语义，不改变父子所有权。
                .map(|layout| layout.measure_natural(constraints))
                // 无布局能力时采用默认零尺寸。
                .unwrap_or_default()
        });
        // 与普通测量相同，在统一边界清除无界哨兵和非法尺寸。
        crate::ui::layout::engine::normalize_layout_size(measured)
    }

    pub fn flex_grow(&self) -> f32 {
        self.widget()
            .as_layout()
            .map(|l| l.flex_grow())
            .unwrap_or(0.0)
    }
    pub fn flex_shrink(&self) -> f32 {
        self.widget()
            .as_layout()
            .map(|l| l.flex_shrink())
            .unwrap_or(0.0)
    }
    pub fn child_overflow_expands_parent(&self) -> bool {
        self.widget()
            .as_layout()
            .map(|layout| layout.child_overflow_expands_parent())
            .unwrap_or(true)
    }
    pub fn children_clip(&self, frame: Rect) -> Option<Rect> {
        self.widget()
            .as_render()
            .and_then(|r| r.children_clip(frame))
    }
    // 更新父布局为当前节点子树提供的可见片段。
    // 该写入口只由 table 的跨单元格子布局消费。
    #[cfg(feature = "table")]
    pub(crate) fn set_parent_clip_regions(&self, regions: Option<Vec<Rect>>) {
        let mut slot = self.parent_clip_regions.borrow_mut();
        match regions {
            Some(regions) => slot
                .get_or_insert_with(|| Box::new(ParentClipRegions::default()))
                .set(regions),
            None => {
                if let Some(regions) = slot.as_mut() {
                    regions.enabled = false;
                    regions.len = 0;
                }
            }
        }
    }
    // 把父布局片段写入节点自有数组，稳定帧保留既有容量。
    #[cfg(feature = "table")]
    pub(crate) fn set_parent_clip_regions_reusing(&self, regions: impl Iterator<Item = Rect>) {
        let mut slot = self.parent_clip_regions.borrow_mut();
        slot.get_or_insert_with(|| Box::new(ParentClipRegions::default()))
            .set(regions);
    }
    // 在内部借用作用域内读取当前节点子树应使用的父级可见片段。
    pub(crate) fn visit_parent_clip_regions(&self, visitor: &mut dyn FnMut(&[Rect])) -> bool {
        // 借用只覆盖同步访问器调用，不向组件树或 draw 边界泄漏 RefCell guard。
        let regions = self.parent_clip_regions.borrow();
        let Some(regions) = regions.as_deref().filter(|regions| regions.enabled) else {
            return false;
        };
        visitor(regions.as_slice());
        true
    }
    pub fn dirty_rect(&self, frame: Rect) -> Rect {
        self.widget()
            .as_render()
            .map(|r| r.dirty_rect(frame))
            .unwrap_or(frame)
    }
    pub fn overlay_entry(&self, id: WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.widget()
            .as_render()
            .and_then(|r| r.overlay_entry(id, frame))
    }
    // 使用当前逻辑表面查询组件浮层登记。
    pub fn overlay_entry_for_surface(
        // 借用装箱组件。
        &self,
        // 传入浮层所属组件标识。
        id: WidgetId,
        // 传入组件布局矩形。
        frame: Rect,
        // 传入当前逻辑表面矩形。
        surface: Rect,
        // 返回组件生成的可选浮层登记。
    ) -> Option<crate::ui::OverlayEntry> {
        // 获取组件渲染能力。
        self.widget()
            // 仅渲染组件能够声明浮层。
            .as_render()
            // 将当前表面连同组件几何交给渲染能力。
            .and_then(|render| render.overlay_entry_for_surface(id, frame, surface))
    }
    pub fn scroll_delta(&self, frame: Rect) -> Option<(f32, f32)> {
        self.widget().as_event().and_then(|e| e.scroll_delta(frame))
    }
    pub fn scroll_delta_for_dirty(&self) -> Option<(f32, f32)> {
        self.widget()
            .as_event()
            .and_then(|e| e.scroll_delta_for_dirty())
    }
    pub fn scroll_composite_viewport(&self, frame: Rect) -> Rect {
        self.widget()
            .as_event()
            .and_then(|e| e.scroll_composite_viewport(frame))
            .and_then(|viewport| viewport.intersect(&frame))
            .unwrap_or(frame)
    }
    pub fn viewport_scroll_offset(&self) -> Option<(f32, f32)> {
        self.widget()
            .as_event()
            .and_then(|e| e.viewport_scroll_offset())
    }
    pub fn scroll_descendant_by(&mut self, dx: f32, dy: f32) -> bool {
        self.widget_mut()
            .as_event_mut()
            .is_some_and(|event| event.scroll_descendant_by(dx, dy))
    }
    pub fn active_timer(&self) -> Option<(u64, std::time::Duration)> {
        self.widget().as_event().and_then(|e| e.active_timer())
    }
    pub fn wants_capture_phase(&self) -> bool {
        self.system_event_handlers
            .iter()
            .any(SystemEventHandlerRegistration::wants_capture_phase)
            || self
                .widget()
                .as_event()
                .is_some_and(|e| e.wants_capture_phase())
    }
    pub fn wants_continuous_pointer_move(&self) -> bool {
        self.system_event_handlers
            .iter()
            .any(SystemEventHandlerRegistration::wants_continuous_pointer_move)
            || self
                .widget()
                .as_event()
                .is_some_and(|e| e.wants_continuous_pointer_move())
    }
    pub fn hit_test_frame(&self, actual_frame: Rect) -> Rect {
        self.widget()
            .as_event()
            .map(|e| e.hit_test_frame(actual_frame))
            .unwrap_or(actual_frame)
    }
    pub fn hit_test_children(&self) -> bool {
        self.widget()
            .as_event()
            .is_none_or(|event| event.hit_test_children())
    }
    pub fn hit_test_3d(&self, ray: &Ray3D, spatial: &SpatialContext, frame: Rect) -> bool {
        self.widget()
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
        let widget_result = self.with_widget_context_mut(|widget| {
            widget
                .as_event_mut()
                .map(|handler| handler.on_event(event))
                .unwrap_or(EventResult::NotHandled)
        });
        if widget_result == EventResult::NotHandled && bubbled {
            EventResult::Bubbled
        } else {
            widget_result
        }
    }
    pub(crate) fn on_focus_within(&mut self, focused: bool) -> EventResult {
        self.with_widget_context_mut(|widget| {
            widget
                .as_event_mut()
                .map(|handler| handler.on_focus_within(focused))
                .unwrap_or(EventResult::NotHandled)
        })
    }
    pub(crate) fn take_window_action(&mut self) -> Option<crate::ui::event::WindowAction> {
        self.with_widget_context_mut(|widget| {
            widget
                .as_event_mut()
                .and_then(|event| event.take_window_action())
        })
    }
    pub(crate) fn take_layout_request(&mut self) -> bool {
        self.with_widget_context_mut(|widget| {
            widget
                .as_event_mut()
                .is_some_and(|event| event.take_layout_request())
        })
    }
    pub fn semantic_event(
        &self,
        id: WidgetId,
        event: &SystemEvent,
    ) -> Option<crate::ui::event::SemanticEvent> {
        self.widget()
            .as_event()
            .and_then(|e| e.semantic_event(id, event))
    }
    pub fn is_focusable(&self) -> bool {
        self.tab_index() > 0
            && self.visible()
            && self.accepts_events()
            && self.is_interaction_enabled()
    }

    pub(crate) fn set_tab_index_override(&mut self, tab_index: Option<i32>) {
        self.tab_index_override = tab_index.map(|index| index.max(0));
    }

    pub(crate) fn visibility_gate(&self) -> bool {
        self.visible
    }

    pub(crate) fn parent_visibility_gate(&self) -> bool {
        self.parent_visible
    }

    pub(crate) fn set_parent_visible(&mut self, visible: bool) {
        self.parent_visible = visible;
    }

    pub(crate) fn child_visible(&self, index: usize) -> bool {
        self.with_widget_context(|widget| {
            widget
                .as_layout()
                .is_none_or(|layout| layout.child_visible(index))
        })
    }

    pub(crate) fn accepts_events(&self) -> bool {
        self.widget.as_event().is_some() || !self.system_event_handlers.is_empty()
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
        self.widget()
            .as_render()
            .is_some_and(|render| render.uses_palette())
    }
    pub fn picture_policy(&self) -> PicturePolicy {
        self.widget().picture_policy()
    }
    pub fn has_dynamic_content(&self) -> bool {
        self.widget().has_dynamic_content()
    }
    pub fn has_interactive_state(&self) -> bool {
        self.caps.contains(WidgetCapabilities::EVENT) || !self.system_event_handlers.is_empty()
    }
    pub fn on_attach(&mut self) {
        self.with_widget_context_mut(|widget| {
            if let Some(lifecycle) = widget.as_lifecycle_mut() {
                lifecycle.on_attach();
            }
        });
    }
    pub fn on_mount(&mut self) {
        self.with_widget_context_mut(|widget| {
            if let Some(lifecycle) = widget.as_lifecycle_mut() {
                lifecycle.on_mount();
            }
        });
    }
    pub fn on_active(&mut self) {
        self.with_widget_context_mut(|widget| {
            if let Some(lifecycle) = widget.as_lifecycle_mut() {
                lifecycle.on_active();
            }
        });
    }
    pub fn on_inactive(&mut self) {
        self.with_widget_context_mut(|widget| {
            if let Some(lifecycle) = widget.as_lifecycle_mut() {
                lifecycle.on_inactive();
            }
        });
    }
    pub fn on_theme_changed(&mut self) {
        self.with_widget_context_mut(|widget| {
            if let Some(lifecycle) = widget.as_lifecycle_mut() {
                lifecycle.on_theme_changed();
            }
        });
    }
    pub fn on_unmount(&mut self) {
        self.with_widget_context_mut(|widget| {
            if let Some(lifecycle) = widget.as_lifecycle_mut() {
                lifecycle.on_unmount();
            }
        });
    }
    pub fn on_detach(&mut self) {
        self.with_widget_context_mut(|widget| {
            if let Some(lifecycle) = widget.as_lifecycle_mut() {
                lifecycle.on_detach();
            }
        });
    }
    pub fn on_destroy(&mut self) {
        self.with_widget_context_mut(|widget| {
            if let Some(lifecycle) = widget.as_lifecycle_mut() {
                lifecycle.on_destroy();
            }
        });
    }
    pub fn render(
        &self,
        frame: Rect,
        ctx: &mut crate::ui::widget_runtime::paint_context::PaintContext,
        tree: &WidgetTree,
    ) {
        let config = &self.provider_context.config;
        let theme = config.theme.as_ref().map(|theme| theme.tokens_arc());
        let patch = config.widget_tokens.get(self.widget.as_any().type_id());
        let previous_scope = ctx.replace_token_scope(theme, patch);
        // 将组件绘制包进必定恢复 TokenScope 的 panic 展开边界。
        run_with_unwind_restore(
            // 传入当前 UI 绘制上下文作为需要恢复的状态。
            ctx,
            // 在临时主题作用域内执行组件绘制。
            |ctx| {
                // 保持 ProviderContext 与组件调用约定不变。
                self.with_widget_context(|widget| {
                    // 仅调用具备绘制能力的组件。
                    if let Some(render) = widget.as_render() {
                        // 把当前临时作用域中的上下文交给组件。
                        render.render(frame, ctx, tree);
                    }
                });
            },
            // 无论正常返回或 panic 都恢复进入组件前的令牌作用域。
            |ctx| ctx.restore_token_scope(previous_scope),
        );
    }
}

impl WidgetCore for BoxedWidget {
    #[cfg(any(test, feature = "test-harness"))]
    fn id(&self) -> WidgetId {
        self.id
    }
    fn set_id(&mut self, id: WidgetId) {
        self.id = id;
    }
    fn parent(&self) -> Option<WidgetId> {
        self.parent
    }
    fn set_parent(&mut self, id: Option<WidgetId>) {
        self.parent = id;
    }
    fn children(&self) -> &[WidgetId] {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<WidgetId> {
        &mut self.children
    }
    fn frame(&self) -> Rect {
        self.frame
    }
    fn set_frame(&mut self, rect: Rect) {
        // 所有直接或树内 frame 写入都在最终存储边界满足实际几何契约。
        self.frame = crate::ui::layout::engine::normalize_layout_rect(rect);
    }
    fn visible(&self) -> bool {
        self.visible && self.parent_visible && self.with_widget_context(Widget::visible)
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
        self.tab_index_override
            .unwrap_or_else(|| self.with_widget_context(Widget::tab_index))
    }
    fn set_tab_index(&mut self, v: i32) {
        self.set_tab_index_override(Some(v));
    }
}

// 仅在测试构建中编译 TokenScope 展开回归。
#[cfg(test)]
// 将子模块显式定位到 widget 目录中的独立测试文件。
#[path = "../../../../../tests/unit/ui/widget_runtime/widget/boxed_scope_tests.rs"]
// 从独立文件加载作用域恢复测试，保持本文件低于行数上限。
mod boxed_scope_tests;
