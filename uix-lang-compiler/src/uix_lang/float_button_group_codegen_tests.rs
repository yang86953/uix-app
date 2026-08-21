// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证静态子按钮、标准点击事件、触发方式与拒绝路径的完整契约。
#[test]
fn generates_float_button_group_and_rejects_unstable_direct_children() {
    // 生成两项源码顺序稳定且分别使用普通点击与事件载荷的浮动按钮组。
    let snapshot = generate(r#"<FloatButtonGroup trigger="click" automationId="actions"><FloatButton icon="edit" @click="edit" /><FloatButton icon="share" @click="share($event)" /></FloatButtonGroup>"#)
        // 合法组必须生成公开运行时组合。
        .expect("静态 FloatButton 子项应生成公开组 View");
    // 父组件必须调用保留完整子 ViewNode 的窄入口。
    assert!(snapshot.contains("FloatButtonGroup :: new") && snapshot.contains("button_views"));
    // 文档 click 必须映射为父组件展开触发方式。
    assert!(snapshot.contains("TriggerMode :: Click"));
    // 两个子项必须保持源码顺序。
    assert!(snapshot.find("\"edit\"") < snapshot.find("\"share\""));
    // 普通点击与读取事件载荷的点击必须分别保留标准注册入口。
    assert!(snapshot.contains("on_click_fn") && snapshot.contains("on_click_event"));
    // 组自身公共自动化身份必须在包装器物化后应用。
    assert!(snapshot.contains("automation_id"));

    // 直接 If 的运行时基数不稳定，必须给出明确诊断。
    let dynamic = generate(
        r#"<FloatButtonGroup><If {show}><FloatButton icon="edit" /></If></FloatButtonGroup>"#,
    )
    // 动态直接子项当前不允许生成。
    .expect_err("直接 If 必须被拒绝");
    // 诊断必须点名动态控制流边界。
    assert!(dynamic.message.contains("If 或 For"));
    // 非 FloatButton 直接根不能被静默接受。
    let wrong = generate(r#"<FloatButtonGroup><Button>编辑</Button></FloatButtonGroup>"#)
        // 错误组件类型必须失败。
        .expect_err("直接 Button 必须被拒绝");
    // 诊断必须点名实际非法标签。
    assert!(wrong.message.contains("Button"));
    // 未登记触发方式不能回退到默认值。
    let trigger = generate(
        r#"<FloatButtonGroup trigger="focus"><FloatButton icon="edit" /></FloatButtonGroup>"#,
    )
    // 非 click/hover 关键字必须失败。
    .expect_err("未登记 trigger 必须被拒绝");
    // 诊断必须点名非法 trigger。
    assert!(trigger.message.contains("trigger"));
}
