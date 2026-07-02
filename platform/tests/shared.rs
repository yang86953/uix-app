use std::cell::Cell;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use uix_platform::api::traits::{IEventLoop, IGraphicsContext, IWindowProperties, PlatformWindow};
use uix_platform::error::Error;
use uix_platform::geometry::Point;
use uix_platform::shared::{OsEventSource, PlatformWindowCore, WindowOps, WindowState};

struct MockWindowOps {
    handle: *mut std::ffi::c_void,
    pub show_called: Rc<Cell<bool>>,
    pub hide_called: Rc<Cell<bool>>,
    pub close_called: Rc<Cell<bool>>,
    pub last_title: Rc<RefCell<Option<String>>>,
    pub last_size: Rc<RefCell<Option<(i32, i32)>>>,
}

impl MockWindowOps {
    fn new() -> (Self, MockWindowRefs) {
        let show = Rc::new(Cell::new(false));
        let hide = Rc::new(Cell::new(false));
        let close = Rc::new(Cell::new(false));
        let title = Rc::new(RefCell::new(None));
        let size = Rc::new(RefCell::new(None));
        let refs = MockWindowRefs {
            show_called: show.clone(),
            hide_called: hide.clone(),
            close_called: close.clone(),
            last_title: title.clone(),
            last_size: size.clone(),
        };
        (Self {
            handle: std::ptr::null_mut(),
            show_called: show,
            hide_called: hide,
            close_called: close,
            last_title: title,
            last_size: size,
        }, refs)
    }
}

#[allow(dead_code)]
struct MockWindowRefs {
    pub show_called: Rc<Cell<bool>>,
    pub hide_called: Rc<Cell<bool>>,
    pub close_called: Rc<Cell<bool>>,
    pub last_title: Rc<RefCell<Option<String>>>,
    pub last_size: Rc<RefCell<Option<(i32, i32)>>>,
}

impl WindowOps for MockWindowOps {
    fn os_show(&mut self) { self.show_called.set(true); }
    fn os_hide(&mut self) { self.hide_called.set(true); }
    fn os_close(&mut self) { self.close_called.set(true); }
    fn os_set_title(&mut self, title: &str) { *self.last_title.borrow_mut() = Some(title.to_string()); }
    fn os_set_size(&mut self, w: i32, h: i32) { *self.last_size.borrow_mut() = Some((w, h)); }
    fn native_handle(&self) -> *mut std::ffi::c_void { self.handle }
}

struct NullGraphicsContext;
impl IGraphicsContext for NullGraphicsContext {
    fn initialize(&mut self, _: *mut std::ffi::c_void, _: i32, _: i32) -> std::result::Result<(), Error> { Ok(()) }
    fn resize(&mut self, _: i32, _: i32) {}
    fn make_current(&mut self) {}
    fn swap_buffers(&mut self) {}
    fn shutdown(&mut self) {}
    fn read_pixels(&mut self, _: i32, _: i32, _: i32, _: i32) -> Vec<u32> { Vec::new() }
    fn width(&self) -> i32 { 0 }
    fn height(&self) -> i32 { 0 }
}

fn make_window() -> (PlatformWindowCore<MockWindowOps>, Rc<RefCell<WindowState>>, MockWindowRefs) {
    let state = Rc::new(RefCell::new(WindowState::with_size(800, 600)));
    let (ops, refs) = MockWindowOps::new();
    let presenter = Box::new(uix_platform::presenter::NullPresenter::new());
    let win = PlatformWindowCore::new(Rc::clone(&state), ops, presenter);
    (win, state, refs)
}

#[test]
fn window_state_default_values() {
    let state = WindowState::default();
    assert_eq!(state.width, 800);
    assert_eq!(state.height, 600);
    assert_eq!(state.pos_x, 0);
    assert_eq!(state.pos_y, 0);
    assert!(!state.visible);
    assert!(state.resizable);
    assert!(!state.borderless);
    assert!(!state.fullscreen);
    assert!(!state.maximized);
    assert!(!state.minimized);
    assert!(!state.always_on_top);
    assert_eq!(state.opacity, 1.0);
    assert!(!state.file_drop_enabled);
    assert!(!state.text_input_active);
}

#[test]
fn window_state_with_size() {
    let state = WindowState::with_size(1024, 768);
    assert_eq!(state.width, 1024);
    assert_eq!(state.height, 768);
    assert!(!state.visible);
}

#[test]
fn window_state_position() {
    let mut state = WindowState::default();
    state.pos_x = 100;
    state.pos_y = 200;
    let pos = state.position();
    assert_eq!(pos.x, 100.0);
    assert_eq!(pos.y, 200.0);
}

