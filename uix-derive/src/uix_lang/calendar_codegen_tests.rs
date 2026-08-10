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

// 验证 Calendar 未声明专有属性保持编译期 Gate。
#[test]
// 声明 Calendar 未知属性测试。
fn rejects_unknown_calendar_attribute() {
    // 当前 UIX 文档没有登记 defaultDate 属性。
    let error = generate(r#"<Calendar defaultDate={selected_date} />"#)
        // 未登记属性必须失败。
        .expect_err("Calendar 未登记属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(error.message.contains("defaultDate"));
}
