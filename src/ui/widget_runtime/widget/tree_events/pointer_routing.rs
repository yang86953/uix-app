use super::*;
use crate::ui::event::SemanticKind;
use crate::ui::{OverlayEntry, OverlayKind};

impl WidgetTree {
    pub(crate) fn cancel_hidden_interaction(&mut self) {
        let targets = [
            self.managers().interaction.hovered_component(),
            self.managers().interaction.pressed_component(),
            self.managers().drag.target(),
        ];
        let mut cancelled = Vec::new();
        for target in targets.into_iter().flatten() {
            if !cancelled.contains(&target) && !self.is_effectively_visible(target) {
                self.cancel_pointer_state_in_subtree(target);
                cancelled.push(target);
            }
        }
        if self
            .managers()
            .focus
            .focused_component()
            .is_some_and(|focused| !self.focus_target_available(focused))
        {
            self.set_focus(None);
        }
    }

    pub(crate) fn cancel_subtree_interaction(&mut self, root: WidgetId) {
        self.cancel_pointer_state_in_subtree(root);
        if self
            .managers()
            .focus
            .focused_component()
            .is_some_and(|focused| self.is_descendant_of(focused, root))
        {
            self.set_focus(None);
        }
    }

    pub(crate) fn cancel_pointer_state_in_subtree(&mut self, root: WidgetId) {
        self.cancel_pointer_hover_in_subtree(root);
        self.cancel_pointer_gesture_in_subtree(root);
    }

    pub(crate) fn cancel_pointer_hover_in_subtree(&mut self, root: WidgetId) {
        let hovered = self
            .managers()
            .interaction
            .hovered_component()
            .filter(|target| self.is_descendant_of(*target, root));
        let Some(hovered) = hovered else {
            return;
        };
        let leave_is_delivered_by_gesture_cancel =
            self.managers().interaction.pressed_component() == Some(hovered);
        self.managers_mut().interaction.set_hovered_component(None);
        if !leave_is_delivered_by_gesture_cancel
            && self.dispatch_to(hovered, &SystemEvent::PointerLeave) == EventResult::Handled
        {
            self.invalidate_paint(hovered);
        }
        self.rebuild_widget_overlays();
    }

    pub(crate) fn cancel_pointer_gesture_in_subtree(&mut self, root: WidgetId) {
        let owns_pressed = self
            .managers()
            .interaction
            .pressed_component()
            .is_some_and(|target| self.is_descendant_of(target, root));
        let owns_drag = self
            .managers()
            .drag
            .target()
            .is_some_and(|target| self.is_descendant_of(target, root));
        if owns_pressed || owns_drag {
            self.cancel_active_pointer_gesture();
        }
    }

    pub(super) fn cancel_active_pointer_gesture(&mut self) {
        let pressed = self.managers().interaction.pressed_component();
        let drag = self.managers().drag.is_dragging().then(|| {
            (
                self.managers().drag.target(),
                self.managers().drag.last_pos(),
                self.managers().drag.button(),
                self.managers().drag.mods(),
            )
        });
        self.managers_mut().interaction.set_pressed_component(None);
        self.managers_mut().drag.end_drag();

        if let Some((Some(target), pos, button, mods)) = drag {
            // DragEnd 未被拖拽目标消费：记录日志定位路由失败，行为不变。
            if self.dispatch_to(target, &SystemEvent::DragEnd { pos, button, mods })
                == EventResult::NotHandled
            {
                tracing::warn!(event = "DragEnd", target = ?target, "drag end was not handled");
            }
        }
        if let Some(pressed) = pressed {
            self.invalidate_paint(pressed);
            // PointerLeave 是可选生命周期通知；无状态组件不消费时仍保持真实返回值。
            let _ = self.dispatch_to(pressed, &SystemEvent::PointerLeave);
        }
        self.rebuild_widget_overlays();
    }

    // 原生窗口管理器接管指针后，清除 UI pressed/drag 而不改变键盘焦点。
    pub(crate) fn cancel_pointer_gesture_for_native_handoff(&mut self) {
        // 复用唯一手势取消实现，统一交付 DragEnd 与 PointerLeave 清理语义。
        self.cancel_active_pointer_gesture();
        // 结束原生指针接管清理。
    }