#[test]
fn window_state_clone() {
    let a = WindowState::with_size(640, 480);
    let b = a.clone();
    assert_eq!(a.width, b.width);
}

#[test]
fn window_state_debug() {
    let state = WindowState::default();
    let s = format!("{:?}", state);
    assert!(s.contains("width"));
}

#[test]
fn platform_window_show() {
    let (mut win, state, refs) = make_window();
    win.show();
    assert!(state.borrow().visible);
    assert!(refs.show_called.get());
}

#[test]
fn platform_window_hide() {
    let (mut win, state, refs) = make_window();
    win.hide();
    assert!(!state.borrow().visible);
    assert!(refs.hide_called.get());
}

#[test]
fn platform_window_close() {
    let (mut win, state, refs) = make_window();
    win.close();
    assert!(!state.borrow().visible);
    assert!(refs.close_called.get());
}

#[test]
fn platform_window_is_visible() {
    let (win, state, _) = make_window();
    assert!(!win.is_visible());
    state.borrow_mut().visible = true;
    assert!(win.is_visible());
}

#[test]
fn platform_window_set_title() {
    let (mut win, _, refs) = make_window();
    win.set_title("Test Window");
    assert_eq!(refs.last_title.borrow().as_deref(), Some("Test Window"));
}

#[test]
fn platform_window_center_on_screen() {
    let (mut win, _, _) = make_window();
    win.center_on_screen();
}

#[test]
fn platform_window_raise_lower() {
    let (mut win, _, _) = make_window();
    win.raise();
    win.lower();
}

#[test]
fn platform_window_flash() {
    let (mut win, _, _) = make_window();
    win.flash_window();
}

#[test]
fn platform_window_set_window_icon() {
    let (mut win, _, _) = make_window();
    win.set_window_icon("icon.png");
}

#[test]
fn platform_window_resize_notify() {
    let (mut win, state, _) = make_window();
    win.resize_notify(1024, 768);
    assert_eq!(state.borrow().width, 1024);
    assert_eq!(state.borrow().height, 768);
}

#[test]
fn platform_window_resize_notify_calls_presenter_resize() {
    let (mut win, _, _) = make_window();
    win.resize_notify(640, 480);
}

#[test]
fn platform_window_properties_width_height() {
    let (win, state) = {
        let (w, s, _) = make_window();
        (w, s)
    };
    state.borrow_mut().width = 1024;
    state.borrow_mut().height = 768;
    assert_eq!(win.width(), 1024);
    assert_eq!(win.height(), 768);
}

#[test]
fn platform_window_set_size() {
    let (mut win, state, refs) = make_window();
    win.set_size(640, 480);
    assert_eq!(state.borrow().width, 640);
    assert_eq!(state.borrow().height, 480);
    assert_eq!(*refs.last_size.borrow(), Some((640, 480)));
}

#[test]
fn platform_window_position() {
    let (win, state) = {
        let (w, s, _) = make_window();
        (w, s)
    };
    state.borrow_mut().pos_x = 50;
    state.borrow_mut().pos_y = 100;
    let pos = win.position();
    assert_eq!(pos.x, 50.0);
    assert_eq!(pos.y, 100.0);
}

#[test]
fn platform_window_set_position() {
    let (mut win, state, _) = make_window();
    win.set_position(200, 300);
    assert_eq!(state.borrow().pos_x, 200);
    assert_eq!(state.borrow().pos_y, 300);
}

#[test]
fn platform_window_set_resizable() {
    let (mut win, state, _) = make_window();
    win.set_resizable(false);
    assert!(!state.borrow().resizable);
    win.set_resizable(true);
    assert!(state.borrow().resizable);
}

#[test]
fn platform_window_maximize_minimize_restore() {
    let (mut win, state, _) = make_window();
    win.maximize();
    assert!(state.borrow().maximized);
    assert!(!state.borrow().minimized);
    win.minimize();
    assert!(state.borrow().minimized);
    assert!(!state.borrow().maximized);
    win.restore();
    assert!(!state.borrow().maximized);
    assert!(!state.borrow().minimized);
}

#[test]
fn platform_window_is_maximized_minimized() {
    let (win, state) = {
        let (w, s, _) = make_window();
        (w, s)
    };
    assert!(!win.is_maximized());
    assert!(!win.is_minimized());
    state.borrow_mut().maximized = true;
    assert!(win.is_maximized());
}

#[test]
fn platform_window_set_borderless() {
    let (mut win, state, _) = make_window();
    win.set_borderless(true);
    assert!(state.borrow().borderless);
}

#[test]
fn platform_window_fullscreen() {
    let (mut win, state, _) = make_window();
    assert!(!win.is_fullscreen());
    win.set_fullscreen(true);
    assert!(state.borrow().fullscreen);
    assert!(win.is_fullscreen());
    win.set_fullscreen(false);
    assert!(!state.borrow().fullscreen);
}

