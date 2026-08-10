// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Alert 状态、动态关闭能力、关闭事件与公共属性的完整生成契约。
#[test]
// 声明完整 Alert 生成测试。
fn generates_alert_contract() {
    // 生成覆盖动态字符串、布尔值、事件载荷与公共属性的警告提示。
    let snapshot = generate(
        r#"<Alert message={alert_message} type="warning" closable={can_close} @close="record_close($event)" width="320px" automationId="disk-alert" />"#,
    )
    // 合法警告提示必须成功生成。
    .expect("文档属性应映射到公开 Alert API");
    // 动态消息必须以临时借用进入会复制内容的构造器。
    assert!(snapshot.contains("Alert :: new (& * (alert_message))"));
    // 状态必须映射到公开枚举。
    assert!(snapshot.contains("StatusLevel :: Warning"));
    // 动态关闭能力必须保留 bool 类型检查。
    assert!(snapshot.contains("if can_close") && snapshot.contains("closable ()"));
    // 关闭处理器必须接入统一 Change 文本入口。
    assert!(snapshot.contains("on_change_fn"));
    // 处理器必须只观察已建立的关闭事实。
    assert!(snapshot.contains("== \"closed\""));
    // 事件载荷必须传给文档处理器。
    assert!(snapshot.contains("(record_close) (__uix_alert_change)"));
    // 公共宽度与自动化标识仍由公共属性层消费。
    assert!(snapshot.contains("width (320.0)") && snapshot.contains("automation_id"));
}

// 验证 Alert 的状态与关闭能力缺省值。
#[test]
// 声明 Alert 默认值测试。
fn generates_alert_defaults() {
    // 生成只包含必需消息的最小警告提示。
    let snapshot = generate(r#"<Alert message="准备完成" />"#)
        // 最小属性集合必须成功生成。
        .expect("缺省 Alert 属性应使用文档默认值");
    // 缺省状态必须显式固定为信息状态。
    assert!(snapshot.contains("StatusLevel :: Info"));
    // 未声明 closable 时不得启用关闭入口。
    assert!(!snapshot.contains("closable ()"));

    // 布尔简写必须启用公开关闭能力。
    let shorthand = generate(r#"<Alert message="可关闭" closable />"#)
        // 合法简写必须成功生成。
        .expect("closable 简写应映射为 true");
    // 静态 true 必须直接调用关闭能力构建器。
    assert!(shorthand.contains("closable ()"));

    // 显式 false 必须保持运行时默认值。
    let disabled = generate(r#"<Alert message="不可关闭" closable="false" />"#)
        // 合法 false 字面量必须成功生成。
        .expect("closable=false 应保留默认值");
    // 静态 false 不需要生成多余分支或构建调用。
    assert!(!disabled.contains("closable ()"));
}

// 验证 Alert 必需消息、叶节点与类型关键字诊断。
#[test]
// 声明 Alert 核心错误测试。
fn rejects_invalid_alert_core_contracts() {
    // 缺失 message 时没有运行时内容来源。
    let missing = generate(r#"<Alert />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 message 必须被拒绝");
    // 诊断必须点名 message。
    assert!(missing.message.contains("message"));

    // Alert 自身绘制内容，不能接受 View 子树。
    let child = generate(r#"<Alert message="提示"><Text>非法</Text></Alert>"#)
        // 嵌套元素必须失败。
        .expect_err("Alert 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));

    // 未登记状态不能被静默降级为信息提示。
    let status = generate(r#"<Alert message="提示" type="pending" />"#)
        // 非法关键字必须失败。
        .expect_err("未登记 Alert 类型必须被拒绝");
    // 修复建议必须列出全部合法关键字。
    assert!(
        status
            .suggestion
            .contains("info、success、warning 或 error")
    );

    // 动态状态无法确定公开枚举类型。
    let dynamic = generate(r#"<Alert message="提示" type={alert_type} />"#)
        // 动态关键字必须失败。
        .expect_err("动态 Alert 类型必须被拒绝");
    // 诊断必须说明字符串字面量要求。
    assert!(dynamic.message.contains("字符串字面量"));
}

// 验证 Alert 布尔值、专有属性与事件边界。
#[test]
// 声明 Alert 属性与事件错误测试。
fn rejects_invalid_alert_attributes_and_events() {
    // 静态 closable 只接受布尔字面量。
    let closable = generate(r#"<Alert message="提示" closable="yes" />"#)
        // 非布尔值必须失败。
        .expect_err("非法 closable 必须被拒绝");
    // 诊断必须说明布尔值要求。
    assert!(closable.message.contains("布尔"));

    // 未登记普通属性不能被公共映射静默忽略。
    let unknown = generate(r#"<Alert message="提示" dismissible="true" />"#)
        // 未知属性必须失败。
        .expect_err("未知 Alert 属性必须被拒绝");
    // 诊断必须包含具体未知属性名。
    assert!(unknown.message.contains("dismissible"));

    // 未登记事件不能伪装成关闭事件。
    let event = generate(r#"<Alert message="提示" @dismiss="on_dismiss" />"#)
        // 未知事件必须失败。
        .expect_err("未知 Alert 事件必须被拒绝");
    // 诊断必须包含具体未知事件名。
    assert!(event.message.contains("@dismiss"));
}
