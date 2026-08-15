// 引入宏生成代码承诺使用的公开 prelude。
use crate::prelude::*;

// 提供 Calendar UIX 禁用日期策略的真实公开函数项。
fn is_disabled_date(date: Date) -> bool {
    // 每月一号作为确定的禁用日期样例。
    date.day == 1
}

// 验证 Calendar 两种日期所有权模式与 Change 载荷通过真实公开 API 类型检查。
#[test]
fn calendar_date_contract_compiles_against_public_uix_api() {
    // 创建调用方拥有的受控日期状态。
    let selected_date = State::new(Date::new(2026, 8, 10));
    // 创建接收规范日期文本借用的观察器。
    let on_calendar_change = |_value: &str| {};
    // 创建独立于选中日期的初始显示月份。
    let displayed_date = Date::new(2026, 9, 1);
    // 创建类型化事件标记集合。
    let calendar_events = vec![
        // 为九月三号添加公开日历事件。
        CalendarEvent::new(Date::new(2026, 9, 3), "发布日", Color::BLUE),
    ];
    // 提供动态日期格尺寸表达式。
    let calendar_cell_size = 32.0_f32;
    // 提供动态整年跳转策略。
    let jump_by_year = true;
    // 展开受控日期、独立月份、策略、数据、公共尺寸与 Change 事件映射。
    let _controlled: ViewNode = crate::uix!(
        // 使用完整已登记 Calendar UIX 形状。
        r#"<Calendar value={selected_date} defaultDisplayed={displayed_date} cellSize={calendar_cell_size} yearJump={jump_by_year} events={calendar_events} disabledDate={is_disabled_date} @change="on_calendar_change($event)" width="300px" />"#
    );
    // 创建非受控初始日期值。
    let initial_date = Date::new(2026, 9, 3);
    // 展开只建立初值的非受控日期映射。
    let _uncontrolled: ViewNode = crate::uix!(r#"<Calendar defaultDate={initial_date} />"#);
}