    pub(super) fn dispatch_pointer_release(
        &mut self,
        event: &SystemEvent,
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    ) -> EventResult {
        let hold = self
            .managers_mut()
            .interaction
            .release_pressed_pointer(button);
        let releases_drag = self.managers().drag.is_gesture_button(button);
        // 仅由启动键结束拖拽；其他按键释放不改变 potential/active 手势。
        if releases_drag && self.managers().drag.is_dragging() {
            if let Some(target) = self.managers().drag.target() {
                let drag_end = SystemEvent::DragEnd { pos, button, mods };
                // DragEnd 未被拖拽目标消费：记录日志，行为不变。
                if self.dispatch_to(target, &drag_end) == EventResult::NotHandled {
                    tracing::warn!(event = "DragEnd", target = ?target, "drag end was not handled");
                }
            }
        }
        if releases_drag {
            self.managers_mut().drag.end_drag();
        }
        let hit = self.pointer_target_at(pos);
        let mut result = EventResult::NotHandled;
        if let Some(target) = hit {
            self.invalidate_paint(target);
            let actions_before_dispatch = self.pending_window_actions.len();
            if self.capture_to(target, event).is_some() {
                if let Some(pressed) = hold {
                    self.invalidate_paint(pressed);
                    // 捕获接管只需交付离开通知；零消费者是合法状态。
                    let _ = self.dispatch_to(pressed, &SystemEvent::PointerLeave);
                }
                self.rebuild_widget_overlays();
                return EventResult::Handled;
            }
            result = self.dispatch_to(target, event);
            let title_bar_system_menu = self.pending_window_actions[actions_before_dispatch..]
                .contains(&WindowAction::ShowSystemMenuFromTitleBar);
            // 只有同一目标、同一鼠标键的 down/up 才合成 Click。
            if hold == Some(target) && !title_bar_system_menu {
                let click = ClickEvent {
                    button,
                    pos,
                    modifiers: mods,
                };
                // Click 是可选观察事件；零订阅者必须安静地保留 NotHandled 契约。
                let _ = self.dispatch_semantic(SemanticEvent::click(target, click));
                if button == MouseButton::Right {
                    let mut context_menu = SemanticEvent::context_menu(target, click);
                    // ContextMenu 语义未被消费：记录日志，行为不变。
                    if self.dispatch_semantic_event(&mut context_menu) == EventResult::NotHandled {
                        tracing::warn!(
                            event = "ContextMenu",
                            target = ?target,
                            "context menu semantic was not handled"
                        );
                    }
                    if !context_menu.default_prevented() {
                        self.open_context_menu_overlay(target, click.pos);
                    }
                }
            }
        }
        if let Some(pressed) = hold {
            if Some(pressed) != hit {
                self.invalidate_paint(pressed);
                // 捕获目标外松开时，先通知指针已离开，再交付最终 PointerUp。
                // 离开通知没有消费者属于正常情况，不提升为路由失败。
                let _ = self.dispatch_to(pressed, &SystemEvent::PointerLeave);
                // PointerUp 是已建立手势的最终命令，未被消费仍保留诊断。
                if self.dispatch_to(pressed, event) == EventResult::NotHandled {
                    tracing::warn!(event = "PointerUp", target = ?pressed, "pointer up was not handled");
                }
            }
        }
        self.rebuild_widget_overlays();
        result
    }

    pub(super) fn overlay_target_at(&self, pos: Point) -> Option<WidgetId> {
        let owner = self
            .overlay_stack
            .hit_test(pos.x, pos.y)
            .map(|entry| entry.owner())?;
        self.hit_test_internal(owner, pos).or(Some(owner))
    }

    pub(crate) fn pointer_target_at(&self, pos: Point) -> Option<WidgetId> {
        self.overlay_target_at(pos).or_else(|| self.hit_test(pos))
    }

