use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// 引入 typed failure，使 callback adapter 能把投递失败送入平台 failure queue。
use crate::core::{Errc, Error, Result, WindowId};
use crate::native::windowing::event::UiEvent;
use crate::native::windowing::shared::WindowState;

// 保留窗口生命周期事件辅助器，供尚未接入的异步平台回调使用。
#[allow(dead_code)]
pub(crate) fn push_window_close(
    // 借用 callback 与 owner thread 之间的唯一窗口事件队列。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // 标记关闭事实所属的稳定窗口身份。
    window_id: WindowId,
) -> Result<()> {
    // 锁中毒必须成为 typed failure，不能把 close 静默丢弃。
    let mut queue = events.lock().map_err(|_| {
        // 生成可由平台 failure queue 继续传播的稳定状态错误。
        Error::new(
            // 事件队列失去可用 owner 状态。
            Errc::InvalidState,
            // 保留精确的投递阶段。
            "native window close event queue mutex poisoned",
        )
    })?;
    // 只有成功取得队列 owner 后才提交 close event。
    queue.push_back(UiEvent::close().for_window(window_id));
    // 向 callback adapter 确认事件已经入队。
    Ok(())
}

// 保留窗口生命周期事件辅助器，供尚未接入的异步平台回调使用。
#[allow(dead_code)]
pub(crate) fn push_window_resize(
    // 借用 callback 与 owner thread 之间的唯一窗口事件队列。
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    // 标记 resize 事实所属的稳定窗口身份。
    window_id: WindowId,
    // 借用同一窗口的 native 尺寸状态。
    state: &Rc<RefCell<WindowState>>,
    // 接收 callback 报告的客户区宽度。
    width: i32,
    // 接收 callback 报告的客户区高度。
    height: i32,
) -> Result<()> {
    // 非正尺寸不形成可提交的 drawable 事实。
    if width <= 0 || height <= 0 {
        // 保持既有忽略语义，但显式报告本次投递完成。
        return Ok(());
    }
    // 先取得队列 owner，失败时不得只更新 WindowState 形成半写。
    let mut queue = events.lock().map_err(|_| {
        // 生成可由平台 failure queue 继续传播的稳定状态错误。
        Error::new(
            // 事件队列失去可用 owner 状态。
            Errc::InvalidState,
            // 保留精确的投递阶段。
            "native window resize event queue mutex poisoned",
        )
    })?;
    // 队列可提交后再检查状态 owner，避免借用 panic 在持锁期间毒化队列。
    {
        // 可重入借用冲突必须成为 typed failure，不能越过 callback FFI 边界。
        let mut window_state = state.try_borrow_mut().map_err(|_| {
            // 构造可由平台 failure queue 继续传播的稳定状态错误。
            Error::new(
                // 同一窗口状态当前正由另一个同步调用持有。
                Errc::InvalidState,
                // 保留 resize 状态提交阶段。
                "native window resize state is already borrowed",
            )
        })?;
        // 提交新客户区宽度。
        window_state.width = width;
        // 提交新客户区高度。
        window_state.height = height;
    }
    // 把与状态快照同源的 resize event 交给 owner thread。
    queue.push_back(UiEvent::resize(width, height).for_window(window_id));
    // 向 callback adapter 确认状态与事件均已提交。
    Ok(())
}

// 验证事件队列失效时 shared helper 不会静默丢失或半写状态。
#[cfg(test)]
mod tests {
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
}
