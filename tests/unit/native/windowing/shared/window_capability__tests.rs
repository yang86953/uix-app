use super::*;
use std::cell::Cell;

struct TestPresenter;

impl IPresenter for TestPresenter {
    fn present(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        _damage: crate::core::PresentDamage,
    ) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }
}

struct TestWindowOps {
    capabilities: WindowCapabilities,
    set_position_calls: Rc<Cell<usize>>,
}

impl TestWindowOps {
    fn new(capabilities: WindowCapabilities, set_position_calls: Rc<Cell<usize>>) -> Self {
        Self {
            capabilities,
            set_position_calls,
        }
    }
}

impl WindowOps for TestWindowOps {
    fn capabilities(&self) -> WindowCapabilities {
        self.capabilities
    }

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
    fn client_logical_extent(&self, width: i32, height: i32) -> (i32, i32) {
        (width, height)
    }
    fn os_center_on_screen(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_raise(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_lower(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_set_icon(&mut self, _path: &str) -> Result<()> {
        Ok(())
    }
    fn os_flash(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_set_min_size(&mut self, _w: i32, _h: i32) -> Result<()> {
        Ok(())
    }
    fn os_set_max_size(&mut self, _w: i32, _h: i32) -> Result<()> {
        Ok(())
    }
    fn os_set_position(&mut self, _x: i32, _y: i32) -> Result<()> {
        self.set_position_calls
            .set(self.set_position_calls.get() + 1);
        Ok(())
    }
    fn os_set_resizable(&mut self, _resizable: bool) -> Result<()> {
        Ok(())
    }
    fn os_maximize(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_minimize(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_restore(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_set_system_title_bar_visible(&mut self, _visible: bool) -> Result<()> {
        Ok(())
    }
    fn os_set_borderless(&mut self, _borderless: bool) -> Result<()> {
        Ok(())
    }
    fn os_set_fullscreen(&mut self, _fullscreen: bool) -> Result<()> {
        Ok(())
    }
    fn os_set_always_on_top(&mut self, _on: bool) -> Result<()> {
        Ok(())
    }
    fn os_set_opacity(&mut self, _opacity: f32) -> Result<()> {
        Ok(())
    }
    fn os_start_text_input(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_stop_text_input(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_enable_file_drop(&mut self, _enable: bool) -> Result<()> {
        Ok(())
    }
    fn os_resize_notify(&mut self, _w: i32, _h: i32) -> Result<()> {
        Ok(())
    }
    fn os_request_native_frame(&mut self, _request: NativeFrameRequest) -> Result<bool> {
        Ok(true)
    }
    fn os_native_frame_presented(&mut self, _token: FrameRequestToken) -> Result<()> {
        Ok(())
    }
    fn os_cancel_native_frame(&mut self, _token: FrameRequestToken) -> Result<()> {
        Ok(())
    }
    fn os_request_close(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_begin_move_drag(
        &mut self,
        _pointer_activation: Option<PointerActivationId>,
    ) -> Result<()> {
        Ok(())
    }
    fn os_begin_resize_drag(
        &mut self,
        _edge: WindowResizeEdge,
        _pointer_activation: Option<PointerActivationId>,
    ) -> Result<()> {
        Ok(())
    }
    fn os_show_system_menu(&mut self) -> Result<()> {
        Ok(())
    }
    fn os_occlusion_state(&self) -> WindowOcclusionState {
        WindowOcclusionState::Unknown
    }
    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

fn core_with_capabilities(
    capabilities: WindowCapabilities,
    calls: Rc<Cell<usize>>,
) -> (PlatformWindowCore<TestWindowOps>, Rc<RefCell<WindowState>>) {
    let state = Rc::new(RefCell::new(WindowState::with_size(320, 240)));
    let ops = TestWindowOps::new(capabilities, calls);
    let core = PlatformWindowCore::new(Rc::clone(&state), ops, Box::new(TestPresenter));
    (core, state)
}

#[test]
fn unsupported_operation_does_not_delegate_or_change_shared_state() {
    let calls = Rc::new(Cell::new(0));
    let (mut core, state) = core_with_capabilities(WindowCapabilities::EMPTY, Rc::clone(&calls));

    let error = core
        .set_position(40, 50)
        .expect_err("缺少能力时必须由共享门禁拒绝");

    assert_eq!(error.code(), crate::core::Errc::NotImplemented);
    assert_eq!(
        error.message(),
        "IWindowProperties::set_position is not supported by this window"
    );
    assert_eq!(calls.get(), 0);
    assert_eq!((state.borrow().pos_x, state.borrow().pos_y), (0, 0));
}

#[test]
fn supported_operation_delegates_before_committing_shared_state() {
    let calls = Rc::new(Cell::new(0));
    let capabilities = WindowCapabilities::from_slice(&[WindowCapability::SetPosition]);
    let (mut core, state) = core_with_capabilities(capabilities, Rc::clone(&calls));

    core.set_position(40, 50)
        .expect("已声明能力必须委托 concrete adapter");

    assert_eq!(calls.get(), 1);
    assert_eq!((state.borrow().pos_x, state.borrow().pos_y), (40, 50));
}
