// 复用窗口驱动内部契约和测试所需类型。
use super::*;
// 使用只记录命令的 RenderTarget 观测 logical extent。
use crate::draw::painting::recorder::CommandRecorder;
// 使用内存窗口观测 resize_notify 调用。
use crate::native::test_harness::FakeWindow;

// 构造不携带几何的窗口状态事实。
fn lifecycle_event(type_: UiEventType) -> UiEvent {
    // 返回只表达最大化或还原状态的原生事件。
    UiEvent {
        // 单窗口驱动已确定路由目标，事件无需重复携带身份。
        window_id: None,
        // 保存调用方要求的窗口状态事实类型。
        type_,
        // 状态事实不携带客户区尺寸。
        payload: UiEventPayload::None,
    }
}

// 验证最大化与还原只消费状态事实，唯一 resize 来自 WindowResize。
#[test]
fn maximize_restore_keeps_resize_as_single_geometry_authority() {
    // 以 800x600 初始逻辑客户区创建窗口驱动。
    let mut driver = WindowDriver::new(800, 600, false);
    // 创建可接收 full-frame dirty 的空组件树。
    let mut tree = WidgetTree::new();
    // 创建记录 logical extent 的测试渲染目标。
    let mut engine = CommandRecorder::new();
    // 初始化渲染目标到窗口初始尺寸。
    engine.initialize(800, 600).expect("初始化测试渲染目标");
    // 创建记录平台 resize_notify 的测试窗口。
    let mut window = FakeWindow::new(1, "测试窗口", 800, 600);
    // 创建窗口独占的文本输入状态。
    let mut text_input = WindowTextInputState::default();

    // 先投递两次最大化状态事实，模拟重复原生通知。
    for _ in 0..2 {
        // 最大化事实只改变调度和 dirty，不得触发几何事务。
        assert!(driver.handle_window_event(
            &lifecycle_event(UiEventType::WindowMaximize),
            &mut tree,
            &mut engine,
            &mut window,
            &mut text_input,
        ));
    }
    // 最大化事实不能用显示器 bounds 改写渲染目标。
    assert_eq!(engine.logical_extent(), (800, 600));
    // 最大化事实不能重复调用平台 resize_notify。
    assert!(window.state.resize_notify_calls.is_empty());
    // 投递最大化后的权威逻辑客户区 resize。
    let maximized_resize = UiEvent::resize(1920, 1040);
    // WindowResize 独占一次 graphics 与平台 resize 事务。
    assert!(driver.handle_window_event(
        &maximized_resize,
        &mut tree,
        &mut engine,
        &mut window,
        &mut text_input,
    ));
    // 渲染目标采用原生事件给出的当前逻辑客户区尺寸。
    assert_eq!(engine.logical_extent(), (1920, 1040));
    // 平台窗口只接收一次相同尺寸的 resize_notify。
    assert_eq!(window.state.resize_notify_calls, vec![(1920, 1040)]);

    // 再投递两次还原状态事实，覆盖重复最大化切换序列。
    for _ in 0..2 {
        // 还原事实只改变调度和 dirty，不得回放初始尺寸。
        assert!(driver.handle_window_event(
            &lifecycle_event(UiEventType::WindowRestore),
            &mut tree,
            &mut engine,
            &mut window,
            &mut text_input,
        ));
    }
    // 还原事实到权威 resize 到达前必须保留当前 surface extent。
    assert_eq!(engine.logical_extent(), (1920, 1040));
    // 还原事实不得增加平台 resize_notify 次数。
    assert_eq!(window.state.resize_notify_calls, vec![(1920, 1040)]);

    // 投递还原后的权威逻辑客户区 resize。
    let restored_resize = UiEvent::resize(800, 600);
    // WindowResize 再独占一次 geometry/surface 事务。
    assert!(driver.handle_window_event(
        &restored_resize,
        &mut tree,
        &mut engine,
        &mut window,
        &mut text_input,
    ));
    // 渲染目标最终回到原生事件报告的还原尺寸。
    assert_eq!(engine.logical_extent(), (800, 600));
    // 两次权威 resize 各产生且只产生一次平台通知。
    assert_eq!(
        window.state.resize_notify_calls,
        vec![(1920, 1040), (800, 600)]
    );

    // 再覆盖 Wayland 的 resize 先于状态事实顺序。
    let resize_before_maximize = UiEvent::resize(1600, 900);
    // 先到的权威 resize 正常推进第三次几何事务。
    assert!(driver.handle_window_event(
        &resize_before_maximize,
        &mut tree,
        &mut engine,
        &mut window,
        &mut text_input,
    ));
    // 后到的最大化事实不得再次改写已采用的客户区尺寸。
    assert!(driver.handle_window_event(
        &lifecycle_event(UiEventType::WindowMaximize),
        &mut tree,
        &mut engine,
        &mut window,
        &mut text_input,
    ));
    // resize 后状态顺序仍保留权威 logical extent。
    assert_eq!(engine.logical_extent(), (1600, 900));
    // 最大化事实没有增加第四次平台 resize_notify。
    assert_eq!(
        window.state.resize_notify_calls,
        vec![(1920, 1040), (800, 600), (1600, 900)]
    );

    // 覆盖 resize 先于还原事实的反向切换。
    let resize_before_restore = UiEvent::resize(800, 600);
    // 先到的还原尺寸正常推进第四次几何事务。
    assert!(driver.handle_window_event(
        &resize_before_restore,
        &mut tree,
        &mut engine,
        &mut window,
        &mut text_input,
    ));
    // 后到的还原事实不得回放其他历史尺寸。
    assert!(driver.handle_window_event(
        &lifecycle_event(UiEventType::WindowRestore),
        &mut tree,
        &mut engine,
        &mut window,
        &mut text_input,
    ));
    // 最终 logical extent 与最后一个权威 resize 完全一致。
    assert_eq!(engine.logical_extent(), (800, 600));
    // 四次权威 resize 与四次平台通知保持一一对应。
    assert_eq!(
        window.state.resize_notify_calls,
        vec![(1920, 1040), (800, 600), (1600, 900), (800, 600)]
    );
}

// 根 Surface 缩小时必须发布布局失效；副窗口即使不携带 had_layout_event 也能重排子树。
#[test]
fn resize_publishes_root_layout_invalidation() {
    let mut driver = WindowDriver::new(1200, 800, false);
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(crate::ui::widgets::Label::new("响应式内容")));
    let mut engine = CommandRecorder::new();
    engine.initialize(1200, 800).expect("初始化测试渲染目标");
    sync_root_frame_exactly_to_engine(&mut tree, &mut engine);
    tree.layout();
    tree.reset_invalidation();
    assert!(!has_layout_work(&tree));

    let mut window = FakeWindow::new(1, "测试窗口", 1200, 800);
    let mut text_input = WindowTextInputState::default();
    assert!(driver.handle_window_event(
        &UiEvent::resize(640, 480),
        &mut tree,
        &mut engine,
        &mut window,
        &mut text_input,
    ));

    assert_eq!(
        tree.root().expect("根节点存在").frame(),
        Rect::new(0.0, 0.0, 640.0, 480.0)
    );
    assert!(has_layout_work(&tree));
}
