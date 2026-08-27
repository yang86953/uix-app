// ============================================================================
// platform/shared/window.rs — 窗口实现共享层
//
// 作用：
//   WindowOps trait         — 后端显式实现全部方法并声明真实能力集合
//   PlatformWindowCore<O>   — 与 WindowOps 组合，自动获得 PlatformWindow +
//                             IWindowProperties + INativeHandle 的完整实现
//
// 公开接口（IWindowProperties / INativeHandle / IWindowManager / PlatformWindow）
// 由 crate::platform::windowing 中立边界持有。
// ============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::error::{Error, Result};
use crate::platform::presentation::IPresenter;
// 引入帧令牌与原生指针动作的不可解释激活身份。
use crate::native::windowing::shared::state::WindowState;
use crate::platform::windowing::event::{FrameRequestToken, PointerActivationId};
use crate::platform::windowing::window::{
    INativeHandle, IWindowProperties, NativeFrameRequest, PlatformWindow, WindowOcclusionState,
};
// 共享窗口核心只转交平台中立的调整大小方向。
use crate::platform::windowing::{WindowCapabilities, WindowCapability, WindowResizeEdge};

// ════════════════════════════════════════════════════════════════════════════
// WindowOps — 平台特有的窗口操作
//
// 所有方法均须由后端显式实现；能力集合决定共享核心是否允许委托可选方法。
//
// 与 PlatformWindowCore<O> 组合使用，自动获得 PlatformWindow +
// IWindowProperties + INativeHandle 三个 trait 的完整实现。
// ════════════════════════════════════════════════════════════════════════════

pub(crate) trait WindowOps {
    fn capabilities(&self) -> WindowCapabilities;
    fn os_show(&mut self) -> Result<()>;
    fn os_hide(&mut self) -> Result<()>;
    fn os_close(&mut self) -> Result<()>;
    fn os_set_title(&mut self, title: &str) -> Result<()>;
    fn os_set_size(&mut self, w: i32, h: i32) -> Result<()>;
    fn native_handle(&self) -> *mut std::ffi::c_void;
    fn client_logical_extent(&self, fallback_width: i32, fallback_height: i32) -> (i32, i32);
    fn os_center_on_screen(&mut self) -> Result<()>;
    fn os_raise(&mut self) -> Result<()>;
    fn os_lower(&mut self) -> Result<()>;
    fn os_set_icon(&mut self, path: &str) -> Result<()>;
    fn os_flash(&mut self) -> Result<()>;
    fn os_set_min_size(&mut self, w: i32, h: i32) -> Result<()>;
    fn os_set_max_size(&mut self, w: i32, h: i32) -> Result<()>;
    fn os_set_position(&mut self, x: i32, y: i32) -> Result<()>;
    fn os_set_resizable(&mut self, resizable: bool) -> Result<()>;
    fn os_maximize(&mut self) -> Result<()>;
    fn os_minimize(&mut self) -> Result<()>;
    fn os_restore(&mut self) -> Result<()>;
    fn os_set_system_title_bar_visible(&mut self, visible: bool) -> Result<()>;
    fn os_set_borderless(&mut self, borderless: bool) -> Result<()>;
    fn os_set_fullscreen(&mut self, fullscreen: bool) -> Result<()>;
    fn os_set_always_on_top(&mut self, on: bool) -> Result<()>;
    fn os_set_opacity(&mut self, opacity: f32) -> Result<()>;
    fn os_start_text_input(&mut self) -> Result<()>;
    fn os_stop_text_input(&mut self) -> Result<()>;
    fn os_enable_file_drop(&mut self, enable: bool) -> Result<()>;
    fn os_resize_notify(&mut self, w: i32, h: i32) -> Result<()>;
    fn os_request_native_frame(&mut self, request: NativeFrameRequest) -> Result<bool>;
    fn os_native_frame_presented(&mut self, token: FrameRequestToken) -> Result<()>;
    fn os_cancel_native_frame(&mut self, token: FrameRequestToken) -> Result<()>;
    fn os_request_close(&mut self) -> Result<()>;
    fn os_begin_move_drag(&mut self, pointer_activation: Option<PointerActivationId>)
    -> Result<()>;
    fn os_begin_resize_drag(
        &mut self,
        edge: WindowResizeEdge,
        pointer_activation: Option<PointerActivationId>,
    ) -> Result<()>;
    fn os_show_system_menu(&mut self) -> Result<()>;
    fn os_occlusion_state(&self) -> WindowOcclusionState;
    fn native_surface_ptr(&self) -> *mut std::ffi::c_void;
}

