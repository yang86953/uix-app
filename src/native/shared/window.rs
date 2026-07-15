// ============================================================================
// platform/shared/window.rs — 窗口实现共享层
//
// 作用：
//   WindowOps trait         — 平台只需实现 6 个必须方法，其余能力有 typed 默认
//   PlatformWindowCore<O>   — 与 WindowOps 组合，自动获得 PlatformWindow +
//                             IWindowProperties + INativeHandle 的完整实现
//
// 公开接口（IWindowProperties / INativeHandle / IWindowManager / PlatformWindow）
// 已迁移至 crate::native::traits 功能模块。
// ============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::error::{Errc, Error, Result};
use crate::native::shared::state::WindowState;
use crate::native::traits::event::FrameRequestToken;
use crate::native::traits::present::{IGraphicsContext, IPresenter};
use crate::native::traits::window::{
    INativeHandle, IWindowProperties, NativeFrameRequest, PlatformWindow, WindowOcclusionState,
};

// ════════════════════════════════════════════════════════════════════════════
// WindowOps — 平台特有的窗口操作
//
// 必须实现（6 个）：os_show, os_hide, os_close, os_set_title, os_set_size, native_handle
// 其余可选能力有默认实现：返回 Err(NotImplemented)。
//
// 与 PlatformWindowCore<O> 组合使用，自动获得 PlatformWindow +
// IWindowProperties + INativeHandle 三个 trait 的完整实现。
// ════════════════════════════════════════════════════════════════════════════

pub trait WindowOps {
    // ── 必须实现（无默认，编译期强制）────────────────────────────
    fn os_show(&mut self) -> Result<()>;
    fn os_hide(&mut self) -> Result<()>;
    fn os_close(&mut self) -> Result<()>;
    fn os_set_title(&mut self, title: &str) -> Result<()>;
    fn os_set_size(&mut self, w: i32, h: i32) -> Result<()>;
    fn native_handle(&self) -> *mut std::ffi::c_void;

    // ── 窗口外观 ─────────────────────────────────────────
    fn os_center_on_screen(&mut self) -> Result<()> {
        unimpl("os_center_on_screen")
    }
    fn os_raise(&mut self) -> Result<()> {
        unimpl("os_raise")
    }
    fn os_lower(&mut self) -> Result<()> {
        unimpl("os_lower")
    }
    fn os_set_icon(&mut self, _p: &str) -> Result<()> {
        unimpl("os_set_icon")
    }
    fn os_flash(&mut self) -> Result<()> {
        unimpl("os_flash")
    }

    // ── 尺寸约束 ─────────────────────────────────────────
    fn os_set_min_size(&mut self, _w: i32, _h: i32) -> Result<()> {
        unimpl("os_set_min_size")
    }
    fn os_set_max_size(&mut self, _w: i32, _h: i32) -> Result<()> {
        unimpl("os_set_max_size")
    }
    fn os_set_position(&mut self, _x: i32, _y: i32) -> Result<()> {
        unimpl("os_set_position")
    }

    // ── 窗口状态 ─────────────────────────────────────────
    fn os_set_resizable(&mut self, _r: bool) -> Result<()> {
        unimpl("os_set_resizable")
    }
    fn os_maximize(&mut self) -> Result<()> {
        unimpl("os_maximize")
    }
    fn os_minimize(&mut self) -> Result<()> {
        unimpl("os_minimize")
    }
    fn os_restore(&mut self) -> Result<()> {
        unimpl("os_restore")
    }
    fn os_set_system_title_bar_visible(&mut self, _visible: bool) -> Result<()> {
        unimpl("os_set_system_title_bar_visible")
    }
    fn os_set_borderless(&mut self, _b: bool) -> Result<()> {
        unimpl("os_set_borderless")
    }
    fn os_set_fullscreen(&mut self, _f: bool) -> Result<()> {
        unimpl("os_set_fullscreen")
    }
    fn os_set_always_on_top(&mut self, _on: bool) -> Result<()> {
        unimpl("os_set_always_on_top")
    }
    fn os_set_opacity(&mut self, _o: f32) -> Result<()> {
        unimpl("os_set_opacity")
    }