    fn blocking_top_overlay_at(&self, pos: Point) -> Option<&OverlayEntry> {
        let top = self.overlay_stack.top()?;
        let inside_top = top.bounds_rect().is_some_and(|bounds| bounds.contains(pos));
        (!inside_top && (top.is_modal() || top.traps_focus() || top.dismisses_on_outside()))
            .then_some(top)
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn pointer_down_blocker_at(&self, pos: Point) -> Option<WidgetId> {
        self.blocking_top_overlay_at(pos).map(OverlayEntry::owner)
    }

    pub(super) fn intercept_top_overlay_outside_pointer_down(
        &mut self,
        pos: Point,
    ) -> Option<EventResult> {
        let top = self.blocking_top_overlay_at(pos).cloned()?;
        if top.dismisses_on_outside() {
            // 先通知具体 owner 执行用户取消语义，再移除当前浮层登记。
            crate::ui::tree_widget_hooks::dismiss_overlay_owner_from_outside(self, top.owner());
            self.overlay_stack.remove(top.id());
            self.invalidate_paint(top.owner());
        }
        self.restore_focus_after_trap_owner(top.owner());
        Some(EventResult::Handled)
    }

    pub(super) fn open_context_menu_overlay(&mut self, owner: WidgetId, pos: Point) {
        self.overlay_stack
            .retain_entries(|entry| entry.kind() != OverlayKind::ContextMenu);
        self.overlay_stack.push_entry(
            OverlayEntry::new(owner, OverlayKind::ContextMenu)
                .bounds(Rect::new(pos.x, pos.y, 160.0, 160.0))
                .z_index(1200)
                .dismiss_on_outside(true)
                .managed(true),
        );
        self.invalidate_paint(owner);
    }

    pub(super) fn secondary_pointer_drag_boundary(
        &self,
        target: WidgetId,
        event: &SystemEvent,
    ) -> Option<WidgetId> {
        if !matches!(
            event,
            SystemEvent::PointerDown {
                button: MouseButton::Right,
                ..
            }
        ) {
            return None;
        }

        let mut current = Some(target);
        let mut reserves_context_menu = false;
        while let Some(id) = current {
            let node = self.get(id)?;
            if crate::ui::tree_widget_hooks::is_drag_region(self, id) {
                return reserves_context_menu.then_some(id);
            }
            reserves_context_menu |= node.tab_index() > 0
                || node
                    .handler_signatures()
                    .iter()
                    .any(|signature| signature.kind == SemanticKind::ContextMenu);
            current = node.parent();
        }
        None
    }
}

// 单元测试验证原生窗口接管只清理指针手势而保留键盘焦点。
#[cfg(test)]
// 测试模块直接观察 WidgetTree manager 状态，不依赖真实窗口。
mod tests {
    // 引入当前模块的 WidgetTree 与输入类型。
    use super::*;
    // 使用原子计数器记录当前测试线程收到的警告。
    use std::sync::Arc;
    // 引入无锁计数器及其内存序。
    use std::sync::atomic::{AtomicUsize, Ordering};
    // 引入 tracing span 生命周期类型以实现最小订阅器。
    use tracing::span::{Attributes, Id, Record};
    // 引入 callsite 兴趣声明，确保测试订阅器能观察目标日志点。
    use tracing::subscriber::Interest;
    // 引入事件元数据与订阅器契约。
    use tracing::{Event, Metadata, Subscriber};
    // 使用简单节点建立可寻址的 pressed 与 focused 目标。
    use crate::ui::widgets::Label;

    // 仅统计当前线程警告事件的最小订阅器。
    struct WarningCounter {
        // 跨闭包共享警告数量。
        count: Arc<AtomicUsize>,
    }

    // 为回归测试实现 tracing 订阅器契约。
    impl Subscriber for WarningCounter {
        // 要求 callsite 在局部订阅器启用时重新检查。
        fn register_callsite(&self, _metadata: &'static Metadata<'static>) -> Interest {
            // 始终允许当前测试观察事件。
            Interest::always()
        }

        // 当前测试接收全部级别，再在 event 中筛选警告。
        fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
            // 保持 callsite 启用。
            true
        }

        // 测试不记录 span，只返回稳定虚拟身份。
        fn new_span(&self, _span: &Attributes<'_>) -> Id {
            // 使用非零稳定身份满足 tracing 契约。
            Id::from_u64(1)
        }

        // 测试不保存 span 字段。
        fn record(&self, _span: &Id, _values: &Record<'_>) {}

        // 测试不保存 span 继承关系。
        fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

        // 统计当前局部订阅器收到的警告事件。
        fn event(&self, event: &Event<'_>) {
            // 仅累加 WARN，忽略正常诊断级别。
            if *event.metadata().level() == tracing::Level::WARN {
                // 单线程测试只需宽松内存序。
                self.count.fetch_add(1, Ordering::Relaxed);
            }
        }

        // 测试不保存 span 进入状态。
        fn enter(&self, _span: &Id) {}

        // 测试不保存 span 离开状态。
        fn exit(&self, _span: &Id) {}
    }

    // 在局部订阅器中执行操作并返回警告数量。
    fn warning_count_during(operation: impl FnOnce()) -> usize {
        // 创建共享计数器供订阅器与断言读取。
        let count = Arc::new(AtomicUsize::new(0));
        // 构造仅作用于当前线程闭包的订阅器。
        let subscriber = WarningCounter {
            // 共享相同警告计数器。
            count: Arc::clone(&count),
        };
        // 在局部 tracing 上下文中执行待验证操作。
        tracing::subscriber::with_default(subscriber, operation);
        // 返回操作期间观察到的最终警告数。
        count.load(Ordering::Relaxed)
    }

