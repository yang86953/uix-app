use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::core::WindowId;
use crate::native::shared::WindowState;
use crate::native::traits::event::UiEvent;

pub fn push_window_close(events: &Arc<Mutex<VecDeque<UiEvent>>>, window_id: WindowId) {
    let _ = events
        .lock()
        .map(|mut queue| queue.push_back(UiEvent::close().for_window(window_id)));
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_window_resize_updates_state_and_queues_event() {
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let state = Rc::new(RefCell::new(WindowState::with_id_and_size(
            WindowId::new(3),
            100,
            80,
        )));

        push_window_resize(&events, WindowId::new(3), &state, 640, 480);

        let st = state.borrow();
        assert_eq!(st.width, 640);
        assert_eq!(st.height, 480);
        let queue = events.lock().unwrap();
        assert_eq!(queue.len(), 1);
        let event = queue.front().expect("resize event");
        assert_eq!(event.window_id, Some(WindowId::new(3)));
        assert_eq!(
            event.type_,
            crate::native::traits::event::UiEventType::WindowResize
        );
    }

    #[test]
    fn push_window_close_queues_tagged_event() {
        let events = Arc::new(Mutex::new(VecDeque::new()));

        push_window_close(&events, WindowId::new(9));

        let queue = events.lock().unwrap();
        assert_eq!(queue.len(), 1);
        let event = queue.front().expect("close event");
        assert_eq!(event.window_id, Some(WindowId::new(9)));
        assert_eq!(
            event.type_,
            crate::native::traits::event::UiEventType::WindowClose
        );
    }

    #[test]
    fn push_window_resize_ignores_non_positive_dimensions() {
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let state = Rc::new(RefCell::new(WindowState::with_id_and_size(
            WindowId::new(1),
            120,
            90,
        )));

        push_window_resize(&events, WindowId::new(1), &state, 0, 480);
        push_window_resize(&events, WindowId::new(1), &state, 640, -1);

        assert_eq!(state.borrow().width, 120);
        assert_eq!(state.borrow().height, 90);
        assert!(events.lock().unwrap().is_empty());
    }
}
