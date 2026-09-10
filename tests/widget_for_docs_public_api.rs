// 仅在公开无窗口测试驱动可用时编译 Widget 逐实例的外部 API 验收。
#![cfg(feature = "test-harness")]

// 引入 UIX 编译入口、响应式状态、样式扩展与公开 View 类型。
use uix_app::prelude::{State, StyleExt, ViewNode, uix};
// 引入与真实组件事件和协调路径一致的公开测试驱动。
use uix_app::ui::test_harness::TestApp;

// 保存测试列表中一行的稳定业务身份与自动化入口。
#[derive(Clone)]
// 该值只作为 UIX For 的拥有型迭代项。
struct RowSpec {
    // 保存 keyed For 使用的稳定业务键。
    key: String,
    // 保存当前行状态文本的自动化标识。
    value_id: String,
    // 保存当前行更新按钮的自动化标识。
    button_id: String,
}

// 为测试行提供确定性的身份构造入口。
impl RowSpec {
    // 从短业务键构造完整测试行。
    fn new(key: &str) -> Self {
        // 返回字段互不混淆的拥有型行值。
        Self {
            // 复制业务键供重排和重新插入复用。
            key: key.to_owned(),
            // 为状态文本生成稳定自动化标识。
            value_id: format!("row-{key}-value"),
            // 为更新按钮生成稳定自动化标识。
            button_id: format!("row-{key}-button"),
        }
    }
}

// 构造读取外部行集合并生成 keyed Widget 实例的根工厂。
#[allow(non_snake_case)]
// UIX Widget 与 camelCase prop 会进入卫生局部名，测试只消费语言规范名称。
fn row_root(rows: State<Vec<RowSpec>>) -> impl Fn() -> ViewNode {
    // 每次 reconcile 都从同一响应式集合取得当前拥有型快照。
    move || {
        // 读取集合同时登记根 View 的结构性响应式依赖。
        let items = rows.get();
        // 让真实 UIX Widget、For、私有状态和事件进入公开宏路径。
        uix!(
            r#"
            <Widget name="StatefulRow" props="valueId: String, buttonId: String" state="count: 0">
              <Column width="180px" height="56px" gap="4px">
                <Text automationId={valueId}>{count}</Text>
                <Button automationId={buttonId} @click="setState(count: count + 1)">增加</Button>
              </Column>
            </Widget>
            <Column width="220px" height="180px" gap="8px">
              <For {item} in {items} key={item.key}>
                <StatefulRow valueId={item.value_id} buttonId={item.button_id} />
              </For>
            </Column>
            "#
        )
    }
}

// 验证 keyed For 重排保留各自状态，移除后释放并在重插时重新初始化。
#[test]
fn keyed_for_widget_state_follows_business_identity_and_lifecycle() {
    // 创建最初按 A、B 排列的业务行集合。
    let rows = State::new(vec![RowSpec::new("a"), RowSpec::new("b")]);
    // 创建复用同一行集合的公开无窗口应用。
    let mut app = TestApp::new((260.0, 220.0), row_root(rows.clone()));

    // 只点击 A 行以建立可辨识的私有状态。
    app.click("row-a-button")
        // 点击与私有状态触发的协调都必须完成。
        .expect("A 行点击应完成协调");
    // A 行必须显示自己的更新值。
    assert_eq!(app.text("row-a-value").as_deref(), Ok("1"));
    // B 行不得共享 A 行的私有状态槽。
    assert_eq!(app.text("row-b-value").as_deref(), Ok("0"));

    // 交换业务行顺序但保留相同 key。
    rows.set(vec![RowSpec::new("b"), RowSpec::new("a")]);
    // 执行由集合状态请求的声明树协调。
    app.settle()
        // keyed 重排必须在有限轮次内收敛。
        .expect("keyed 行重排应完成协调");
    // A 行移动后仍必须保留原私有状态。
    assert_eq!(app.text("row-a-value").as_deref(), Ok("1"));
    // B 行移动后仍保持自己的初始状态。
    assert_eq!(app.text("row-b-value").as_deref(), Ok("0"));

    // 从实际声明树中完全移除 A 行。
    rows.set(vec![RowSpec::new("b")]);
    // 完成移除事务并触发失活作用域清理。
    app.settle()
        // 行移除必须正常收敛。
        .expect("A 行移除应完成协调");
    // 存活的 B 行不得受另一实例卸载影响。
    assert_eq!(app.text("row-b-value").as_deref(), Ok("0"));

    // 使用相同业务 key 重新插入已经卸载的 A 行。
    rows.set(vec![RowSpec::new("a"), RowSpec::new("b")]);
    // 完成重新插入后的声明树协调。
    app.settle()
        // 重新插入必须正常收敛。
        .expect("A 行重新插入应完成协调");
    // 已卸载实例的旧私有状态必须已经释放并重新初始化。
    assert_eq!(app.text("row-a-value").as_deref(), Ok("0"));
    // 始终存活的 B 行继续保持原有私有状态。
    assert_eq!(app.text("row-b-value").as_deref(), Ok("0"));
}
