// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证面板、手风琴、受控 key 集合、事件与公共属性生成契约。
#[test]
fn generates_collapse_contract() {
    // 生成覆盖完整专有属性与自动化标识的折叠组。
    let snapshot = generate(
        // 同名 header 使用显式不同 key。
        r#"<Collapse panels={[CollapsePanel('同名', '甲').key('alpha').expanded(), CollapsePanel('同名', '乙').key('beta')]} accordion={accordion_enabled} activeKeys={active_keys} @change="record_panel($event)" width="360px" automationId="collapse" />"#,
    )
    // 合法折叠组必须成功生成。
    .expect("文档属性应映射到公开 Collapse API");

    // 语言面构造必须映射到公开 CollapsePanel::new。
    assert!(snapshot.contains("CollapsePanel :: new"));
    // 显式 key 与 expanded 构造链必须保留。
    assert!(snapshot.contains("key") && snapshot.contains("expanded"));
    // 动态手风琴配置必须进入布尔构建器。
    assert!(snapshot.contains("accordion_enabled (accordion_enabled)"));
    // 展开集合必须映射到受控状态构建器。
    assert!(snapshot.contains("active_keys (& (active_keys))"));
    // 变化观察器必须复用公开 View Change 注册入口。
    assert!(snapshot.contains("on_change_fn") && snapshot.contains("record_panel"));
    // $event 必须投影为卫生的稳定 key 文本借用。
    assert!(snapshot.contains("__uix_collapse_change"));
    // 公共尺寸与自动化标识仍由统一属性层消费。
    assert!(snapshot.contains("width") && snapshot.contains("automation_id"));
}

// 验证省略受控状态与手风琴时保留运行时非受控默认值。
#[test]
fn generates_uncontrolled_collapse_defaults() {
    // 生成只包含必需面板集合的最小声明。
    let snapshot = generate(r#"<Collapse panels={collapse_panels} />"#)
        // 最小合法声明必须成功生成。
        .expect("缺省 Collapse 应保留非受控运行时行为");

    // 面板集合必须进入公开 panels 构建器。
    assert!(snapshot.contains("Collapse :: new () . panels"));
    // 省略 accordion 时不得生成额外配置。
    assert!(!snapshot.contains("accordion_enabled"));
    // 省略 activeKeys 时不得伪造状态绑定。
    assert!(!snapshot.contains("active_keys"));
    // 省略 Change 时不得注册额外观察器。
    assert!(!snapshot.contains("on_change_fn"));
}

// 验证必需集合、状态表达式与叶节点边界。
#[test]
fn rejects_missing_literal_active_keys_and_children() {
    // 缺失 panels 时没有折叠内容来源。
    let missing = generate(r#"<Collapse />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 panels 必须被拒绝");
    // 诊断必须点名 panels。
    assert!(missing.message.contains("panels"));

    // 单个字符串字面量不能伪装成类型化面板集合。
    let literal = generate(r#"<Collapse panels="甲" />"#)
        // 字面量集合必须失败。
        .expect_err("字面量 Collapse panels 必须被拒绝");
    // 诊断必须说明 Vec<CollapsePanel> 类型要求。
    assert!(literal.message.contains("Vec<CollapsePanel>"));

    // activeKeys 字面量不能提供可写回状态句柄。
    let active = generate(r#"<Collapse panels={collapse_panels} activeKeys="alpha" />"#)
        // 字面量展开集合必须失败。
        .expect_err("字面量 Collapse activeKeys 必须被拒绝");
    // 诊断必须点名 State<Vec<String>> 绑定。
    assert!(active.message.contains("State<Vec<String>>"));

    // 运行时自行物化面板内容，不能接受任意 View 子树。
    let child = generate(r#"<Collapse panels={collapse_panels}><Text>额外节点</Text></Collapse>"#)
        // 嵌套元素必须失败。
        .expect_err("Collapse 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}

// 验证未登记属性不能穿过公共映射。
#[test]
fn rejects_unknown_collapse_attribute() {
    // borderless 尚未进入当前 UIX 专有属性契约。
    let unknown = generate(r#"<Collapse panels={collapse_panels} borderless />"#)
        // 未登记属性必须失败。
        .expect_err("未知 Collapse 属性必须被拒绝");
    // 诊断必须包含具体属性名。
    assert!(unknown.message.contains("borderless"));
}
