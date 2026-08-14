// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证类型化条目、受控活动 id、变化事件与公共属性生成契约。
#[test]
fn generates_selectable_list_contract() {
    // 生成覆盖完整专有属性与自动化标识的可选中列表。
    let snapshot = generate(
        // active 与 Change 必须共同使用稳定条目 id。
        r#"<SelectableList items={[SelectableItem('alpha', 'Alpha'), SelectableItem('beta', 'Beta')]} active={active_id} @change="record_active($event)" width="320px" automationId="selector" />"#,
    )
    // 合法可选中列表必须成功生成。
    .expect("文档属性应映射到公开 SelectableList API");

    // 类型化集合必须以拥有型 Vec 克隆进入运行时。
    assert!(snapshot.contains("items ((:: std :: vec !"));
    // 语言面构造必须映射到公开 SelectableItem::new。
    assert!(snapshot.contains("SelectableItem :: new"));
    // 活动状态必须映射到稳定 id 受控构建器。
    assert!(snapshot.contains("active_state (& (active_id))"));
    // 变化观察器必须复用公开 View Change 注册入口。
    assert!(snapshot.contains("on_change_fn"));
    // 显式处理器名称必须保留在生成闭包中。
    assert!(snapshot.contains("record_active"));
    // $event 必须投影为卫生的稳定 id 文本借用。
    assert!(snapshot.contains("__uix_selectable_list_change"));
    // 公共尺寸与自动化标识仍由统一属性层消费。
    assert!(snapshot.contains("width") && snapshot.contains("automation_id"));
}

// 验证省略受控状态和观察器时保留运行时非受控默认值。
#[test]
fn generates_uncontrolled_selectable_list_defaults() {
    // 生成只包含必需条目集合的最小声明。
    let snapshot = generate(r#"<SelectableList items={selectable_items} />"#)
        // 最小合法声明必须成功生成。
        .expect("缺省 SelectableList 应保留非受控运行时行为");

    // 条目集合必须进入公开 items 构建器。
    assert!(snapshot.contains("SelectableList :: new () . items"));
    // 省略 active 时不得伪造状态绑定。
    assert!(!snapshot.contains("active_state"));
    // 省略 Change 时不得注册额外观察器。
    assert!(!snapshot.contains("on_change_fn"));
}

// 验证必需集合、状态表达式与叶节点边界。
#[test]
fn rejects_missing_literal_active_and_children() {
    // 缺失 items 时没有列表内容来源。
    let missing = generate(r#"<SelectableList />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 items 必须被拒绝");
    // 诊断必须点名 items。
    assert!(missing.message.contains("items"));

    // 单个字符串字面量不能伪装成类型化条目集合。
    let literal = generate(r#"<SelectableList items="alpha" />"#)
        // 字面量集合必须失败。
        .expect_err("字面量 SelectableList items 必须被拒绝");
    // 诊断必须说明 Vec<SelectableItem> 类型要求。
    assert!(literal.message.contains("Vec<SelectableItem>"));

    // active 字面量不能提供可写回状态句柄。
    let active = generate(r#"<SelectableList items={selectable_items} active="alpha" />"#)
        // 字面量活动值必须失败。
        .expect_err("字面量 SelectableList active 必须被拒绝");
    // 诊断必须点名 State<Option<String>> 绑定。
    assert!(active.message.contains("State<Option<String>>"));

    // 运行时自行绘制行列表，不能接受任意 View 子树。
    let child = generate(
        r#"<SelectableList items={selectable_items}><Text>额外节点</Text></SelectableList>"#,
    )
    // 嵌套元素必须失败。
    .expect_err("SelectableList 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
}

// 验证未登记属性不能穿过公共映射。
#[test]
fn rejects_unknown_selectable_list_attribute() {
    // 索引初值不属于 UIX 稳定 id 契约。
    let unknown = generate(r#"<SelectableList items={selectable_items} activeIndex="1" />"#)
        // 未登记属性必须失败。
        .expect_err("未知 SelectableList 属性必须被拒绝");
    // 诊断必须包含具体属性名。
    assert!(unknown.message.contains("activeIndex"));
}