    // 验证无人订阅的 Click 不会伪报路由失败。
    #[test]
    // 测试覆盖可命中但没有事件处理器的普通叶子节点。
    fn unobserved_click_does_not_emit_route_warning() {
        // 创建空组件树。
        let mut tree = WidgetTree::new();
        // 使用 Label 表示合法的零 Click 订阅节点。
        let target = tree.set_root(Box::new(Label::new("passive click target")));
        // 为根节点设置可命中的稳定几何。
        tree.set_frame_dirty(target, Rect::new(0.0, 0.0, 80.0, 24.0));
        // 模拟同一目标上的左键按压状态。
        let began = tree
            // 访问交互管理器。
            .managers_mut()
            // 选择按压状态。
            .interaction
            // 绑定目标与左键。
            .begin_pressed_pointer(Some(target), MouseButton::Left);
        // 确认手势成功建立。
        assert!(began);
        // 构造同一目标范围内的释放事件。
        let event = SystemEvent::PointerUp {
            // 使用根节点内部坐标。
            pos: Point::new(8.0, 8.0),
            // 匹配此前建立的左键手势。
            button: MouseButton::Left,
            // 当前没有修饰键。
            mods: KeyMod::NONE,
        };
        // 捕获释放路径发出的警告。
        let warnings = warning_count_during(|| {
            // 分发释放并允许语义 Click 保持 NotHandled。
            let result = tree.dispatch_pointer_release(
                // 传入完整系统事件。
                &event,
                // 传入命中目标的屏幕坐标。
                Point::new(8.0, 8.0),
                // 匹配左键手势。
                MouseButton::Left,
                // 当前没有修饰键。
                KeyMod::NONE,
            );
            // 被动节点的系统 PointerUp 仍应保持真实 NotHandled。
            assert_eq!(result, EventResult::NotHandled);
        });
        // 零 Click 订阅者不得产生警告。
        assert_eq!(warnings, 0);
    }

    // 验证无人消费的 PointerLeave 不会伪报路由失败。
    #[test]
    // 测试覆盖取消按压手势时的可选生命周期通知。
    fn unhandled_pointer_leave_does_not_emit_route_warning() {
        // 创建空组件树。
        let mut tree = WidgetTree::new();
        // 使用不处理 PointerLeave 的 Label 作为合法目标。
        let target = tree.set_root(Box::new(Label::new("passive leave target")));
        // 建立需要取消的左键按压状态。
        let began = tree
            // 访问交互管理器。
            .managers_mut()
            // 选择按压状态。
            .interaction
            // 绑定目标与左键。
            .begin_pressed_pointer(Some(target), MouseButton::Left);
        // 确认手势成功建立。
        assert!(began);
        // 捕获取消路径发出的警告。
        let warnings = warning_count_during(|| {
            // 取消手势会向目标交付可选 PointerLeave。
            tree.cancel_active_pointer_gesture();
        });
        // 零 PointerLeave 消费者不得产生警告。
        assert_eq!(warnings, 0);
        // 取消后必须清除按压状态。
        assert_eq!(tree.managers().interaction.pressed_component(), None);
    }

    // 验证 native handoff 后不会残留 pressed/potential drag。
    #[test]
    // 测试覆盖窗口移动接管与键盘焦点隔离契约。
    fn native_move_handoff_cancels_pointer_gesture_without_blurring_keyboard_focus() {
        // 创建空组件树。
        let mut tree = WidgetTree::new();
        // 添加稳定根节点作为手势与焦点目标。
        let target = tree.set_root(Box::new(Label::new("native handoff")));
        // 模拟 PointerDown 建立 pressed 捕获。
        let began = tree
            // 访问交互 manager。
            .managers_mut()
            // 选择 pressed 状态所有者。
            .interaction
            // 绑定左键与目标节点。
            .begin_pressed_pointer(Some(target), MouseButton::Left);
        // 当前没有冲突按键，手势建立必须成功。
        assert!(began);
        // 模拟同一次 PointerDown 建立潜在拖动手势。
        tree.managers_mut().drag.begin_gesture(
            // 绑定相同目标节点。
            Some(target),
            // 保存稳定起点。
            Point::new(4.0, 5.0),
            // 使用标题栏拖动的左键。
            MouseButton::Left,
            // 本场景没有修饰键。
            KeyMod::NONE,
            // 结束潜在手势建立。
        );
        // 独立建立键盘焦点，确保指针取消不会伪造 WindowBlur。
        tree.managers_mut()
            // 访问焦点 manager。
            .focus
            // 绑定同一稳定节点作为测试焦点。
            .set_focused_component(Some(target));
        // 原生窗口管理器接管 pointer。
        tree.cancel_pointer_gesture_for_native_handoff();
        // pressed 捕获必须被清除。
        assert_eq!(tree.managers().interaction.pressed_component(), None);
        // 潜在拖动必须被清除。
        assert!(!tree.managers().drag.is_potential());
        // 活跃拖动同样不得残留。
        assert!(!tree.managers().drag.is_dragging());
        // 键盘焦点必须保持，不能用 WindowBlur 代替 pointer cancel。
        assert_eq!(tree.managers().focus.focused_component(), Some(target));
        // 结束原生接管手势测试。
    }
    // 结束 WidgetTree 原生接管测试模块。
}