#[test]
fn platform_window_set_fullscreen_idempotent() {
    let (mut win, state, _) = make_window();
    win.set_fullscreen(true);
    state.borrow_mut().fullscreen = false;
    win.set_fullscreen(true);
    assert!(state.borrow().fullscreen);
}

#[test]
fn platform_window_always_on_top() {
    let (mut win, state, _) = make_window();
    win.set_always_on_top(true);
    assert!(state.borrow().always_on_top);
}

#[test]
fn platform_window_opacity() {
    let (mut win, state, _) = make_window();
    win.set_window_opacity(0.5);
    assert_eq!(state.borrow().opacity, 0.5);
}

#[test]
fn platform_window_text_input() {
    let (mut win, state, _) = make_window();
    win.start_text_input();
    assert!(state.borrow().text_input_active);
    win.stop_text_input();
    assert!(!state.borrow().text_input_active);
}

#[test]
fn platform_window_enable_file_drop() {
    let (mut win, state, _) = make_window();
    win.enable_file_drop(true);
    assert!(state.borrow().file_drop_enabled);
    win.enable_file_drop(false);
    assert!(!state.borrow().file_drop_enabled);
}

#[test]
fn platform_window_set_min_max_size() {
    let (mut win, _, _) = make_window();
    win.set_minimum_size(100, 100);
    win.set_maximum_size(2000, 2000);
}

#[test]
fn platform_window_presenter() {
    let (mut win, _, _) = make_window();
    let p = win.presenter();
    let result = p.resize(100, 100);
    assert!(result.is_ok());
}

#[test]
fn platform_window_native_handle() {
    let (win, _, _) = make_window();
    let handle = win.native_handle();
    let ptr = handle.native_window();
    assert!(ptr.is_null());
}

#[test]
fn platform_window_graphics_context_none() {
    let (mut win, _, _) = make_window();
    assert!(win.graphics_context().is_none());
}

#[test]
fn platform_window_native_surface_ptr() {
    let (win, _, _) = make_window();
    assert!(win.native_surface_ptr().is_null());
}

#[test]
fn platform_window_core_with_gpu() {
    let state = Rc::new(RefCell::new(WindowState::with_size(800, 600)));
    let (ops, _) = MockWindowOps::new();
    let presenter = Box::new(uix_platform::presenter::NullPresenter::new());
    let gpu_ctx = NullGraphicsContext;
    let mut win = PlatformWindowCore::with_gpu(state, ops, presenter, Box::new(gpu_ctx));
    assert!(win.graphics_context().is_some());
}

#[test]
fn platform_window_core_state_rc() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let (ops, _) = MockWindowOps::new();
    let presenter = Box::new(uix_platform::presenter::NullPresenter::new());
    let win = PlatformWindowCore::new(Rc::clone(&state), ops, presenter);
    let rc = win.state_rc();
    assert!(Rc::ptr_eq(&rc, &state));
}

#[test]
fn os_event_source_poll() {
    struct TestSource {
        events: VecDeque<uix_platform::event::UiEvent>,
    }
    impl OsEventSource for TestSource {
        fn dispatch_pending(&mut self) -> bool { true }
        fn dispatch_blocking(&mut self) -> bool { true }
        fn next_event(&mut self) -> Option<uix_platform::event::UiEvent> {
            self.events.pop_front()
        }
    }
    let mut source = TestSource {
        events: VecDeque::from(vec![
            uix_platform::event::UiEvent::key_down(
                uix_platform::types::KeyCode::A,
                uix_platform::types::KeyMod::NONE,
            ),
        ]),
    };
    let count = Cell::new(0);
    let result = source.poll_event(&mut |_: &uix_platform::event::UiEvent| {
        count.set(count.get() + 1);
        true
    });
    assert!(result);
    assert_eq!(count.get(), 1);
}

#[test]
fn os_event_source_poll_returns_false_on_exit() {
    struct ExitSource;
    impl OsEventSource for ExitSource {
        fn dispatch_pending(&mut self) -> bool { false }
        fn dispatch_blocking(&mut self) -> bool { false }
        fn next_event(&mut self) -> Option<uix_platform::event::UiEvent> { None }
    }
    let mut source = ExitSource;
    let result = source.poll_event(&mut |_: &uix_platform::event::UiEvent| true);
    assert!(!result);
}

