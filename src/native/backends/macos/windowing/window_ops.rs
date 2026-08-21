use super::*;
use crate::platform::windowing::event::PointerActivationId;
use crate::platform::windowing::{WindowCapabilities, WindowCapability, WindowResizeEdge};

const MACOS_WINDOW_CAPABILITIES: WindowCapabilities = WindowCapabilities::from_slice(&[
    WindowCapability::RequestClose,
    WindowCapability::CenterOnScreen,
    WindowCapability::Raise,
    WindowCapability::Lower,
    WindowCapability::ResizeNotify,
    WindowCapability::StartTextInput,
    WindowCapability::StopTextInput,
    WindowCapability::RequestNativeFrame,
    WindowCapability::NativeFramePresented,
    WindowCapability::CancelNativeFrame,
    WindowCapability::ExactOcclusionState,
    WindowCapability::NativeSurface,
]);

// 原生窗口操作组件仅由 macOS 窗口工厂构造并交给共享窗口核心持有。
pub(super) struct MacosWindowOps {
    window: cocoa::Id,
    layer: cocoa::Id,
    window_id: WindowId,
    frame_pacer: MacosFramePacer,
    text_input_owner: text_input_view::SharedImeOwner,
    open: Rc<Cell<bool>>,
}

impl MacosWindowOps {
    // 为单个 NSWindow 构造 AppKit 操作端口与帧节拍器。
    pub(super) fn new(
        window: cocoa::Id,
        layer: cocoa::Id,
        window_id: WindowId,
        events: Arc<Mutex<VecDeque<UiEvent>>>,
        text_input_owner: text_input_view::SharedImeOwner,
        open: Rc<Cell<bool>>,
        pending_failures: PendingFailureSource,
    ) -> Self {
        Self {
            window,
            layer,
            window_id,
            frame_pacer: MacosFramePacer::new(window, events, window_id, pending_failures),
            text_input_owner,
            open,
        }
    }

    fn ensure_valid_window(&self, operation: &str) -> crate::core::Result<()> {
        if self.window.is_null() || !self.open.get() {
            return Err(Error::new(
                Errc::InvalidState,
                format!("{operation}: NSWindow is closed or invalid"),
            ));
        }
        Ok(())
    }

    fn close_and_release(&mut self) {
        self.frame_pacer.shutdown();
        let window = std::mem::replace(&mut self.window, std::ptr::null_mut());
        self.layer = std::ptr::null_mut();
        if window.is_null() {
            return;
        }
        let was_open = self.open.replace(false);
        // SAFETY: this struct owns the +1 NSWindow returned by alloc/init.
        // releasedWhenClosed is disabled, so close cannot consume that owner;
        // the matching release below is the unique final relinquish point.
        unsafe {
            if was_open {
                cocoa::close_window(window);
            }
            cocoa::release_object(window);
        }
    }
}

impl Drop for MacosWindowOps {
    fn drop(&mut self) {
        self.close_and_release();
    }
}

impl WindowOps for MacosWindowOps {
    fn capabilities(&self) -> WindowCapabilities {
        MACOS_WINDOW_CAPABILITIES
    }

    fn os_show(&mut self) -> crate::core::Result<()> {
        self.ensure_valid_window("os_show")?;
        // SAFETY: self.window is the NSWindow pointer returned by create_window.
        unsafe {
            cocoa::show_window(self.window);
        }
        Ok(())
    }

    fn os_hide(&mut self) -> crate::core::Result<()> {
        self.ensure_valid_window("os_hide")?;
        // SAFETY: self.window is the NSWindow pointer returned by create_window.
        unsafe {
            cocoa::hide_window(self.window);
        }
        Ok(())
    }

    fn os_close(&mut self) -> crate::core::Result<()> {
        self.close_and_release();
        Ok(())
    }

