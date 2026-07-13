use crate::tests::common::*;
use crate::core::error::{ Result };
use crate::native::shared::state::WindowState;
use crate::native::traits::present::{ IPresenter };
use crate::native::traits::window::{INativeHandle, IWindowProperties, PlatformWindow};
use crate::native::shared::window::*;
use crate::native::presenter::NullPresenter;

fn assert_error_code(result: Result<()>, expected: Errc) {
    match result {
        Ok(()) => panic!("window operation unexpectedly succeeded"),
        Err(error) => assert_eq!(error.code(), expected),
    }
}

struct UnsupportedOptionalOps;

impl WindowOps for UnsupportedOptionalOps {
    fn os_show(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_hide(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_close(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_set_title(&mut self, _title: &str) -> Result<()> {
        Ok(())
    }
    fn os_set_size(&mut self, _w: i32, _h: i32) -> Result<()> {
        Ok(())
    }
    fn native_handle(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

struct CloseTrackingOps {
    close_calls: Rc<Cell<usize>>,
}

impl WindowOps for CloseTrackingOps {
    fn os_show(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_hide(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_close(&mut self) -> Result<()> {
        self.close_calls.set(self.close_calls.get() + 1);
        Ok(())
    }
    fn os_set_title(&mut self, _title: &str) -> Result<()> {
        Ok(())
    }
    fn os_set_size(&mut self, _w: i32, _h: i32) -> Result<()> {
        Ok(())
    }
    fn native_handle(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

struct FailingRequiredOps;

impl FailingRequiredOps {
    fn fail(operation: &str) -> Result<()> {
        Err(Error::new(
            Errc::PlatformError,
            format!("{operation} failed"),
        ))
    }
}

impl WindowOps for FailingRequiredOps {
    fn os_show(&mut self) -> Result<()> {
        Self::fail("os_show")
    }
    fn os_hide(&mut self) -> Result<()> {
        Self::fail("os_hide")
    }
    fn os_close(&mut self) -> Result<()> {
        Self::fail("os_close")
    }
    fn os_set_title(&mut self, _title: &str) -> Result<()> {
        Self::fail("os_set_title")
    }
    fn os_set_size(&mut self, _w: i32, _h: i32) -> Result<()> {
        Self::fail("os_set_size")
    }
    fn os_resize_notify(&mut self, _w: i32, _h: i32) -> Result<()> {
        Self::fail("os_resize_notify")
    }
    fn native_handle(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

#[test]
fn unsupported_optional_window_ops_do_not_mutate_shared_state() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let mut window = PlatformWindowCore::new(
        Rc::clone(&state),
        UnsupportedOptionalOps,
        Box::new(NullPresenter::new()),
    );

    assert_error_code(window.center_on_screen(), Errc::NotImplemented);
    assert_error_code(window.raise(), Errc::NotImplemented);
    assert_error_code(window.lower(), Errc::NotImplemented);
    assert_error_code(window.set_window_icon("icon.png"), Errc::NotImplemented);
    assert_error_code(window.flash_window(), Errc::NotImplemented);

    let props = window.properties_mut();
    assert_error_code(props.set_minimum_size(100, 100), Errc::NotImplemented);
    assert_error_code(props.set_maximum_size(1600, 1200), Errc::NotImplemented);
    assert_error_code(props.set_position(40, 50), Errc::NotImplemented);
    assert_error_code(props.set_resizable(false), Errc::NotImplemented);
    assert_error_code(props.maximize(), Errc::NotImplemented);
    assert_error_code(props.minimize(), Errc::NotImplemented);
    assert_error_code(props.restore(), Errc::NotImplemented);
    assert_error_code(props.set_borderless(true), Errc::NotImplemented);
    assert_error_code(props.set_fullscreen(true), Errc::NotImplemented);
    assert_error_code(props.set_always_on_top(true), Errc::NotImplemented);
    assert_error_code(props.set_window_opacity(0.5), Errc::NotImplemented);
    assert_error_code(props.start_text_input(), Errc::NotImplemented);
    assert_error_code(props.stop_text_input(), Errc::NotImplemented);
    assert_error_code(props.enable_file_drop(true), Errc::NotImplemented);

    let state = state.borrow();
    assert_eq!(state.pos_x, 0);
    assert_eq!(state.pos_y, 0);
    assert!(state.resizable);
    assert!(!state.maximized);
    assert!(!state.minimized);
    assert!(!state.borderless);
    assert!(!state.fullscreen);
    assert!(!state.always_on_top);
    assert_eq!(state.opacity, 1.0);
    assert!(!state.text_input_active);
    assert!(!state.file_drop_enabled);
}

#[test]
fn platform_window_close_is_idempotent() {
    let close_calls = Rc::new(Cell::new(0));
    let mut window = PlatformWindowCore::new(
        Rc::new(RefCell::new(WindowState::default())),
        CloseTrackingOps {
            close_calls: close_calls.clone(),
        },
        Box::new(NullPresenter::new()),
    );
    window.close().unwrap();
    window.close().unwrap();
    assert_eq!(close_calls.get(), 1);
}

#[test]
fn failed_required_window_ops_are_observable_and_do_not_mutate_shared_state() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let mut window = PlatformWindowCore::new(
        Rc::clone(&state),
        FailingRequiredOps,
        Box::new(NullPresenter::new()),
    );

    assert_error_code(window.show(), Errc::PlatformError);
    state.borrow_mut().visible = true;
    assert_error_code(window.hide(), Errc::PlatformError);
    assert_error_code(window.close(), Errc::PlatformError);
    assert_error_code(window.set_title("new title"), Errc::PlatformError);
    assert_error_code(
        window.properties_mut().set_size(1024, 768),
        Errc::PlatformError,
    );
    assert_error_code(window.resize_notify(1024, 768), Errc::PlatformError);

    let state = state.borrow();
    assert!(state.visible);
    assert_eq!((state.width, state.height), (800, 600));
}
