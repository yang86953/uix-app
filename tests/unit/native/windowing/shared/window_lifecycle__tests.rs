// 复用被测 helper、事件类型和同步容器。
use super::*;

// 构造一个已经中毒的窗口事件队列。
fn poisoned_event_queue() -> Arc<Mutex<VecDeque<UiEvent>>> {
    // 创建与生产 callback 相同形状的共享队列。
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    // 给测试线程克隆同一 mutex owner。
    let worker_queue = queue.clone();
    // 持锁 panic 是标准的 mutex poison 触发方式。
    let worker = std::thread::spawn(move || {
        // 在 panic 前取得唯一队列锁。
        let _guard = worker_queue
            // 锁定刚创建且尚未中毒的测试 mutex。
            .lock()
            // 初次锁定必须成功。
            .expect("fresh event queue mutex must lock");
        // 模拟 callback producer 在持锁阶段 panic。
        panic!("poison event queue for callback delivery test");
    });
    // 测试明确期待 worker panic，而不是让 panic 传播到当前线程。
    assert!(worker.join().is_err());
    // 返回可触发 typed delivery failure 的中毒队列。
    queue
}

// close event 的队列锁失败必须成为稳定 InvalidState。
#[test]
fn close_delivery_reports_poisoned_event_queue() {
    // 构造已经中毒的唯一事件队列。
    let queue = poisoned_event_queue();
    // 检查式投递必须返回错误而不是静默成功。
    let error = push_window_close(&queue, WindowId::new(7))
        // 中毒队列不允许成功入队。
        .expect_err("poisoned close queue must fail");
    // 错误分类必须稳定为 owner 状态失效。
    assert_eq!(error.code(), Errc::InvalidState);
    // 错误文本必须保留 close 投递阶段。
    assert!(error.what().contains("close event queue mutex poisoned"));
}

// resize event 的队列锁失败不得提前改写 WindowState。
#[test]
fn resize_delivery_does_not_half_write_state_when_queue_is_poisoned() {
    // 构造已经中毒的唯一事件队列。
    let queue = poisoned_event_queue();
    // 建立可观察的原始窗口尺寸。
    let state = Rc::new(RefCell::new(WindowState::with_size(320, 240)));
    // 检查式投递必须返回错误而不是提交半个 resize 事务。
    let error = push_window_resize(&queue, WindowId::new(8), &state, 640, 480)
        // 中毒队列不允许成功入队。
        .expect_err("poisoned resize queue must fail");
    // 错误分类必须稳定为 owner 状态失效。
    assert_eq!(error.code(), Errc::InvalidState);
    // 读取失败后的窗口状态快照。
    let state = state.borrow();
    // 宽度不得在事件未入队时提前提交。
    assert_eq!(state.width, 320);
    // 高度不得在事件未入队时提前提交。
    assert_eq!(state.height, 240);
}

// resize callback 的可重入状态借用冲突不得毒化健康事件队列。
#[test]
fn resize_delivery_reports_state_borrow_conflict_without_poisoning_queue() {
    // 创建可观察的健康事件队列。
    let queue = Arc::new(Mutex::new(VecDeque::new()));
    // 建立可观察的原始窗口尺寸。
    let state = Rc::new(RefCell::new(WindowState::with_size(320, 240)));
    // 模拟同步调用仍持有同一 WindowState 的只读借用。
    let state_borrow = state.borrow();
    // 检查式投递必须返回错误而不是在持队列锁时 panic。
    let error = push_window_resize(&queue, WindowId::new(9), &state, 640, 480)
        // 借用冲突不允许成功提交 resize。
        .expect_err("borrowed resize state must fail");
    // 错误分类必须稳定为 owner 状态冲突。
    assert_eq!(error.code(), Errc::InvalidState);
    // 错误文本必须保留状态借用阶段。
    assert!(error.what().contains("resize state is already borrowed"));
    // 读取借用期间不得改写原始宽度。
    assert_eq!(state_borrow.width, 320);
    // 读取借用期间不得改写原始高度。
    assert_eq!(state_borrow.height, 240);
    // 结束模拟的可重入状态借用。
    drop(state_borrow);
    // 失败返回后事件队列必须仍可正常加锁。
    let queue = queue.lock().expect("borrow conflict must not poison queue");
    // 状态提交失败时不得残留 resize event。
    assert!(queue.is_empty());
}
