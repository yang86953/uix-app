// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Calendar 默认运行时契约与公共属性生成。
#[test]
// 声明 Calendar 正向生成测试。
fn generates_calendar_contract() {
    // 生成带公共尺寸、样式与自动化标识的日历。
    let snapshot = generate(
        r#"<Calendar width="308px" height="286px" style="padding: 4px;" automationId="monthly-calendar" />"#,
    )
    // 当前文档化形状必须成功生成。
    .expect("Calendar 应映射到公开默认构造器");
    // 日历必须从公开构造器开始。
    assert!(snapshot.contains("Calendar :: new ()"));
    // 组件必须物化为公开叶节点。
    assert!(snapshot.contains("ViewNode :: leaf"));
    // 公共尺寸、样式与自动化标识必须继续映射。
    assert!(
        snapshot.contains("width")
            && snapshot.contains("height")
            && snapshot.contains("padding")
            && snapshot.contains("automation_id")
    );
}

// 验证 Calendar 不静默丢弃可渲染子树。
#[test]
// 声明 Calendar 叶节点错误测试。
fn rejects_calendar_children() {
    // 日历日期格由运行时组件拥有，UIX 子节点没有已登记槽位。
    let error = generate(r#"<Calendar><Text>非法</Text></Calendar>"#)
        // 嵌套元素必须失败。
        .expect_err("Calendar 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(error.message.contains("不接受子节点"));
}

// 验证 Calendar 默认日期映射到非受控构造器。
#[test]
// 声明默认日期正向生成测试。
fn generates_calendar_default_date() {
    // 生成引用调用方 Date 值的日历。
    let snapshot = generate(r#"<Calendar defaultDate={initial_date} />"#)
        // 已登记默认日期必须成功生成。
        .expect("Calendar defaultDate 应映射到公开构造器");
    // 生成物必须调用非受控默认日期入口。
    assert!(snapshot.contains("default_date"));
    // 生成物必须保留调用方表达式。
    assert!(snapshot.contains("initial_date"));
}

// 验证 Calendar 受控值与 Change 事件共同生成。
#[test]
// 声明受控日期正向生成测试。
fn generates_calendar_controlled_change_contract() {
    // 生成受控日期与带载荷处理器。
    let snapshot =
        generate(r#"<Calendar value={selected_date} @change="on_calendar_change($event)" />"#)
            // 已登记受控形状必须成功生成。
            .expect("Calendar value 与 @change 应映射到公开契约");
    // 生成物必须调用受控值构造器。
    assert!(snapshot.contains("value"));
    // 生成物必须注册统一 Change 观察器。
    assert!(snapshot.contains("on_change_fn"));
    // 处理器必须取得卫生事件载荷。
    assert!(snapshot.contains("__uix_calendar_change"));
}

// 验证 Calendar 非节点配置完整映射到公开运行时构造器。
#[test]
// 声明 Calendar 配置正向生成测试。
fn generates_calendar_non_node_configuration() {
    // 生成显示月份、导航、尺寸、事件、禁用策略与日期格工厂配置。
    let snapshot = generate(
        r#"<Calendar defaultDisplayed={displayed_date} cellSize="32px" yearJump={jump_by_year} events={calendar_events} disabledDate={is_disabled_date} dateCell={calendar_date_cell} />"#,
    )
    // 全部已登记配置必须成功生成。
    .expect("Calendar 非节点配置应映射到公开构造器");
    // 默认显示日期必须单次求值后映射年月。
    assert!(
        snapshot.contains("__uix_calendar_displayed") && snapshot.contains("default_displayed")
    );
    // 日期格尺寸必须调用公开 cell_size。
    assert!(snapshot.contains("cell_size"));
    // 整年导航必须调用公开 year_jump。
    assert!(snapshot.contains("year_jump"));
    // 事件集合必须收集为公开 CalendarEvent 向量。
    assert!(snapshot.contains("CalendarEvent") && snapshot.contains("events"));
    // 禁用策略必须调用公开 disabled_date。
    assert!(snapshot.contains("disabled_date") && snapshot.contains("is_disabled_date"));
    // 日期格工厂必须调用公开 date_cell 并保留调用方函数。
    assert!(snapshot.contains("date_cell") && snapshot.contains("calendar_date_cell"));
}

// 验证默认日期与受控值不能形成两个状态所有者。
#[test]
// 声明 Calendar 所有权冲突测试。
fn rejects_calendar_default_and_controlled_conflict() {
    // 同时声明两种模式。
    let error = generate(r#"<Calendar defaultDate={initial_date} value={selected_date} />"#)
        // 所有权冲突必须在编译期失败。
        .expect_err("Calendar 两种日期所有权模式必须互斥");
    // 诊断必须点明两个冲突属性。
    assert!(error.message.contains("defaultDate") && error.message.contains("value"));
}

// 验证 Calendar 状态属性拒绝不能类型检查的字面量。
#[test]
// 声明 Calendar 字面量形状错误测试。
fn rejects_calendar_date_literals() {
    // 默认日期字符串不提供 Date 值。
    let default_error = generate(r#"<Calendar defaultDate="2026-08-10" />"#)
        // 默认日期字面量必须失败。
        .expect_err("Calendar defaultDate 字面量必须被拒绝");
    // 诊断必须说明 Date 表达式要求。
    assert!(default_error.message.contains("Date 表达式"));
    // value 字符串不提供 State<Date> 所有权。
    let value_error = generate(r#"<Calendar value="2026-08-10" />"#)
        // 受控值字面量必须失败。
        .expect_err("Calendar value 字面量必须被拒绝");
    // 诊断必须说明 State<Date> 要求。
    assert!(value_error.message.contains("State<Date>"));
}

// 验证 Calendar 类型化配置拒绝字符串伪装的数据与策略。
#[test]
// 声明 Calendar 配置形状错误测试。
fn rejects_calendar_configuration_literals() {
    // 默认显示月份字符串不提供 Date 类型。
    let displayed_error = generate(r#"<Calendar defaultDisplayed="2026-09" />"#)
        // 非 Date 字面量必须失败。
        .expect_err("Calendar defaultDisplayed 字面量必须被拒绝");
    // 诊断必须说明 Date 表达式要求。
    assert!(displayed_error.message.contains("Date 表达式"));
    // 事件字符串不提供 CalendarEvent 集合。
    let events_error = generate(r#"<Calendar events="holiday" />"#)
        // 非集合字面量必须失败。
        .expect_err("Calendar events 字面量必须被拒绝");
    // 诊断必须说明 CalendarEvent 集合要求。
    assert!(events_error.message.contains("CalendarEvent"));
    // 禁用日期字符串不提供判定函数。
    let disabled_error = generate(r#"<Calendar disabledDate="weekend" />"#)
        // 非函数字面量必须失败。
        .expect_err("Calendar disabledDate 字面量必须被拒绝");
    // 诊断必须说明函数类型要求。
    assert!(disabled_error.message.contains("Fn(Date) -> bool"));
    // 日期格字符串不提供类型化 View 工厂。
    let date_cell_error = generate(r#"<Calendar dateCell="compact" />"#)
        // 非工厂字面量必须失败。
        .expect_err("Calendar dateCell 字面量必须被拒绝");
    // 诊断必须说明日期、上下文与 View 返回契约。
    assert!(
        date_cell_error.message.contains("Date")
            && date_cell_error.message.contains("CalendarCellInfo")
            && date_cell_error.message.contains("View")
    );
    // 非数值格子尺寸不能进入运行时归一化。
    let cell_size_error = generate(r#"<Calendar cellSize="large" />"#)
        // 非数值字面量必须失败。
        .expect_err("Calendar cellSize 非数值字面量必须被拒绝");
    // 诊断必须说明 f32 数值映射要求。
    assert!(cell_size_error.message.contains("f32"));
    // 非布尔整年策略不能静默视为 true。
    let year_jump_error = generate(r#"<Calendar yearJump="sometimes" />"#)
        // 非布尔字面量必须失败。
        .expect_err("Calendar yearJump 非布尔字面量必须被拒绝");
    // 诊断必须说明布尔值要求。
    assert!(year_jump_error.message.contains("布尔值"));
}

// 验证 Calendar 未登记属性继续保持编译期 Gate。
#[test]
// 声明 Calendar 未知属性测试。
fn rejects_unknown_calendar_attribute() {
    // calendarTone 不属于 Calendar 公开配置矩阵。
    let error = generate(r#"<Calendar calendarTone="quiet" />"#)
        // 未登记属性必须失败。
        .expect_err("Calendar 未登记属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(error.message.contains("calendarTone"));
}
