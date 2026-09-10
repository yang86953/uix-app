// 仅在公开无窗口测试驱动可用时编译 Tooltip 行内命中的回归验收。
#![cfg(feature = "test-harness")]

// 引入 UIX 编译入口、响应式状态、公开 View 类型与指针事件。
use uix_app::prelude::{KeyMod, MouseButton, Point, State, StyleExt, SystemEvent, ViewNode, uix};
// 引入与真实组件事件和协调路径一致的公开测试驱动。
use uix_app::ui::test_harness::TestApp;

// 按真实窗口路径驱动一次左键点击手势。
//
// 纯语义 @click 节点不消费裸 PointerDown/Up，只有指针路由在释放时
// 合成的语义 Click 才会触发；因此断言依据是状态文本翻转。
fn gesture_click(app: &mut TestApp, automation_id: &str) {
    let node = app
        .snapshot()
        .find(automation_id)
        .expect("automation 节点应存在")
        .clone();
    let pos: Point = node.center().expect("automation 节点应有命中矩形中心");
    app.dispatch_system_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    })
    .expect("PointerDown 应完成分发");
    app.dispatch_system_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    })
    .expect("PointerUp 应完成分发");
}

// 每行的稳定业务键与预算好的自动化标识，避免在文档表达式里拼字符串。
#[derive(Clone)]
struct TipRowSpec {
    key: String,
    tip_id: String,
    trigger_id: String,
    state_id: String,
}

impl TipRowSpec {
    fn new(key: &str) -> Self {
        Self {
            key: key.to_owned(),
            tip_id: format!("tip-{key}"),
            trigger_id: format!("trigger-{key}"),
            state_id: format!("state-{key}"),
        }
    }
}

// 构造 For 行内 Tooltip 包裹触发容器的根工厂。
//
// 与应用侧目录树箭头/行内收藏的落地形态一致：@click 与 automationId
// 都声明在 Tooltip 元素上，命中目标必须停在 Tooltip 自身。
#[allow(non_snake_case)]
fn tip_root(rows: State<Vec<TipRowSpec>>) -> impl Fn() -> ViewNode {
    move || {
        let items = rows.get();
        uix!(
            r#"
            <Widget name="TipRow" props="tipId: String, triggerId: String, stateId: String"
                    state="toggled: bool = false">
              <Column width="64" height="64" align="center" gap="4">
                <Tooltip text="行内提示" automationId={tipId} width="40" height="40"
                         @click="setState(toggled: !toggled)">
                  <Container direction="row" width="40" height="40" automationId={triggerId} />
                </Tooltip>
                <Text automationId={stateId}>{toggled}</Text>
              </Column>
            </Widget>
            <Column width="240" height="400" gap="8">
              <For {row} in {items} key={row.key}>
                <TipRow tipId={row.tip_id} triggerId={row.trigger_id} stateId={row.state_id} />
              </For>
            </Column>
            "#
        )
    }
}

// 静态（非 For）Tooltip 对照组，复刻顶栏图标按钮的既有可用形态。
#[allow(non_snake_case)]
fn static_tip_root() -> impl Fn() -> ViewNode {
    || {
        uix!(
            r#"
            <Widget name="StaticTip" state="toggled: bool = false">
              <Column width="64" height="64" align="center" gap="4">
                <Tooltip text="静态提示" automationId="tip-static" width="40" height="40"
                         @click="setState(toggled: !toggled)">
                  <Container direction="row" width="40" height="40" automationId="tip-static-trigger" />
                </Tooltip>
                <Text automationId="state-static">{toggled}</Text>
              </Column>
            </Widget>
            <Column width="240" height="120">
              <StaticTip />
            </Column>
            "#
        )
    }
}

// 断言 automation 节点的声明尺寸已生效（回归：样式宽度曾对 Tooltip 丢弃，
// 触发区停留主题默认 80×28，横向行内遮挡后继兄弟命中）。
fn assert_frame_size(app: &TestApp, automation_id: &str, width: f32, height: f32) {
    let node = app
        .snapshot()
        .find(automation_id)
        .expect("automation 节点应存在")
        .clone();
    assert!(
        (node.frame.w - width).abs() <= 1.0 && (node.frame.h - height).abs() <= 1.0,
        "{automation_id} 触发区应为 {width}×{height}，实际 {}×{}",
        node.frame.w,
        node.frame.h
    );
}

