use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// 引入 typed failure，使 callback adapter 能把投递失败送入平台 failure queue。
use crate::core::{Errc, Error, Result, WindowId};
use crate::native::windowing::shared::WindowState;
use crate::platform::windowing::event::UiEvent;

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