    // 将应用层关闭意图交给 AppKit，最终关闭事实仍由 windowWillClose 委托回调发布。
    fn os_request_close(&mut self) -> crate::core::Result<()> {
        // 已关闭或失效窗口返回明确的 InvalidState，不伪造成功结果。
        self.ensure_valid_window("os_request_close")?;
        // SAFETY: ensure_valid_window 已确认 NSWindow 仍由本对象持有；performClose: 仅同步借用该指针。
        unsafe {
            // 使用标准 AppKit 关闭入口，避免绕过委托或提前释放窗口所有权。
            cocoa::request_window_close(self.window);
        }
        // AppKit 已接受同步关闭请求；Application System 将在 close 事件后执行统一销毁。
        Ok(())
    }

    fn os_set_title(&mut self, title: &str) -> crate::core::Result<()> {
        self.ensure_valid_window("os_set_title")?;
        // SAFETY: self.window is valid and title is converted to a temporary
        // NSString before the synchronous AppKit setter call.
        unsafe {
            cocoa::set_window_title(self.window, title);
        }
        Ok(())
    }

    fn os_set_size(&mut self, w: i32, h: i32) -> crate::core::Result<()> {
        self.ensure_valid_window("os_set_size")?;
        // SAFETY: self.window is valid and CGRect is repr(C), matching AppKit ABI.
        unsafe {
            cocoa::set_window_size(self.window, w, h);
        }
        Ok(())
    }

    fn native_handle(&self) -> *mut c_void {
        self.window
    }

    fn os_center_on_screen(&mut self) -> crate::core::Result<()> {
        self.ensure_valid_window("os_center_on_screen")?;
        // SAFETY: self.window is the NSWindow pointer returned by create_window.
        unsafe {
            cocoa::msg_void(self.window, "center");
        }
        Ok(())
    }

    fn os_raise(&mut self) -> crate::core::Result<()> {
        self.ensure_valid_window("os_raise")?;
        // SAFETY: ensure_valid_window 已确认 NSWindow 仍由本对象持有，show_window 只在同步 AppKit 调用期间使用该指针。
        unsafe {
            cocoa::show_window(self.window);
        }
        Ok(())
    }

    fn os_lower(&mut self) -> crate::core::Result<()> {
        self.ensure_valid_window("os_lower")?;
        // SAFETY: ensure_valid_window 已确认 NSWindow 仍由本对象持有，hide_window 只在同步 AppKit 调用期间使用该指针。
        unsafe {
            cocoa::hide_window(self.window);
        }
        Ok(())
    }

    fn os_set_icon(&mut self, _path: &str) -> crate::core::Result<()> {
        Err(WindowCapability::SetWindowIcon.unsupported_error())
    }

    fn os_flash(&mut self) -> crate::core::Result<()> {
        Err(WindowCapability::FlashWindow.unsupported_error())
    }

    fn os_set_min_size(&mut self, _w: i32, _h: i32) -> crate::core::Result<()> {
        Err(WindowCapability::SetMinimumSize.unsupported_error())
    }

    fn os_set_max_size(&mut self, _w: i32, _h: i32) -> crate::core::Result<()> {
        Err(WindowCapability::SetMaximumSize.unsupported_error())
    }

    fn os_set_position(&mut self, _x: i32, _y: i32) -> crate::core::Result<()> {
        Err(WindowCapability::SetPosition.unsupported_error())
    }

    fn os_set_resizable(&mut self, _resizable: bool) -> crate::core::Result<()> {
        Err(WindowCapability::SetResizable.unsupported_error())
    }

    fn os_maximize(&mut self) -> crate::core::Result<()> {
        Err(WindowCapability::Maximize.unsupported_error())
    }

    fn os_minimize(&mut self) -> crate::core::Result<()> {
        Err(WindowCapability::Minimize.unsupported_error())
    }

    fn os_restore(&mut self) -> crate::core::Result<()> {
        Err(WindowCapability::Restore.unsupported_error())
    }