// ScrollView + For 行（复刻应用侧目录树结构）内的 Tooltip 命中。
#[test]
fn tooltip_inside_scrolled_for_row_is_clickable() {
    fn scroll_root(rows: State<Vec<TipRowSpec>>) -> impl Fn() -> ViewNode {
        move || {
            let items = rows.get();
            uix!(
                r#"
                <Widget name="ScrollTipRow" props="tipId: String, triggerId: String, stateId: String"
                        state="toggled: bool = false">
                  <Column width="64" height="64" align="center" gap="4">
                    <Tooltip text="行内提示" automationId={tipId} width="40" height="42"
                             @click="setState(toggled: !toggled)">
                      <Container direction="row" width="40" height="42" automationId={triggerId} />
                    </Tooltip>
                    <Text automationId={stateId}>{toggled}</Text>
                  </Column>
                </Widget>
                <ScrollView direction="vertical" width="240" height="120">
                  <Column width="240" gap="8">
                    <For {row} in {items} key={row.key}>
                      <ScrollTipRow tipId={row.tip_id} triggerId={row.trigger_id} stateId={row.state_id} />
                    </For>
                  </Column>
                </ScrollView>
                "#
            )
        }
    }
    let rows = State::new(vec![TipRowSpec::new("a"), TipRowSpec::new("b")]);
    let mut app = TestApp::new((240.0, 200.0), scroll_root(rows));
    // 第一行在滚动视口顶部，触发区必须可命中。
    gesture_click(&mut app, "tip-a");
    assert_eq!(app.text("state-a").as_deref(), Ok("true"));
    assert_eq!(app.text("state-b").as_deref(), Ok("false"));
}

// 悬停快路径回归：容器成为悬停目标后，指针移到其 frame 内更深的 Tooltip
// 上必须重新完整命中并交付 PointerEnter（返回 Handled），不能被快路径吞掉。
#[test]
fn pointer_move_retargets_from_container_to_deeper_tooltip() {
    fn deep_tip_root() -> impl Fn() -> ViewNode {
        || {
            uix!(
                r#"
                <Widget name="DeepTip" state="toggled: bool = false">
                  <Column width="64" height="64" align="center">
                    <Tooltip text="深部提示" automationId="tip-deep" width="40" height="40"
                             @click="setState(toggled: !toggled)">
                      <Container direction="row" width="40" height="40" automationId="tip-deep-trigger" />
                    </Tooltip>
                  </Column>
                </Widget>
                <Column width="240" height="400" align="center">
                  <DeepTip />
                </Column>
                "#
            )
        }
    }
    let mut app = TestApp::new((240.0, 400.0), deep_tip_root());
    // 指针先停在下方空白区，悬停目标成为外层 Column（frame 覆盖全窗口）。
    let outside = Point::new(10.0, 300.0);
    app.dispatch_system_event(&SystemEvent::PointerMove {
        pos: outside,
        mods: KeyMod::NONE,
    })
    .expect("首次 PointerMove 应完成分发");
    // 移到 Tooltip 中心：必须重新命中并把悬停事实切到 Tooltip 自身。
    let node = app
        .snapshot()
        .find("tip-deep")
        .expect("automation 节点应存在")
        .clone();
    let center = node.center().expect("Tooltip 应有命中矩形中心");
    app.dispatch_system_event(&SystemEvent::PointerMove {
        pos: center,
        mods: KeyMod::NONE,
    })
    .expect("PointerMove 应完成分发");
    let hovered = app
        .snapshot()
        .find("tip-deep")
        .expect("automation 节点应存在")
        .hovered;
    assert!(
        hovered,
        "容器悬停目标内的更深 Tooltip 必须成为悬停目标（PointerEnter 已交付）"
    );
}

// 静态 Tooltip 的点击命中必须保持可用（既有已验收行为）。
#[test]
fn static_tooltip_click_hits_tooltip_node() {
    let mut app = TestApp::new((240.0, 120.0), static_tip_root());
    assert_frame_size(&app, "tip-static", 40.0, 40.0);
    gesture_click(&mut app, "tip-static");
    assert_eq!(app.text("state-static").as_deref(), Ok("true"));
}

// For 物化行内的 Tooltip 必须成为命中目标，@click 才能触发。
#[test]
fn tooltip_inside_for_row_is_clickable() {
    let rows = State::new(vec![TipRowSpec::new("a"), TipRowSpec::new("b")]);
    let mut app = TestApp::new((240.0, 400.0), tip_root(rows));
    assert_frame_size(&app, "tip-a", 40.0, 40.0);
    gesture_click(&mut app, "tip-a");
    assert_eq!(app.text("state-a").as_deref(), Ok("true"));
    // 第二行互不干扰。
    assert_eq!(app.text("state-b").as_deref(), Ok("false"));
}
