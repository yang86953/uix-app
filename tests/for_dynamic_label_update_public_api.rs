// 复现 admin/uix-app#91：For 数据更新后行内 prop 插值 dynamic 文本不渲染。
// 首屏同结构正常；同 key 改值与整组替换（范围切换）后文本仍在语义树但
// visible_bounds 为 None 或行盒压扁。修复后本测试必须全绿。
#![cfg(feature = "test-harness")]

// 引入 UIX 编译入口、响应式状态与公开 View 类型。
use uix_app::prelude::{State, ViewNode, uix};
// 引入与真实组件事件和协调路径一致的公开测试驱动。
use uix_app::ui::test_harness::TestApp;

// 保存看板卡片的最小投影字段：稳定业务键与可变标题。
#[derive(Clone)]
// 该值只作为 UIX For 的拥有型迭代项。
struct CardSpec {
    // 保存 keyed For 使用的稳定业务键。
    key: String,
    // 保存卡片标题（首屏后可被数据更新改写）。
    title: String,
}

// 为测试卡片提供确定性构造入口。
impl CardSpec {
    // 从短业务键与标题构造卡片值。
    fn new(key: &str, title: &str) -> Self {
        Self {
            key: key.to_owned(),
            title: title.to_owned(),
        }
    }
}

// 构造读取外部卡片集合并生成 keyed 卡片实例的根工厂。
#[allow(non_snake_case)]
// UIX Widget 与 camelCase prop 进入卫生局部名，测试只消费语言规范名称。
fn board_root(cards: State<Vec<CardSpec>>) -> impl Fn() -> ViewNode {
    // 每次 reconcile 都从同一响应式集合取得当前拥有型快照。
    move || {
        // 读取集合同时登记根 View 的结构性响应式依赖。
        let items = cards.get();
        // 让真实 UIX Widget、For 与 prop 插值进入公开宏路径。
        uix!(
            r#"
            <Widget name="TaskCard" props="cardId: String, title: String">
              <Column width="220px" height="64px" gap="8px">
                <Text automationId={cardId}>{title}</Text>
              </Column>
            </Widget>
            <Column width="260px" height="220px" gap="8px">
              <For {card} in {items} key={card.key}>
                <TaskCard cardId={card.key} title={card.title} />
              </For>
            </Column>
            "#
        )
    }
}

// 读取卡片文本节点及其可见边界，缺失时返回 (文本, 边界) 的失败描述。
fn card_visible_text(app: &TestApp, card_id: &str) -> (Option<String>, bool) {
    // 快照遍历真实协调后的无障碍树。
    let snapshot = app.snapshot();
    // 在节点集合中定位目标自动化标识。
    for node in &snapshot.nodes {
        // 以自动化标识精确匹配卡片标题节点。
        if node.automation_id.as_deref() == Some(card_id) {
            // 返回语义文本与可见边界存在性。
            return (
                node.accessibility.name.clone(),
                node.visible_bounds.is_some(),
            );
        }
    }
    // 节点不存在时返回完全缺失。
    (None, false)
}

// 验证 For 行的 prop 插值标题在数据更新后仍然渲染。
#[test]
fn for_row_prop_text_survives_data_update() {
    // 创建最初只含 A 卡的看板集合。
    let cards = State::new(vec![CardSpec::new("a", "首屏标题")]);
    // 创建复用同一卡片集合的公开无窗口应用。
    let mut app = TestApp::new((320.0, 280.0), board_root(cards.clone()));

    // 首屏卡片必须渲染标题文本。
    assert_eq!(app.text("a").as_deref(), Ok("首屏标题"));
    // 首屏标题必须有可见边界。
    let (name, visible) = card_visible_text(&app, "a");
    assert_eq!(name.as_deref(), Some("首屏标题"));
    assert!(visible, "首屏标题必须可见");

    // 同 key 数据更新：标题改写（保存标题场景）。
    cards.set(vec![CardSpec::new("a", "更新后标题")]);
    // 执行由集合状态请求的声明树协调。
    app.settle().expect("同 key 更新应完成协调");
    // 更新后的文本必须可见且为新值。
    let (name, visible) = card_visible_text(&app, "a");
    assert_eq!(name.as_deref(), Some("更新后标题"), "语义文本应更新");
    assert!(visible, "同 key 更新后标题必须可见（uix-app#91）");

    // 整组替换：不同 key 的新卡片（范围切换场景）。
    cards.set(vec![CardSpec::new("b", "范围切换标题")]);
    // 完成整组替换事务。
    app.settle().expect("整组替换应完成协调");
    // 新 key 卡片必须渲染新标题。
    let (name, visible) = card_visible_text(&app, "b");
    assert_eq!(name.as_deref(), Some("范围切换标题"));
    assert!(visible, "整组替换后新卡标题必须可见（uix-app#91）");

    // 混合更新：保留旧卡并追加新卡。
    cards.set(vec![
        CardSpec::new("b", "范围切换标题"),
        CardSpec::new("c", "追加标题"),
    ]);
    // 完成追加事务。
    app.settle().expect("追加行应完成协调");
    // 追加卡与存活卡的标题都必须可见。
    for (card_id, expected) in [("b", "范围切换标题"), ("c", "追加标题")] {
        let (name, visible) = card_visible_text(&app, card_id);
        assert_eq!(name.as_deref(), Some(expected), "卡片 {card_id} 文本应正确");
        assert!(visible, "卡片 {card_id} 标题必须可见（uix-app#91）");
    }
}

