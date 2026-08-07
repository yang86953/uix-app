use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::core::WindowId;
use crate::native::windowing::event::UiEvent;
use crate::native::windowing::shared::WindowState;

// 保留窗口生命周期事件辅助器，供尚未接入的异步平台回调使用。
#[allow(dead_code)]
pub fn push_window_close(events: &Arc<Mutex<VecDeque<UiEvent>>>, window_id: WindowId) {
    let _ = events
        .lock()
        .map(|mut queue| queue.push_back(UiEvent::close().for_window(window_id)));
}

// 保留窗口生命周期事件辅助器，供尚未接入的异步平台回调使用。
#[allow(dead_code)]
pub fn push_window_resize(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    window_id: WindowId,
    state: &Rc<RefCell<WindowState>>,
    width: i32,
    height: i32,
) {
    if width <= 0 || height <= 0 {
        return;
    }
    {
        let mut window_state = state.borrow_mut();
        window_state.width = width;
        window_state.height = height;
    }
    let _ = events.lock().map(|mut queue| {
        queue.push_back(UiEvent::resize(width, height).for_window(window_id));
    });
}
