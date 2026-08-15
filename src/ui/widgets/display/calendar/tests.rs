// 引入受测 Calendar 与日期值类型。
use super::{Calendar, Date};
// 引入交互、语义事件与响应式状态测试契约。
use crate::ui::{
    // 引入组件身份与事件分发接口。
    ComponentId,
    EventHandler,
    // 引入键盘输入值。
    KeyCode,
    KeyMod,
    // 引入统一语义事件类型。
    SemanticKind,
    SemanticPayload,
    // 引入受控状态与系统事件。
    State,
    SystemEvent,
};

// 验证受控构造器建立日期、月份与焦点的同一投影。
#[test]
fn controlled_value_initializes_calendar_projection() {
    // 创建调用方拥有的初始日期状态。
    let selected = State::new(Date::new(2026, 8, 10));
    // 用外部状态构造受控日历。
    let calendar = Calendar::new().value(&selected);
    // 完整选中日期必须来自外部状态。
    assert_eq!(calendar.selected_date(), Some(Date::new(2026, 8, 10)));
    // 当前月份必须与受控日期一致。
    assert_eq!(calendar.displayed_month(), (2026, 8));
}

// 验证用户选择先回写状态，再只发布一次 Change。
#[test]
fn controlled_selection_writes_state_before_single_change() {
    // 创建十号受控状态。
    let selected = State::new(Date::new(2026, 8, 10));
    // 创建接受键盘交互的受控日历。
    let mut calendar = Calendar::new().value(&selected);
    // 将焦点从十号移动到十一号。
    let _ = EventHandler::on_event(
        // 借用日历事件处理器。
        &mut calendar,
        // 发送向右移动一天的键盘事件。
        &SystemEvent::KeyDown {
            // 使用右方向键。
            key: KeyCode::Right,
            // 不附加修饰键。
            mods: KeyMod::NONE,
        },
    );
    // 提交当前焦点日期。
    let _ = EventHandler::on_event(
        // 再次借用日历事件处理器。
        &mut calendar,
        // 发送确认键事件。
        &SystemEvent::KeyDown {
            // 使用回车提交。
            key: KeyCode::Enter,
            // 不附加修饰键。
            mods: KeyMod::NONE,
        },
    );
    // 观察事件前，唯一外部状态必须已经更新。
    assert_eq!(selected.get(), Date::new(2026, 8, 11));
    // 取走本次日期变化语义事件。
    let event = calendar
        // 使用稳定测试组件身份读取 Change。
        .semantic_event(ComponentId::new(11), &SystemEvent::FocusIn)
        // 用户提交必须产生事件。
        .expect("受控日期选择应产生 Change");
    // 事件种类必须使用公共 Change 契约。
    assert_eq!(event.kind, SemanticKind::Change);
    // 文本载荷必须使用稳定日期格式。
    assert!(matches!(event.payload, SemanticPayload::Text(value) if value == "2026-08-11"));
    // 同次提交不得重复发布事件。
    assert!(
        // 第二次读取应为空。
        calendar
            // 复用相同组件身份读取队列。
            .semantic_event(ComponentId::new(11), &SystemEvent::FocusIn)
            // 断言事件已经消费。
            .is_none()
    );
}

// 验证外部更新在下一轮声明协调时覆盖本地投影。
#[test]
fn controlled_reconcile_reads_latest_external_date() {
    // 创建初始受控日期。
    let selected = State::new(Date::new(2026, 8, 10));
    // 创建首轮运行时组件。
    let mut calendar = Calendar::new().value(&selected);
    // 调用方把唯一状态切换到九月三号。
    selected.set(Date::new(2026, 9, 3));
    // 模拟声明树下一轮用同一状态句柄协调组件。
    calendar.sync_from(Calendar::new().value(&selected));
    // 选中日期必须更新到外部事实。
    assert_eq!(calendar.selected_date(), Some(Date::new(2026, 9, 3)));
    // 当前月份必须同步更新。
    assert_eq!(calendar.displayed_month(), (2026, 9));
}

// 验证受控值未变化时 reconcile 保留用户独立导航月份。
#[test]
fn unchanged_controlled_value_preserves_navigated_month() {
    // 创建八月十号受控状态。
    let selected = State::new(Date::new(2026, 8, 10));
    // 创建受控日历。
    let mut calendar = Calendar::new().value(&selected);
    // 用户导航到下一个月但不改变选中日期。
    let _ = EventHandler::on_event(
        // 借用日历事件处理器。
        &mut calendar,
        // 发送按月翻页事件。
        &SystemEvent::KeyDown {
            // 使用 PageDown 进入九月。
            key: KeyCode::PageDown,
            // 不附加修饰键。
            mods: KeyMod::NONE,
        },
    );
    // 用户导航必须只改变显示月份。
    assert_eq!(calendar.displayed_month(), (2026, 9));
    // 模拟与日期无关的声明树协调。
    calendar.sync_from(Calendar::new().value(&selected));
    // 未变化的受控值不得把显示月份重置回八月。
    assert_eq!(calendar.displayed_month(), (2026, 9));
    // 选中日期仍由外部状态拥有。
    assert_eq!(calendar.selected_date(), Some(Date::new(2026, 8, 10)));
}

// 验证显式默认显示月份可以独立于初始受控选择。
#[test]
fn default_displayed_month_can_differ_from_controlled_selection() {
    // 创建八月十号受控状态。
    let selected = State::new(Date::new(2026, 8, 10));
    // 在受控选择后显式声明九月初始显示月份。
    let calendar = Calendar::new()
        // 先建立受控选中日期。
        .value(&selected)
        // 再覆盖初始显示月份。
        .default_displayed(2026, 9);
    // 选择仍来自八月外部状态。
    assert_eq!(calendar.selected_date(), Some(Date::new(2026, 8, 10)));
    // 显示月份必须使用显式九月初值。
    assert_eq!(calendar.displayed_month(), (2026, 9));
}

// 验证 default_date 最后调用时切回非受控所有权。
#[test]
fn default_date_after_value_restores_uncontrolled_selection() {
    // 创建不应再被日历写入的旧状态。
    let selected = State::new(Date::new(2026, 8, 10));
    // 最后应用默认日期以显式切回非受控模式。
    let mut calendar = Calendar::new()
        // 先绑定旧状态。
        .value(&selected)
        // 再用默认日期接管组件本地初值。
        .default_date(Date::new(2026, 8, 20));
    // 将本地焦点移动到二十一号。
    let _ = EventHandler::on_event(
        // 借用非受控日历。
        &mut calendar,
        // 发送右方向键。
        &SystemEvent::KeyDown {
            // 向后移动一天。
            key: KeyCode::Right,
            // 不附加修饰键。
            mods: KeyMod::NONE,
        },
    );
    // 提交本地日期。
    let _ = EventHandler::on_event(
        // 再次借用非受控日历。
        &mut calendar,
        // 发送回车键。
        &SystemEvent::KeyDown {
            // 确认当前焦点。
            key: KeyCode::Enter,
            // 不附加修饰键。
            mods: KeyMod::NONE,
        },
    );
    // 非受控选择必须保存在组件本地。
    assert_eq!(calendar.selected_date(), Some(Date::new(2026, 8, 21)));
    // 已清除的旧绑定不得收到新选择。
    assert_eq!(selected.get(), Date::new(2026, 8, 10));
}
