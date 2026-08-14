// 仅在启用无窗口测试驱动时编译私有状态行为验收。
#![cfg(feature = "test-harness")]

// 引入 UIX 编译期入口、自动化标识扩展与公开 View 节点类型。
use uix::prelude::{StyleExt, ViewNode, uix};
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

// 构造同时覆盖条件分支、无关协调与卸载重建的真实 UIX 根。
fn lifecycle_root() -> ViewNode {
    // 让两个组件的私有状态通过实际 If 与事件路径进入根协调。
    uix!(
        r#"
        <Component name="LifecycleCounter" state="count: 0">
          <Column width="120px" gap="4px">
            <Text automationId="lifecycle-counter-value">{count}</Text>
            <Button automationId="lifecycle-counter-button" @click="setState(count: count + 1)">增加</Button>
          </Column>
        </Component>
        <Component name="LifecycleHost" state="alternate: false, unrelated: 0, mounted: true">
          <Column width="280px" height="240px" gap="8px">
            <Button automationId="branch-toggle" @click="setState(alternate: !alternate)">切换分支</Button>
            <If {!alternate}>
              <Text automationId="primary-branch">主分支</Text>
            </If>
            <If {alternate}>
              <Text automationId="alternate-branch">备用分支</Text>
            </If>
            <Button automationId="unrelated-update" @click="setState(unrelated: unrelated + 1)">无关更新</Button>
            <Text automationId="unrelated-value">{unrelated}</Text>
            <Button automationId="mount-toggle" @click="setState(mounted: !mounted)">切换挂载</Button>
            <If {mounted}>
              <LifecycleCounter />
            </If>
          </Column>
        </Component>
        <LifecycleHost />
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

// 验证私有状态驱动条件页，并在无关协调与真实卸载间遵守实例生命周期。
#[test]
fn private_component_state_controls_branches_and_reinitializes_after_remount() {
    // 创建承载条件页与嵌套计数器的无窗口应用。
    let mut app = TestApp::new((320.0, 280.0), lifecycle_root);

    // 初始条件必须只物化主分支。
    assert_eq!(app.text("primary-branch").as_deref(), Ok("主分支"));
    // 未选中的备用分支不得提前进入运行时树。
    assert!(app.text("alternate-branch").is_err());
    // 嵌套计数器必须从私有初值开始。
    assert_eq!(app.text("lifecycle-counter-value").as_deref(), Ok("0"));

    // 更新嵌套计数器以建立需要跨无关根协调保留的非初值。
    app.click("lifecycle-counter-button")
        // 点击必须完成 State 请求的根协调。
        .expect("嵌套计数器点击应完成协调");
    // 存活实例必须显示更新后的值。
    assert_eq!(app.text("lifecycle-counter-value").as_deref(), Ok("1"));

    // 更新宿主的无关字段以触发同一根的另一轮协调。
    app.click("unrelated-update")
        // 无关更新也必须成功收敛。
        .expect("无关状态更新应完成协调");
    // 无关字段必须完成自己的更新。
    assert_eq!(app.text("unrelated-value").as_deref(), Ok("1"));
    // 同轮协调不得重置仍存活的嵌套组件实例。
    assert_eq!(app.text("lifecycle-counter-value").as_deref(), Ok("1"));

    // 用宿主私有状态切换到备用条件分支。
    app.click("branch-toggle")
        // 条件页切换必须完成结构协调。
        .expect("条件分支切换应完成协调");
    // 旧主分支必须已经真实移出运行时树。
    assert!(app.text("primary-branch").is_err());
    // 新备用分支必须成为可观测页面。
    assert_eq!(app.text("alternate-branch").as_deref(), Ok("备用分支"));
    // 再次执行无关协调以验证新分支状态不会回退。
    app.click("unrelated-update")
        // 第二次无关更新必须成功收敛。
        .expect("备用分支后的无关更新应完成协调");
    // 新分支必须跨后续协调继续存在。
    assert_eq!(app.text("alternate-branch").as_deref(), Ok("备用分支"));

    // 关闭挂载条件以真实卸载嵌套计数器实例。
    app.click("mount-toggle")
        // 卸载操作必须完成结构协调。
        .expect("嵌套计数器卸载应完成协调");
    // 已卸载实例不得继续出现在自动化树中。
    assert!(app.text("lifecycle-counter-value").is_err());
    // 重新打开挂载条件以创建新的计数器实例。
    app.click("mount-toggle")
        // 重新挂载必须完成结构协调。
        .expect("嵌套计数器重新挂载应完成协调");
    // 新实例必须重新使用声明初值而不是复活旧值一。
    assert_eq!(app.text("lifecycle-counter-value").as_deref(), Ok("0"));
}