    // ── 特性开关 ─────────────────────────────────────────
    fn os_start_text_input(&mut self) -> Result<()> {
        unimpl("os_start_text_input")
    }
    fn os_stop_text_input(&mut self) -> Result<()> {
        unimpl("os_stop_text_input")
    }
    fn os_enable_file_drop(&mut self, _e: bool) -> Result<()> {
        unimpl("os_enable_file_drop")
    }

    // ── 几何通知 ─────────────────────────────────────────
    fn os_resize_notify(&mut self, _w: i32, _h: i32) -> Result<()> {
        Ok(())
    }

    fn os_request_native_frame(&mut self, _request: NativeFrameRequest) -> Result<bool> {
        Ok(false)
    }

    fn os_native_frame_presented(&mut self, _token: FrameRequestToken) -> Result<()> {
        Ok(())
    }

    fn os_cancel_native_frame(&mut self, _token: FrameRequestToken) -> Result<()> {
        Ok(())
    }

    fn os_request_close(&mut self) -> Result<()> {
        unimpl("os_request_close")
    }

    fn os_begin_move_drag(&mut self) -> Result<()> {
        unimpl("os_begin_move_drag")
    }

    fn os_show_system_menu(&mut self) -> Result<()> {
        unimpl("os_show_system_menu")
    }

    /// Exact compositor visibility, when the native window system exposes it.
    fn os_occlusion_state(&self) -> WindowOcclusionState {
        WindowOcclusionState::Unknown
    }

    /// Wayland wl_surface C 指针（EGL 初始化用）。非 Wayland 返回 null。
    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

/// 报告未实现的窗口操作。
pub fn unimpl(method: &str) -> Result<()> {
    crate::core::log::debug_fn(format!(
        "WindowOps::{} 未实现！该平台不支持此窗口操作。",
        method
    ));
    Err(Error::new(
        Errc::NotImplemented,
        format!("WindowOps::{method} is not supported on this platform"),
    ))
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
    ($state:expr, $field:ident) => {
        $state.borrow().$field
    };
}

macro_rules! state_write {
    ($state:expr, $field:ident, $value:expr) => {
        $state.borrow_mut().$field = $value;
    };
}

// ════════════════════════════════════════════════════════════════════════════
// PlatformWindowCore — 平台无关的窗口实现
//
// 一次实现 PlatformWindow + IWindowProperties + INativeHandle。
// ════════════════════════════════════════════════════════════════════════════

pub struct PlatformWindowCore<O: WindowOps> {
    state: Rc<RefCell<WindowState>>,
    ops: O,
    presenter: Box<dyn IPresenter>,
    /// GPU 图形上下文（GPU/Hybrid 模式时设置，CPU 模式为 None）。
    gpu_ctx: Option<Box<dyn IGraphicsContext>>,
    closed: bool,
}

impl<O: WindowOps> PlatformWindowCore<O> {
    pub fn new(state: Rc<RefCell<WindowState>>, ops: O, presenter: Box<dyn IPresenter>) -> Self {
        Self {
            state,
            ops,
            presenter,
            gpu_ctx: None,
            closed: false,
        }
    }

    /// 创建带 GPU 上下文的窗口（GPU/Hybrid 渲染模式）。
    #[allow(dead_code)] // Reserved for crate-local platform assembly only (#187).
    pub(crate) fn with_gpu(
        state: Rc<RefCell<WindowState>>,
        ops: O,
        presenter: Box<dyn IPresenter>,
        gpu_ctx: Box<dyn IGraphicsContext>,
    ) -> Self {
        Self {
            state,
            ops,
            presenter,
            gpu_ctx: Some(gpu_ctx),
            closed: false,
        }
    }