// 构造双层条件中的动态文本，覆盖隐藏、更新并重新插入后的重新测量。
fn nested_conditional_text_root(
    outer: State<bool>,
    inner: State<bool>,
    text: State<String>,
) -> impl Fn() -> ViewNode {
    move || {
        uix!(
            r#"
            <Widget name="NestedConditionalText" reactive props="outer: State<bool>, inner: State<bool>, message: State<String>">
              <Column width="300px" gap="8px" automationId="nested.root">
                <If {outer}>
                  <Column width="280px" gap="4px" automationId="nested.outer">
                    <Text>固定行</Text>
                    <If {inner}>
                      <Column width="260px" automationId="nested.inner">
                        <Text automationId="nested.dynamic">{message}</Text>
                      </Column>
                    </If>
                  </Column>
                </If>
              </Column>
            </Widget>
            <NestedConditionalText outer={outer} inner={inner} message={text} />
            "#
        )
    }
}

// 验证嵌套条件重新插入动态文本后使用新内容的多行固有尺寸。
#[test]
fn nested_conditional_dynamic_text_remeasures_after_reinsert() {
    let outer = State::new(true);
    let inner = State::new(true);
    let text = State::new("单行".to_owned());
    let mut app = TestApp::new(
        (320.0, 280.0),
        nested_conditional_text_root(outer.clone(), inner.clone(), text.clone()),
    );

    let initial = app
        .snapshot()
        .find("nested.dynamic")
        .expect("首屏动态文本应存在")
        .frame;
    let initial_inner = app
        .snapshot()
        .find("nested.inner")
        .expect("首屏内层布局应存在")
        .frame;
    let initial_outer = app
        .snapshot()
        .find("nested.outer")
        .expect("首屏外层布局应存在")
        .frame;
    assert!(initial.h > 0.0);

    text.set("第一行\n第二行\n第三行".to_owned());
    app.settle().expect("可见动态文本更新应完成重新测量");
    let measured = app
        .snapshot()
        .find("nested.dynamic")
        .expect("更新后的动态文本应存在")
        .frame;
    let measured_inner = app
        .snapshot()
        .find("nested.inner")
        .expect("更新后的内层布局应存在")
        .frame;
    assert!(measured.h > initial.h * 2.5);
    assert!(measured_inner.h > initial_inner.h * 2.5);

    inner.set(false);
    app.settle().expect("内层条件隐藏应完成协调");
    assert!(app.snapshot().find("nested.dynamic").is_err());
    let hidden_outer = app
        .snapshot()
        .find("nested.outer")
        .expect("保留固定兄弟后的外层布局应存在")
        .frame;
    assert!(hidden_outer.h < initial_outer.h);

    text.set("重新插入".to_owned());
    inner.set(true);
    app.settle().expect("内层条件重新显示应完成协调与布局");
    let reinserted = app
        .snapshot()
        .find("nested.dynamic")
        .expect("重新插入的动态文本应存在")
        .frame;
    let reinserted_inner = app
        .snapshot()
        .find("nested.inner")
        .expect("重新插入后的内层布局应存在")
        .frame;
    let reinserted_outer = app
        .snapshot()
        .find("nested.outer")
        .expect("重新插入后的外层布局应存在")
        .frame;
    assert_eq!(app.text("nested.dynamic").as_deref(), Ok("重新插入"));
    assert!((reinserted.h - initial.h).abs() < 0.01);
    assert!((reinserted_inner.h - initial_inner.h).abs() < 0.01);
    assert!((reinserted_outer.h - initial_outer.h).abs() < 0.01);

    text.set("第一行\n第二行\n第三行\n第四行".to_owned());
    app.settle().expect("重新插入后的动态文本应再次测量");
    let restored = app
        .snapshot()
        .find("nested.dynamic")
        .expect("二次更新后的动态文本应存在")
        .frame;
    assert!(
        restored.h > initial.h * 3.5,
        "四行文本高度应显著大于首屏单行高度: initial={initial:?}, restored={restored:?}"
    );

    outer.set(false);
    app.settle().expect("外层条件隐藏应完成协调");
    outer.set(true);
    app.settle().expect("外层条件恢复应完成协调与布局");
    let second_restore = app
        .snapshot()
        .find("nested.dynamic")
        .expect("双层条件恢复后的动态文本应存在")
        .frame;
    assert!((second_restore.h - restored.h).abs() < 0.01);
}
