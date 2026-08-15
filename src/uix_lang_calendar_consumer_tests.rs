// 引入宏生成代码承诺使用的公开 prelude。
use crate::prelude::*;

// 验证 Calendar 两种日期所有权模式与 Change 载荷通过真实公开 API 类型检查。
#[test]
fn calendar_date_contract_compiles_against_public_uix_api() {
    // 创建调用方拥有的受控日期状态。
    let selected_date = State::new(Date::new(2026, 8, 10));
    // 创建接收规范日期文本借用的观察器。
    let on_calendar_change = |_value: &str| {};
    // 展开受控日期、公共尺寸与 Change 事件映射。
    let _controlled: ViewNode = crate::uix!(
        // 使用完整已登记 Calendar UIX 形状。
        r#"<Calendar value={selected_date} @change="on_calendar_change($event)" width="300px" />"#
    );
    // 创建非受控初始日期值。
    let initial_date = Date::new(2026, 9, 3);
    // 展开只建立初值的非受控日期映射。
    let _uncontrolled: ViewNode = crate::uix!(r#"<Calendar defaultDate={initial_date} />"#);
}
