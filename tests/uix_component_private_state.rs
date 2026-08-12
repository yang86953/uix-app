// 仅在启用无窗口测试驱动时编译私有状态行为验收。
#![cfg(feature = "test-harness")]

// 引入 UIX 编译期入口、自动化标识扩展与公开 View 节点类型。
use uix::prelude::{uix, StyleExt, ViewNode};
// 引入与真实组件事件路径一致的无窗口应用驱动。
use uix::ui::test_harness::TestApp;

// 通过同一个宏调用点构建两个静态 Counter 实例。
fn counter_root() -> ViewNode {
    // 让真实 UIX Component 私有 state、事件与文本插值进入运行时协调路径。
    uix!(
        r#"
        <Component name="Counter" props="valueId: String, buttonId: String" state="count: 0">
          <Column width="120px" gap="4px">
            <Text automationId={valueId}>{count}</Text>
            <Button automationId={buttonId} @click="setState(count: count + 1)">增加</Button>
          </Column>
        </Component>
        <Column width="280px" height="120px" gap="12px">
          <Counter valueId="counter-a-value" buttonId="counter-a-button" />
          <Counter valueId="counter-b-value" buttonId="counter-b-button" />
        </Column>
        "#
    )
}

// 验证 reconcile 保留私有槽、静态实例隔离且不同树不共享所有权。
#[test]
fn private_component_state_survives_reconcile_and_stays_tree_local() {
    // 创建第一棵树作为发生交互的窗口会话替身。
    let mut first = TestApp::new((320.0, 180.0), counter_root);
    // 创建第二棵树并复用完全相同的宏调用点。
    let second = TestApp::new((320.0, 180.0), counter_root);

    // 首次点击 A 实例并等待 State 请求的 reconcile 完成。
    first
        // 通过真实自动化语义入口派发指针按下与抬起。
        .click("counter-a-button")
        // 点击与协调必须成功收敛。
        .expect("A 实例第一次点击应完成协调");
    // A 实例应显示第一次更新后的值。
    assert_eq!(first.text("counter-a-value").as_deref(), Ok("1"));
    // 同树的 B 实例不得读取 A 的私有状态。
    assert_eq!(first.text("counter-b-value").as_deref(), Ok("0"));

    // 再次点击 A 以验证第二轮工厂重建仍复用同一状态槽。
    first
        // 再次走相同事件与协调路径。
        .click("counter-a-button")
        // 第二轮也必须成功收敛。
        .expect("A 实例第二次点击应完成协调");
    // A 实例必须从一累加到二而不是回到初值。
    assert_eq!(first.text("counter-a-value").as_deref(), Ok("2"));

    // 单独点击 B 以确认两个静态调用拥有不同实例身份。
    first
        // 只向 B 实例派发点击。
        .click("counter-b-button")
        // B 的更新必须成功收敛。
        .expect("B 实例点击应完成协调");
    // A 的既有值不得被 B 的更新覆盖。
    assert_eq!(first.text("counter-a-value").as_deref(), Ok("2"));
    // B 应只更新自己的私有状态。
    assert_eq!(first.text("counter-b-value").as_deref(), Ok("1"));

    // 第二棵树必须仍保持两个实例各自的初始状态。
    assert_eq!(second.text("counter-a-value").as_deref(), Ok("0"));
    // 同一宏调用点也不得让 B 的状态跨树泄漏。
    assert_eq!(second.text("counter-b-value").as_deref(), Ok("0"));
}
