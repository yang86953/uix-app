// 引入当前模块的 WidgetTree 与输入类型。
use super::*;
// 构造可计数动态可见性的事件节点。
use std::any::Any;
// 使用原子计数器记录当前测试线程收到的警告。
use std::sync::Arc;
// 引入无锁计数器及其内存序。
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
// 引入 tracing span 生命周期类型以实现最小订阅器。
use tracing::span::{Attributes, Id, Record};
// 引入 callsite 兴趣声明，确保测试订阅器能观察目标日志点。
use tracing::subscriber::Interest;
// 引入事件元数据与订阅器契约。
use tracing::{Event, Metadata, Subscriber};
// 引入最小组件与事件能力契约。
use crate::ui::{EventHandler, Widget, WidgetCapabilities};
// 使用简单节点建立可寻址的 pressed 与 focused 目标。
use crate::ui::widgets::Label;

// 记录动态可见性查询次数，验证同一事件批次内的可见结果复用。
struct VisibilityProbe {
    visible: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
}

impl Widget for VisibilityProbe {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        let mut caps = WidgetCapabilities::new();
        caps.insert(WidgetCapabilities::EVENT);
        caps
    }

    fn visible(&self) -> bool {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.visible.load(Ordering::Relaxed)
    }

    fn may_produce_overlay(&self) -> bool {
        false
    }

    fn as_event(&self) -> Option<&dyn EventHandler> {
        Some(self)
    }

    fn as_event_mut(&mut self) -> Option<&mut dyn EventHandler> {
        Some(self)
    }
}

impl EventHandler for VisibilityProbe {
    fn interaction_enabled(&self) -> Option<bool> {
        Some(true)
    }
}

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
    assert_eq!(tree.managers().interaction.pressed_widget(), None);
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
        .set_focused_widget(Some(target));
    // 原生窗口管理器接管 pointer。
    tree.cancel_pointer_gesture_for_native_handoff();
    // pressed 捕获必须被清除。
    assert_eq!(tree.managers().interaction.pressed_widget(), None);
    // 潜在拖动必须被清除。
    assert!(!tree.managers().drag.is_potential());
    // 活跃拖动同样不得残留。
    assert!(!tree.managers().drag.is_dragging());
    // 键盘焦点必须保持，不能用 WindowBlur 代替 pointer cancel。
    assert_eq!(tree.managers().focus.focused_widget(), Some(target));
    // 结束原生接管手势测试。
}

// 验证 manager 共享目标时只查询一次可见性，隐藏后仍完整清理全部交互状态。
#[test]
fn hidden_interaction_reuses_visible_target_and_rechecks_hidden_focus() {
    let visible = Arc::new(AtomicBool::new(true));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut tree = WidgetTree::new();
    let target = tree.set_root(Box::new(VisibilityProbe {
        visible: Arc::clone(&visible),
        calls: Arc::clone(&calls),
    }));

    tree.managers_mut()
        .interaction
        .set_hovered_widget(Some(target));
    assert!(
        tree.managers_mut()
            .interaction
            .begin_pressed_pointer(Some(target), MouseButton::Left)
    );
    tree.managers_mut().drag.begin_gesture(
        Some(target),
        Point::new(4.0, 5.0),
        MouseButton::Left,
        KeyMod::NONE,
    );
    tree.managers_mut().focus.set_focused_widget(Some(target));

    tree.cancel_hidden_interaction();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(tree.managers().interaction.hovered_widget(), Some(target));
    assert_eq!(tree.managers().interaction.pressed_widget(), Some(target));
    assert_eq!(tree.managers().drag.target(), Some(target));
    assert_eq!(tree.managers().focus.focused_widget(), Some(target));

    visible.store(false, Ordering::Relaxed);
    calls.store(0, Ordering::Relaxed);
    tree.cancel_hidden_interaction();
    // 取消指针回调后焦点必须重新查询，不能复用隐藏前的失败结果。
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert_eq!(tree.managers().interaction.hovered_widget(), None);
    assert_eq!(tree.managers().interaction.pressed_widget(), None);
    assert_eq!(tree.managers().drag.target(), None);
    assert_eq!(tree.managers().focus.focused_widget(), None);
}
// 结束 WidgetTree 指针路由测试模块。
