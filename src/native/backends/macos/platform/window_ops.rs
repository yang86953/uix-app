use super::*;

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
}
