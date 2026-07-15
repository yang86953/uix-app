use crate::core::error::Result;
use crate::native::presenter::NullPresenter;
use crate::native::shared::state::WindowState;
use crate::native::shared::window::*;
use crate::native::traits::window::{PlatformWindow, WindowOcclusionState};
use crate::tests::common::*;

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

struct OpacityTrackingOps {
    calls: Rc<RefCell<Vec<f32>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeometryCall {
    Size(i32, i32),
    Minimum(i32, i32),
    Maximum(i32, i32),
}

struct GeometryTrackingOps {
    calls: Rc<RefCell<Vec<GeometryCall>>>,
}

impl WindowOps for GeometryTrackingOps {
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
    fn os_set_size(&mut self, width: i32, height: i32) -> Result<()> {
        self.calls
            .borrow_mut()
            .push(GeometryCall::Size(width, height));
        Ok(())
    }
    fn os_set_min_size(&mut self, width: i32, height: i32) -> Result<()> {
        self.calls
            .borrow_mut()
            .push(GeometryCall::Minimum(width, height));
        Ok(())
    }
    fn os_set_max_size(&mut self, width: i32, height: i32) -> Result<()> {
        self.calls
            .borrow_mut()
            .push(GeometryCall::Maximum(width, height));
        Ok(())
    }
    fn native_handle(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

impl WindowOps for OpacityTrackingOps {
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
    fn os_set_opacity(&mut self, opacity: f32) -> Result<()> {
        self.calls.borrow_mut().push(opacity);
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
    assert_error_code(window.show_system_menu(), Errc::NotImplemented);
    assert_eq!(
        window.occlusion_state(),
        WindowOcclusionState::Unknown,
        "unsupported backends must not claim the window is visible"
    );

    let props = window.properties_mut();
    assert_error_code(props.set_minimum_size(100, 100), Errc::NotImplemented);
    assert_error_code(props.set_maximum_size(1600, 1200), Errc::NotImplemented);
    assert_error_code(props.set_position(40, 50), Errc::NotImplemented);
    assert_error_code(props.set_resizable(false), Errc::NotImplemented);
    assert_error_code(props.maximize(), Errc::NotImplemented);
    assert_error_code(props.minimize(), Errc::NotImplemented);
    assert_error_code(props.restore(), Errc::NotImplemented);
    assert_error_code(
        props.set_system_title_bar_visible(false),
        Errc::NotImplemented,
    );
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
    assert_eq!(state.minimum_size, None);
    assert_eq!(state.maximum_size, None);
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
fn window_opacity_rejects_invalid_values_before_platform_mutation() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut window = PlatformWindowCore::new(
        Rc::clone(&state),
        OpacityTrackingOps {
            calls: Rc::clone(&calls),
        },
        Box::new(NullPresenter::new()),
    );

    for opacity in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.01, 1.01] {
        assert_error_code(
            window.properties_mut().set_window_opacity(opacity),
            Errc::InvalidArgument,
        );
    }

    assert!(calls.borrow().is_empty());
    assert_eq!(state.borrow().opacity, 1.0);
}

#[test]
fn window_opacity_accepts_closed_unit_interval() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut window = PlatformWindowCore::new(
        Rc::clone(&state),
        OpacityTrackingOps {
            calls: Rc::clone(&calls),
        },
        Box::new(NullPresenter::new()),
    );

    for opacity in [0.0, 0.25, 1.0] {
        window
            .properties_mut()
            .set_window_opacity(opacity)
            .expect("valid opacity");
    }

    assert_eq!(&*calls.borrow(), &[0.0, 0.25, 1.0]);
    assert_eq!(state.borrow().opacity, 1.0);
}

#[test]
fn window_geometry_rejects_non_positive_extents_before_platform_mutation() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut window = PlatformWindowCore::new(
        Rc::clone(&state),
        GeometryTrackingOps {
            calls: Rc::clone(&calls),
        },
        Box::new(NullPresenter::new()),
    );

    for extent in [(0, 100), (100, 0), (-1, 100), (100, -1)] {
        assert_error_code(
            window.properties_mut().set_size(extent.0, extent.1),
            Errc::InvalidArgument,
        );
        assert_error_code(
            window.properties_mut().set_minimum_size(extent.0, extent.1),
            Errc::InvalidArgument,
        );
        assert_error_code(
            window.properties_mut().set_maximum_size(extent.0, extent.1),
            Errc::InvalidArgument,
        );
    }

    assert!(calls.borrow().is_empty());
    assert_eq!((state.borrow().width, state.borrow().height), (800, 600));
}

#[test]
fn window_geometry_forwards_positive_extents() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut window = PlatformWindowCore::new(
        Rc::clone(&state),
        GeometryTrackingOps {
            calls: Rc::clone(&calls),
        },
        Box::new(NullPresenter::new()),
    );

    window.properties_mut().set_size(640, 480).unwrap();
    window.properties_mut().set_minimum_size(320, 240).unwrap();
    window
        .properties_mut()
        .set_maximum_size(1920, 1080)
        .unwrap();

    assert_eq!(
        &*calls.borrow(),
        &[
            GeometryCall::Size(640, 480),
            GeometryCall::Minimum(320, 240),
            GeometryCall::Maximum(1920, 1080),
        ]
    );
    assert_eq!((state.borrow().width, state.borrow().height), (640, 480));
    assert_eq!(state.borrow().minimum_size, Some((320, 240)));
    assert_eq!(state.borrow().maximum_size, Some((1920, 1080)));
}

#[test]
fn window_geometry_enforces_registered_constraints_before_platform_mutation() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut window = PlatformWindowCore::new(
        Rc::clone(&state),
        GeometryTrackingOps {
            calls: Rc::clone(&calls),
        },
        Box::new(NullPresenter::new()),
    );
    window.properties_mut().set_minimum_size(320, 240).unwrap();
    window.properties_mut().set_maximum_size(800, 600).unwrap();
    calls.borrow_mut().clear();

    for extent in [(319, 480), (640, 239), (801, 480), (640, 601)] {
        assert_error_code(
            window.properties_mut().set_size(extent.0, extent.1),
            Errc::InvalidArgument,
        );
    }
    assert_error_code(
        window.properties_mut().set_minimum_size(801, 240),
        Errc::InvalidArgument,
    );
    assert_error_code(
        window.properties_mut().set_maximum_size(319, 600),
        Errc::InvalidArgument,
    );

    assert!(calls.borrow().is_empty());
    assert_eq!(state.borrow().minimum_size, Some((320, 240)));
    assert_eq!(state.borrow().maximum_size, Some((800, 600)));
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