    fn os_set_system_title_bar_visible(&mut self, visible: bool) -> crate::core::Result<()> {
        let capability = if visible {
            WindowCapability::ShowSystemTitleBar
        } else {
            WindowCapability::HideSystemTitleBar
        };
        Err(capability.unsupported_error())
    }

    fn os_set_borderless(&mut self, _borderless: bool) -> crate::core::Result<()> {
        Err(WindowCapability::SetBorderless.unsupported_error())
    }

    fn os_set_fullscreen(&mut self, _fullscreen: bool) -> crate::core::Result<()> {
        Err(WindowCapability::SetFullscreen.unsupported_error())
    }

    fn os_set_always_on_top(&mut self, _on: bool) -> crate::core::Result<()> {
        Err(WindowCapability::SetAlwaysOnTop.unsupported_error())
    }

    fn os_set_opacity(&mut self, _opacity: f32) -> crate::core::Result<()> {
        Err(WindowCapability::SetWindowOpacity.unsupported_error())
    }

    fn os_start_text_input(&mut self) -> crate::core::Result<()> {
        self.ensure_valid_window("os_start_text_input")?;
        // SAFETY: self.window is the NSWindow pointer returned by create_window.
        unsafe {
            text_input_view::make_window_text_input_active(
                &self.text_input_owner,
                self.window_id,
                self.window,
            )
        }
    }

    fn os_stop_text_input(&mut self) -> crate::core::Result<()> {
        self.ensure_valid_window("os_stop_text_input")?;
        // SAFETY: self.window is the NSWindow pointer returned by create_window.
        unsafe {
            text_input_view::make_window_text_input_inactive(
                &self.text_input_owner,
                self.window_id,
                self.window,
            )
        }
    }

    fn os_enable_file_drop(&mut self, enable: bool) -> crate::core::Result<()> {
        let capability = if enable {
            WindowCapability::EnableFileDrop
        } else {
            WindowCapability::DisableFileDrop
        };
        Err(capability.unsupported_error())
    }

    fn os_resize_notify(&mut self, _w: i32, _h: i32) -> crate::core::Result<()> {
        Ok(())
    }

    fn os_request_native_frame(
        &mut self,
        request: NativeFrameRequest,
    ) -> crate::core::Result<bool> {
        self.ensure_valid_window("os_request_native_frame")?;
        self.frame_pacer.request(request)
    }

    fn os_native_frame_presented(&mut self, token: FrameRequestToken) -> crate::core::Result<()> {
        self.ensure_valid_window("os_native_frame_presented")?;
        self.frame_pacer.presented(token)
    }

    fn os_cancel_native_frame(&mut self, token: FrameRequestToken) -> crate::core::Result<()> {
        self.frame_pacer.cancel(token);
        Ok(())
    }

    fn os_begin_move_drag(
        &mut self,
        _pointer_activation: Option<PointerActivationId>,
    ) -> crate::core::Result<()> {
        Err(WindowCapability::BeginMoveDrag.unsupported_error())
    }

    fn os_begin_resize_drag(
        &mut self,
        _edge: WindowResizeEdge,
        _pointer_activation: Option<PointerActivationId>,
    ) -> crate::core::Result<()> {
        Err(WindowCapability::BeginResizeDrag.unsupported_error())
    }

    fn os_show_system_menu(&mut self) -> crate::core::Result<()> {
        Err(WindowCapability::ShowSystemMenu.unsupported_error())
    }

    fn os_occlusion_state(&self) -> WindowOcclusionState {
        if self.window.is_null() || !self.open.get() {
            return WindowOcclusionState::Unknown;
        }
        // SAFETY: self.window remains owned by this WindowOps until teardown,
        // and occlusionState is a synchronous NSWindow property query.
        unsafe { cocoa::window_occlusion_state(self.window) }
    }

    fn native_surface_ptr(&self) -> *mut c_void {
        self.layer
    }

    fn client_logical_extent(&self, fallback_width: i32, fallback_height: i32) -> (i32, i32) {
        (fallback_width, fallback_height)
    }
}