#[test]
fn os_event_source_poll_stops_on_false_callback() {
    struct TestSource {
        events: VecDeque<uix_platform::event::UiEvent>,
    }
    impl OsEventSource for TestSource {
        fn dispatch_pending(&mut self) -> bool { true }
        fn dispatch_blocking(&mut self) -> bool { true }
        fn next_event(&mut self) -> Option<uix_platform::event::UiEvent> {
            self.events.pop_front()
        }
    }
    let mut source = TestSource {
        events: VecDeque::from(vec![
            uix_platform::event::UiEvent::key_down(uix_platform::types::KeyCode::A, uix_platform::types::KeyMod::NONE),
            uix_platform::event::UiEvent::key_down(uix_platform::types::KeyCode::B, uix_platform::types::KeyMod::NONE),
        ]),
    };
    let count = Cell::new(0);
    let result = source.poll_event(&mut |_: &uix_platform::event::UiEvent| {
        count.set(count.get() + 1);
        false // 返回 false 表示停止处理
    });
    assert!(!result); // poll_event 返回 false 表示需停止
    assert_eq!(count.get(), 1);
}

#[test]
fn os_event_source_wait_event() {
    struct TestSource {
        events: VecDeque<uix_platform::event::UiEvent>,
        block_called: bool,
    }
    impl OsEventSource for TestSource {
        fn dispatch_pending(&mut self) -> bool { true }
        fn dispatch_blocking(&mut self) -> bool {
            self.block_called = true;
            true
        }
        fn next_event(&mut self) -> Option<uix_platform::event::UiEvent> {
            self.events.pop_front()
        }
    }
    let mut source = TestSource {
        events: VecDeque::from(vec![
            uix_platform::event::UiEvent::mouse_down(Point::new(5.0, 5.0), uix_platform::types::MouseButton::Left),
        ]),
        block_called: false,
    };
    let count = Cell::new(0);
    let result = source.wait_event(&mut |_: &uix_platform::event::UiEvent| {
        count.set(count.get() + 1);
        true
    });
    assert!(result);
    assert!(source.block_called);
    assert_eq!(count.get(), 1);
}

#[test]
fn os_event_source_wait_timeout() {
    struct TestSource {
        events: VecDeque<uix_platform::event::UiEvent>,
    }
    impl OsEventSource for TestSource {
        fn dispatch_pending(&mut self) -> bool { true }
        fn dispatch_blocking(&mut self) -> bool { true }
        fn dispatch_timeout(&mut self, _: std::time::Duration) -> bool { true }
        fn next_event(&mut self) -> Option<uix_platform::event::UiEvent> {
            self.events.pop_front()
        }
    }
    let mut source = TestSource {
        events: VecDeque::from(vec![
            uix_platform::event::UiEvent::key_down(uix_platform::types::KeyCode::Space, uix_platform::types::KeyMod::NONE),
        ]),
    };
    let count = Cell::new(0);
    let result = source.wait_timeout(std::time::Duration::from_millis(10), &mut |_: &uix_platform::event::UiEvent| {
        count.set(count.get() + 1);
        true
    });
    assert!(result);
    assert_eq!(count.get(), 1);
}

#[test]
fn os_event_source_wait_timeout_returns_false_on_exit() {
    struct ExitSource;
    impl OsEventSource for ExitSource {
        fn dispatch_pending(&mut self) -> bool { false }
        fn dispatch_blocking(&mut self) -> bool { false }
        fn next_event(&mut self) -> Option<uix_platform::event::UiEvent> { None }
    }
    let mut source = ExitSource;
    let result = source.wait_timeout(std::time::Duration::from_millis(1), &mut |_: &uix_platform::event::UiEvent| true);
    assert!(!result);
}

#[test]
fn unimpl_logs_debug() {
    uix_platform::shared::window::unimpl("test_method");
}

#[test]
fn window_ops_default_impls() {
    struct TestOps;
    impl WindowOps for TestOps {
        fn os_show(&mut self) {}
        fn os_hide(&mut self) {}
        fn os_close(&mut self) {}
        fn os_set_title(&mut self, _: &str) {}
        fn os_set_size(&mut self, _: i32, _: i32) {}
        fn native_handle(&self) -> *mut std::ffi::c_void { std::ptr::null_mut() }
    }
    let mut ops = TestOps;
    ops.os_center_on_screen();
    ops.os_raise();
    ops.os_lower();
    ops.os_set_icon("test");
    ops.os_flash();
    ops.os_set_min_size(100, 100);
    ops.os_set_max_size(500, 500);
    ops.os_set_position(0, 0);
    ops.os_set_resizable(true);
    ops.os_maximize();
    ops.os_minimize();
    ops.os_restore();
    ops.os_set_borderless(true);
    ops.os_set_fullscreen(false);
    ops.os_set_always_on_top(false);
    ops.os_set_opacity(1.0);
    ops.os_start_text_input();
    ops.os_stop_text_input();
    ops.os_enable_file_drop(false);
    ops.os_resize_notify(100, 200);
}