pub(crate) fn validate_window_extent(operation: &str, width: i32, height: i32) -> Result<()> {
    if width <= 0 || height <= 0 {
        return Err(Error::invalid_arg(format!(
            "{operation} requires a positive extent, got {width}x{height}"
        )));
    }
    Ok(())
}

pub(crate) fn validate_window_extent_constraints(
    operation: &str,
    width: i32,
    height: i32,
    minimum: Option<(i32, i32)>,
    maximum: Option<(i32, i32)>,
) -> Result<()> {
    validate_window_extent(operation, width, height)?;
    if let Some((minimum_width, minimum_height)) = minimum {
        if width < minimum_width || height < minimum_height {
            return Err(Error::invalid_arg(format!(
                "{operation} extent {width}x{height} is below minimum {minimum_width}x{minimum_height}"
            )));
        }
    }
    if let Some((maximum_width, maximum_height)) = maximum {
        if width > maximum_width || height > maximum_height {
            return Err(Error::invalid_arg(format!(
                "{operation} extent {width}x{height} exceeds maximum {maximum_width}x{maximum_height}"
            )));
        }
    }
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════
// 辅助宏
// ════════════════════════════════════════════════════════════════════════════

macro_rules! state_read {
    // 状态读取入口采用 Rust 2024 表达式片段语义。
    ($state:expr, $field:ident) => {
        $state.borrow().$field
    };
}

macro_rules! state_write {
    // 状态对象和值均采用 Rust 2024 表达式片段语义。
    ($state:expr, $field:ident, $value:expr) => {
        $state.borrow_mut().$field = $value;
    };
}

// ════════════════════════════════════════════════════════════════════════════
// PlatformWindowCore — 平台无关的窗口实现
//
// 一次实现 PlatformWindow + IWindowProperties + INativeHandle。
// ════════════════════════════════════════════════════════════════════════════

pub(crate) struct PlatformWindowCore<O: WindowOps> {
    state: Rc<RefCell<WindowState>>,
    ops: O,
    presenter: Box<dyn IPresenter>,
    closed: bool,
}

impl<O: WindowOps> PlatformWindowCore<O> {
    pub(crate) fn new(
        state: Rc<RefCell<WindowState>>,
        ops: O,
        presenter: Box<dyn IPresenter>,
    ) -> Self {
        Self {
            state,
            ops,
            presenter,
            closed: false,
        }
    }

    // 保留共享状态句柄访问器，供平台集成与测试按需读取。
    #[allow(dead_code)]
    pub(crate) fn state_rc(&self) -> Rc<RefCell<WindowState>> {
        Rc::clone(&self.state)
    }

    fn require_capability(&self, capability: WindowCapability) -> Result<()> {
        if self.ops.capabilities().supports(capability) {
            Ok(())
        } else {
            Err(capability.unsupported_error())
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// PlatformWindow 实现
// ════════════════════════════════════════════════════════════════════════════

impl<O: WindowOps> PlatformWindow for PlatformWindowCore<O> {
    fn window_id(&self) -> crate::core::WindowId {
        state_read!(self.state, window_id)
    }

    fn capabilities(&self) -> WindowCapabilities {
        self.ops.capabilities()
    }

    fn show(&mut self) -> Result<()> {
        self.ops.os_show()?;
        state_write!(self.state, visible, true);
        Ok(())
    }
    fn hide(&mut self) -> Result<()> {
        self.ops.os_hide()?;
        state_write!(self.state, visible, false);
        Ok(())
    }
    fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.ops.os_close()?;
        self.closed = true;
        state_write!(self.state, visible, false);
        Ok(())
    }
    fn request_close(&mut self) -> Result<()> {
        self.require_capability(WindowCapability::RequestClose)?;
        self.ops.os_request_close()
    }
    // 共享窗口只负责把当前事件上下文转交平台实现。
    fn begin_move_drag(
        // 接收 app 从当前原生事件转交的激活身份。
        &mut self,
        // 该身份不进入共享窗口状态，也不会跨动作保存。
        pointer_activation: Option<PointerActivationId>,
        // 直接返回平台后端的提交或忽略结果。
    ) -> Result<()> {
        self.require_capability(WindowCapability::BeginMoveDrag)?;
        // 将一次性上下文原样交给唯一的 OS 操作所有者。
        self.ops.os_begin_move_drag(pointer_activation)
    }
    // 共享窗口只负责把方向与当前事件上下文转交平台实现。
    fn begin_resize_drag(
        // 接收 UI 声明的精确窗口缩放方向。
        &mut self,
        // 方向不进入共享窗口状态。
        edge: WindowResizeEdge,
        // 接收 app 从当前原生事件转交的激活身份。
        pointer_activation: Option<PointerActivationId>,
        // 直接返回平台后端的提交或忽略结果。
    ) -> Result<()> {
        self.require_capability(WindowCapability::BeginResizeDrag)?;
        // 将两个一次性参数原样交给唯一的 OS 操作所有者。
        self.ops.os_begin_resize_drag(edge, pointer_activation)
    }
    fn show_system_menu(&mut self) -> Result<()> {
        self.require_capability(WindowCapability::ShowSystemMenu)?;
        self.ops.os_show_system_menu()
    }
    fn is_visible(&self) -> bool {
        state_read!(self.state, visible)
    }
    fn occlusion_state(&self) -> WindowOcclusionState {
        if self
            .ops
            .capabilities()
            .supports(WindowCapability::ExactOcclusionState)
        {
            self.ops.os_occlusion_state()
        } else {
            WindowOcclusionState::Unknown
        }
    }
    fn set_title(&mut self, title: &str) -> Result<()> {
        self.ops.os_set_title(title)
    }
    fn center_on_screen(&mut self) -> Result<()> {
        self.require_capability(WindowCapability::CenterOnScreen)?;
        self.ops.os_center_on_screen()
    }
    fn raise(&mut self) -> Result<()> {
        self.require_capability(WindowCapability::Raise)?;
        self.ops.os_raise()
    }
    fn lower(&mut self) -> Result<()> {
        self.require_capability(WindowCapability::Lower)?;
        self.ops.os_lower()
    }
    fn set_window_icon(&mut self, icon_path: &str) -> Result<()> {
        self.require_capability(WindowCapability::SetWindowIcon)?;
        if icon_path.trim().is_empty() {
            return Err(Error::invalid_arg(
                "set_window_icon: path must not be empty",
            ));
        }
        if icon_path.contains('\0') {
            return Err(Error::invalid_arg(
                "set_window_icon: path must not contain NUL",
            ));
        }
        self.ops.os_set_icon(icon_path)
    }
    fn flash_window(&mut self) -> Result<()> {
        self.require_capability(WindowCapability::FlashWindow)?;
        self.ops.os_flash()
    }
    fn resize_notify(&mut self, width: i32, height: i32) -> Result<()> {
        self.require_capability(WindowCapability::ResizeNotify)?;
        self.ops.os_resize_notify(width, height)?;
        self.presenter.resize(width, height)?;
        state_write!(self.state, width, width);
        state_write!(self.state, height, height);
        Ok(())
    }
    fn properties(&self) -> &dyn IWindowProperties {
        self
    }
    fn properties_mut(&mut self) -> &mut dyn IWindowProperties {
        self
    }

    fn client_logical_extent(&self) -> (i32, i32) {
        let fallback = (
            state_read!(self.state, width),
            state_read!(self.state, height),
        );
        if self
            .ops
            .capabilities()
            .supports(WindowCapability::ExactClientLogicalExtent)
        {
            self.ops.client_logical_extent(fallback.0, fallback.1)
        } else {
            fallback
        }
    }

    fn presenter(&mut self) -> &mut dyn IPresenter {
        self.presenter.as_mut()
    }
    fn native_handle(&self) -> &dyn INativeHandle {
        self
    }

    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        if self
            .ops
            .capabilities()
            .supports(WindowCapability::NativeSurface)
        {
            self.ops.native_surface_ptr()
        } else {
            std::ptr::null_mut()
        }
    }

    fn request_native_frame(&mut self, request: NativeFrameRequest) -> Result<bool> {
        self.require_capability(WindowCapability::RequestNativeFrame)?;
        self.ops.os_request_native_frame(request)
    }

    fn native_frame_presented(&mut self, token: FrameRequestToken) -> Result<()> {
        self.require_capability(WindowCapability::NativeFramePresented)?;
        self.ops.os_native_frame_presented(token)
    }

    fn cancel_native_frame(&mut self, token: FrameRequestToken) -> Result<()> {
        self.require_capability(WindowCapability::CancelNativeFrame)?;
        self.ops.os_cancel_native_frame(token)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowProperties 共享实现
// ════════════════════════════════════════════════════════════════════════════

impl<O: WindowOps> IWindowProperties for PlatformWindowCore<O> {
    fn width(&self) -> i32 {
        state_read!(self.state, width)
    }
    fn height(&self) -> i32 {
        state_read!(self.state, height)
    }

    fn set_size(&mut self, w: i32, h: i32) -> Result<()> {
        let (minimum, maximum) = {
            let state = self.state.borrow();
            (state.minimum_size, state.maximum_size)
        };
        validate_window_extent_constraints("set_size", w, h, minimum, maximum)?;
        self.ops.os_set_size(w, h)?;
        state_write!(self.state, width, w);
        state_write!(self.state, height, h);
        Ok(())
    }

    fn set_minimum_size(&mut self, w: i32, h: i32) -> Result<()> {
        self.require_capability(WindowCapability::SetMinimumSize)?;
        let maximum = self.state.borrow().maximum_size;
        validate_window_extent_constraints("set_minimum_size", w, h, Some((w, h)), maximum)?;
        self.ops.os_set_min_size(w, h)?;
        self.state.borrow_mut().minimum_size = Some((w, h));
        Ok(())
    }
    fn set_maximum_size(&mut self, w: i32, h: i32) -> Result<()> {
        self.require_capability(WindowCapability::SetMaximumSize)?;
        let minimum = self.state.borrow().minimum_size;
        validate_window_extent_constraints("set_maximum_size", w, h, minimum, Some((w, h)))?;
        self.ops.os_set_max_size(w, h)?;
        self.state.borrow_mut().maximum_size = Some((w, h));
        Ok(())
    }

    fn position(&self) -> crate::core::geometry::Point {
        crate::core::geometry::Point::new(
            state_read!(self.state, pos_x) as f32,
            state_read!(self.state, pos_y) as f32,
        )
    }
    fn set_position(&mut self, x: i32, y: i32) -> Result<()> {
        self.require_capability(WindowCapability::SetPosition)?;
        self.ops.os_set_position(x, y)?;
        state_write!(self.state, pos_x, x);
        state_write!(self.state, pos_y, y);
        Ok(())
    }

    fn set_resizable(&mut self, r: bool) -> Result<()> {
        self.require_capability(WindowCapability::SetResizable)?;
        self.ops.os_set_resizable(r)?;
        state_write!(self.state, resizable, r);
        Ok(())
    }
    fn is_maximized(&self) -> bool {
        state_read!(self.state, maximized)
    }
    fn is_minimized(&self) -> bool {
        state_read!(self.state, minimized)
    }
    fn maximize(&mut self) -> Result<()> {
        self.require_capability(WindowCapability::Maximize)?;
        self.ops.os_maximize()?;
        state_write!(self.state, maximized, true);
        state_write!(self.state, minimized, false);
        Ok(())
    }
    fn minimize(&mut self) -> Result<()> {
        self.require_capability(WindowCapability::Minimize)?;
        self.ops.os_minimize()?;
        state_write!(self.state, minimized, true);
        state_write!(self.state, maximized, false);
        Ok(())
    }
    fn restore(&mut self) -> Result<()> {
        self.require_capability(WindowCapability::Restore)?;
        self.ops.os_restore()?;
        state_write!(self.state, maximized, false);
        state_write!(self.state, minimized, false);
        Ok(())
    }
    fn set_system_title_bar_visible(&mut self, visible: bool) -> Result<()> {
        let capability = if visible {
            WindowCapability::ShowSystemTitleBar
        } else {
            WindowCapability::HideSystemTitleBar
        };
        self.require_capability(capability)?;
        self.ops.os_set_system_title_bar_visible(visible)
    }
    fn set_borderless(&mut self, b: bool) -> Result<()> {
        self.require_capability(WindowCapability::SetBorderless)?;
        self.ops.os_set_borderless(b)?;
        state_write!(self.state, borderless, b);
        Ok(())
    }

    fn set_fullscreen(&mut self, f: bool) -> Result<()> {
        self.require_capability(WindowCapability::SetFullscreen)?;
        if f == state_read!(self.state, fullscreen) {
            return Ok(());
        }
        self.ops.os_set_fullscreen(f)?;
        state_write!(self.state, fullscreen, f);
        Ok(())
    }
    fn is_fullscreen(&self) -> bool {
        state_read!(self.state, fullscreen)
    }
    fn set_always_on_top(&mut self, on: bool) -> Result<()> {
        self.require_capability(WindowCapability::SetAlwaysOnTop)?;
        self.ops.os_set_always_on_top(on)?;
        state_write!(self.state, always_on_top, on);
        Ok(())
    }
    fn set_window_opacity(&mut self, opacity: f32) -> Result<()> {
        self.require_capability(WindowCapability::SetWindowOpacity)?;
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err(Error::invalid_arg(
                "window opacity must be a finite value in 0.0..=1.0",
            ));
        }
        self.ops.os_set_opacity(opacity)?;
        state_write!(self.state, opacity, opacity);
        Ok(())
    }
    fn start_text_input(&mut self) -> Result<()> {
        self.require_capability(WindowCapability::StartTextInput)?;
        self.ops.os_start_text_input()?;
        state_write!(self.state, text_input_active, true);
        Ok(())
    }
    fn stop_text_input(&mut self) -> Result<()> {
        self.require_capability(WindowCapability::StopTextInput)?;
        self.ops.os_stop_text_input()?;
        state_write!(self.state, text_input_active, false);
        Ok(())
    }
    fn enable_file_drop(&mut self, enable: bool) -> Result<()> {
        let capability = if enable {
            WindowCapability::EnableFileDrop
        } else {
            WindowCapability::DisableFileDrop
        };
        self.require_capability(capability)?;
        self.ops.os_enable_file_drop(enable)?;
        state_write!(self.state, file_drop_enabled, enable);
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// INativeHandle 共享实现
// ════════════════════════════════════════════════════════════════════════════

impl<O: WindowOps> INativeHandle for PlatformWindowCore<O> {
    fn native_window(&self) -> *mut std::ffi::c_void {
        self.ops.native_handle()
    }
}