    pub fn state_rc(&self) -> Rc<RefCell<WindowState>> {
        Rc::clone(&self.state)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// PlatformWindow 实现
// ════════════════════════════════════════════════════════════════════════════

impl<O: WindowOps> PlatformWindow for PlatformWindowCore<O> {
    fn window_id(&self) -> crate::core::WindowId {
        state_read!(self.state, window_id)
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
        if let Some(gpu_ctx) = self.gpu_ctx.as_mut() {
            gpu_ctx.try_shutdown()?;
        }
        self.ops.os_close()?;
        self.closed = true;
        state_write!(self.state, visible, false);
        Ok(())
    }
    fn request_close(&mut self) -> Result<()> {
        self.ops.os_request_close()
    }
    fn begin_move_drag(&mut self) -> Result<()> {
        self.ops.os_begin_move_drag()
    }
    fn show_system_menu(&mut self) -> Result<()> {
        self.ops.os_show_system_menu()
    }
    fn is_visible(&self) -> bool {
        state_read!(self.state, visible)
    }
    fn occlusion_state(&self) -> WindowOcclusionState {
        self.ops.os_occlusion_state()
    }
    fn set_title(&mut self, title: &str) -> Result<()> {
        self.ops.os_set_title(title)
    }
    fn center_on_screen(&mut self) -> Result<()> {
        self.ops.os_center_on_screen()
    }
    fn raise(&mut self) -> Result<()> {
        self.ops.os_raise()
    }
    fn lower(&mut self) -> Result<()> {
        self.ops.os_lower()
    }
    fn set_window_icon(&mut self, icon_path: &str) -> Result<()> {
        self.ops.os_set_icon(icon_path)
    }
    fn flash_window(&mut self) -> Result<()> {
        self.ops.os_flash()
    }
    fn resize_notify(&mut self, width: i32, height: i32) -> Result<()> {
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
    fn presenter(&mut self) -> &mut dyn IPresenter {
        self.presenter.as_mut()
    }
    fn native_handle(&self) -> &dyn INativeHandle {
        self
    }

    fn graphics_context(&mut self) -> Option<&mut dyn IGraphicsContext> {
        match self.gpu_ctx {
            Some(ref mut ctx) => Some(&mut **ctx),
            None => None,
        }
    }

    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        self.ops.native_surface_ptr()
    }

    fn request_native_frame(&mut self, request: NativeFrameRequest) -> Result<bool> {
        self.ops.os_request_native_frame(request)
    }

    fn native_frame_presented(&mut self, token: FrameRequestToken) -> Result<()> {
        self.ops.os_native_frame_presented(token)
    }

    fn cancel_native_frame(&mut self, token: FrameRequestToken) -> Result<()> {
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
        let maximum = self.state.borrow().maximum_size;
        validate_window_extent_constraints("set_minimum_size", w, h, Some((w, h)), maximum)?;
        self.ops.os_set_min_size(w, h)?;
        self.state.borrow_mut().minimum_size = Some((w, h));
        Ok(())
    }
    fn set_maximum_size(&mut self, w: i32, h: i32) -> Result<()> {
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
        self.ops.os_set_position(x, y)?;
        state_write!(self.state, pos_x, x);
        state_write!(self.state, pos_y, y);
        Ok(())
    }

    fn set_resizable(&mut self, r: bool) -> Result<()> {
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
        self.ops.os_maximize()?;
        state_write!(self.state, maximized, true);
        state_write!(self.state, minimized, false);
        Ok(())
    }
    fn minimize(&mut self) -> Result<()> {
        self.ops.os_minimize()?;
        state_write!(self.state, minimized, true);
        state_write!(self.state, maximized, false);
        Ok(())
    }
    fn restore(&mut self) -> Result<()> {
        self.ops.os_restore()?;
        state_write!(self.state, maximized, false);
        state_write!(self.state, minimized, false);
        Ok(())
    }
    fn set_system_title_bar_visible(&mut self, visible: bool) -> Result<()> {
        self.ops.os_set_system_title_bar_visible(visible)
    }
    fn set_borderless(&mut self, b: bool) -> Result<()> {
        self.ops.os_set_borderless(b)?;
        state_write!(self.state, borderless, b);
        Ok(())
    }

    fn set_fullscreen(&mut self, f: bool) -> Result<()> {
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
        self.ops.os_set_always_on_top(on)?;
        state_write!(self.state, always_on_top, on);
        Ok(())
    }
    fn set_window_opacity(&mut self, opacity: f32) -> Result<()> {
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
        self.ops.os_start_text_input()?;
        state_write!(self.state, text_input_active, true);
        Ok(())
    }
    fn stop_text_input(&mut self) -> Result<()> {
        self.ops.os_stop_text_input()?;
        state_write!(self.state, text_input_active, false);
        Ok(())
    }
    fn enable_file_drop(&mut self, enable: bool) -> Result<()> {
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
