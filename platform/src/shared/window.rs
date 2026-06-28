// ============================================================================
// platform/shared/window.rs — 窗口实现共享层
//
// 作用：
//   WindowOps trait         — 平台只需实现 6 个必须方法，其余 20 个有合理默认
//   PlatformWindowCore<O>   — 与 WindowOps 组合，自动获得 PlatformWindow +
//                             IWindowProperties + INativeHandle 的完整实现
//   窗口 API trait 定义     — IWindowProperties / INativeHandle / IWindowManager
//                             / PlatformWindow 也在此定义
// ============================================================================

use std::cell::RefCell;
use std::rc::Rc;

use crate::error::Error;
use crate::geometry::Point;
use crate::presenter::{IPresenter, IGraphicsContext};
use crate::shared::state::WindowState;

// ════════════════════════════════════════════════════════════════════════════
// IWindowProperties — 窗口属性查询与修改（PlatformWindow 的子组件）
// ════════════════════════════════════════════════════════════════════════════

pub trait IWindowProperties {
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    fn set_size(&mut self, w: i32, h: i32);
    fn set_minimum_size(&mut self, w: i32, h: i32);
    fn set_maximum_size(&mut self, w: i32, h: i32);
    fn position(&self) -> Point;
    fn set_position(&mut self, x: i32, y: i32);
    fn set_resizable(&mut self, resizable: bool);
    fn is_maximized(&self) -> bool;
    fn is_minimized(&self) -> bool;
    fn maximize(&mut self);
    fn minimize(&mut self);
    fn restore(&mut self);
    fn set_borderless(&mut self, borderless: bool);
    fn set_fullscreen(&mut self, fullscreen: bool);
    fn is_fullscreen(&self) -> bool;
    fn set_always_on_top(&mut self, on: bool);
    fn set_window_opacity(&mut self, opacity: f32);
    fn start_text_input(&mut self);
    fn stop_text_input(&mut self);
    fn enable_file_drop(&mut self, enable: bool);
}

// ════════════════════════════════════════════════════════════════════════════
// INativeHandle — 原生窗口句柄（每个窗口独立）
// ════════════════════════════════════════════════════════════════════════════

pub trait INativeHandle {
    fn native_window(&self) -> *mut std::ffi::c_void;
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowManager — 窗口工厂（平台共享，创建 PlatformWindow）
// ════════════════════════════════════════════════════════════════════════════

pub trait IWindowManager {
    /// 创建一个新窗口，返回窗口级操作句柄。
    fn create_window(&mut self, title: &str, width: i32, height: i32) -> Result<Box<dyn PlatformWindow>, Error>;
}

// ════════════════════════════════════════════════════════════════════════════
// PlatformWindow — 单窗口操作接口（多窗口架构核心）
//
// 每个窗口是一个独立的 trait object，封装：
//   - 窗口生命周期（show/hide/close）
//   - 窗口外观（title/icon/flash）
//   - 窗口属性（IWindowProperties 子组件）
//   - 像素呈现（IPresenter 子组件）
//   - 原生句柄（INativeHandle 子组件）
//
// 共享资源（事件循环、剪贴板等）通过 Platform trait 访问。
// ════════════════════════════════════════════════════════════════════════════

pub trait PlatformWindow {
    // ── 窗口生命周期 ──────────────────────────────────────────
    fn show(&mut self);
    fn hide(&mut self);
    fn close(&mut self);
    fn is_visible(&self) -> bool;

    // ── 窗口外观 ──────────────────────────────────────────────
    fn set_title(&mut self, title: &str);
    fn center_on_screen(&mut self);
    fn raise(&mut self);
    fn lower(&mut self);
    fn set_window_icon(&mut self, icon_path: &str);
    fn flash_window(&mut self);

    // ── 几何通知 ──────────────────────────────────────────────
    /// 通知窗口尺寸已变化（用于同步平台资源如 SHM 缓冲）。
    fn resize_notify(&mut self, width: i32, height: i32);

    // ── 子组件访问 ────────────────────────────────────────────
    fn properties(&self) -> &dyn IWindowProperties;
    fn properties_mut(&mut self) -> &mut dyn IWindowProperties;
    fn presenter(&mut self) -> &mut dyn IPresenter;
    fn native_handle(&self) -> &dyn INativeHandle;

    /// GPU 图形上下文。仅 GPU/Hybrid 模式可用，CPU 模式返回 `None`。
    fn graphics_context(&mut self) -> Option<&mut dyn IGraphicsContext> { None }

    /// Wayland wl_surface 原始指针（供 EGL 初始化）。非 Wayland 平台返回 null。
    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// WindowOps — 平台特有的窗口操作
//
// 必须实现（6 个）：os_show, os_hide, os_close, os_set_title, os_set_size, native_handle
// 其余 20 个有合理默认实现（运行时输出 ERROR 日志表示不支持）。
//
// 与 PlatformWindowCore<O> 组合使用，自动获得 PlatformWindow +
// IWindowProperties + INativeHandle 三个 trait 的完整实现。
// ════════════════════════════════════════════════════════════════════════════

pub trait WindowOps {
    // ── 必须实现（无默认，编译期强制）───────────────────

    fn os_show(&mut self);
    fn os_hide(&mut self);
    fn os_close(&mut self);
    fn os_set_title(&mut self, title: &str);
    fn os_set_size(&mut self, w: i32, h: i32);
    fn native_handle(&self) -> *mut std::ffi::c_void;

    // ── 窗口外观 ─────────────────────────────────────────

    fn os_center_on_screen(&mut self) { unimpl("os_center_on_screen"); }
    fn os_raise(&mut self) { unimpl("os_raise"); }
    fn os_lower(&mut self) { unimpl("os_lower"); }
    fn os_set_icon(&mut self, _p: &str) { unimpl("os_set_icon"); }
    fn os_flash(&mut self) { unimpl("os_flash"); }

    // ── 尺寸约束 ─────────────────────────────────────────

    fn os_set_min_size(&mut self, _w: i32, _h: i32) { unimpl("os_set_min_size"); }
    fn os_set_max_size(&mut self, _w: i32, _h: i32) { unimpl("os_set_max_size"); }
    fn os_set_position(&mut self, _x: i32, _y: i32) { unimpl("os_set_position"); }

    // ── 窗口状态 ─────────────────────────────────────────

    fn os_set_resizable(&mut self, _r: bool) { unimpl("os_set_resizable"); }
    fn os_maximize(&mut self) { unimpl("os_maximize"); }
    fn os_minimize(&mut self) { unimpl("os_minimize"); }
    fn os_restore(&mut self) { unimpl("os_restore"); }
    fn os_set_borderless(&mut self, _b: bool) { unimpl("os_set_borderless"); }
    fn os_set_fullscreen(&mut self, _f: bool) { unimpl("os_set_fullscreen"); }
    fn os_set_always_on_top(&mut self, _on: bool) { unimpl("os_set_always_on_top"); }
    fn os_set_opacity(&mut self, _o: f32) { unimpl("os_set_opacity"); }

    // ── 特性开关 ─────────────────────────────────────────

    fn os_start_text_input(&mut self) { unimpl("os_start_text_input"); }
    fn os_stop_text_input(&mut self) { unimpl("os_stop_text_input"); }
    fn os_enable_file_drop(&mut self, _e: bool) { unimpl("os_enable_file_drop"); }

    // ── 几何通知 ─────────────────────────────────────────

    /// 窗口尺寸变化通知。
    fn os_resize_notify(&mut self, _w: i32, _h: i32) {}

    /// Wayland wl_surface C 指针（EGL 初始化用）。非 Wayland 返回 null。
    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

/// 报告未实现的窗口操作。
pub fn unimpl(method: &str) {
    crate::log::debug_fn(format!("WindowOps::{} 未实现！该平台不支持此窗口操作。", method));
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
}

impl<O: WindowOps> PlatformWindowCore<O> {
    pub fn new(state: Rc<RefCell<WindowState>>, ops: O, presenter: Box<dyn IPresenter>) -> Self {
        Self { state, ops, presenter, gpu_ctx: None }
    }

    /// 创建带 GPU 上下文的窗口（GPU/Hybrid 渲染模式）。
    pub fn with_gpu(
        state: Rc<RefCell<WindowState>>,
        ops: O,
        presenter: Box<dyn IPresenter>,
        gpu_ctx: Box<dyn IGraphicsContext>,
    ) -> Self {
        Self { state, ops, presenter, gpu_ctx: Some(gpu_ctx) }
    }

    pub fn state_rc(&self) -> Rc<RefCell<WindowState>> {
        Rc::clone(&self.state)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// PlatformWindow 实现
// ════════════════════════════════════════════════════════════════════════════

impl<O: WindowOps> PlatformWindow for PlatformWindowCore<O> {
    fn show(&mut self) {
        state_write!(self.state, visible, true);
        self.ops.os_show();
    }
    fn hide(&mut self) {
        state_write!(self.state, visible, false);
        self.ops.os_hide();
    }
    fn close(&mut self) {
        state_write!(self.state, visible, false);
        self.ops.os_close();
    }
    fn is_visible(&self) -> bool {
        state_read!(self.state, visible)
    }
    fn set_title(&mut self, title: &str) {
        self.ops.os_set_title(title);
    }
    fn center_on_screen(&mut self) {
        self.ops.os_center_on_screen();
    }
    fn raise(&mut self) {
        self.ops.os_raise();
    }
    fn lower(&mut self) {
        self.ops.os_lower();
    }
    fn set_window_icon(&mut self, icon_path: &str) {
        self.ops.os_set_icon(icon_path);
    }
    fn flash_window(&mut self) {
        self.ops.os_flash();
    }
    fn resize_notify(&mut self, width: i32, height: i32) {
        state_write!(self.state, width, width);
        state_write!(self.state, height, height);
        self.ops.os_resize_notify(width, height);
        if let Err(e) = self.presenter.resize(width, height) {
            crate::log::warn_fn(format!("resize_notify: presenter.resize({}, {}) failed: {}", width, height, e.short_what()));
        }
    }
    fn properties(&self) -> &dyn IWindowProperties { self }
    fn properties_mut(&mut self) -> &mut dyn IWindowProperties { self }
    fn presenter(&mut self) -> &mut dyn IPresenter { self.presenter.as_mut() }
    fn native_handle(&self) -> &dyn INativeHandle { self }

    fn graphics_context(&mut self) -> Option<&mut dyn IGraphicsContext> {
        match self.gpu_ctx {
            Some(ref mut ctx) => Some(&mut **ctx),
            None => None,
        }
    }

    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        self.ops.native_surface_ptr()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowProperties 共享实现
// ════════════════════════════════════════════════════════════════════════════

impl<O: WindowOps> IWindowProperties for PlatformWindowCore<O> {
    fn width(&self) -> i32 { state_read!(self.state, width) }
    fn height(&self) -> i32 { state_read!(self.state, height) }

    fn set_size(&mut self, w: i32, h: i32) {
        state_write!(self.state, width, w);
        state_write!(self.state, height, h);
        self.ops.os_set_size(w, h);
    }

    fn set_minimum_size(&mut self, w: i32, h: i32) { self.ops.os_set_min_size(w, h); }
    fn set_maximum_size(&mut self, w: i32, h: i32) { self.ops.os_set_max_size(w, h); }

    fn position(&self) -> Point {
        Point::new(
            state_read!(self.state, pos_x) as f32,
            state_read!(self.state, pos_y) as f32,
        )
    }
    fn set_position(&mut self, x: i32, y: i32) {
        state_write!(self.state, pos_x, x);
        state_write!(self.state, pos_y, y);
        self.ops.os_set_position(x, y);
    }

    fn set_resizable(&mut self, r: bool) { state_write!(self.state, resizable, r); self.ops.os_set_resizable(r); }
    fn is_maximized(&self) -> bool { state_read!(self.state, maximized) }
    fn is_minimized(&self) -> bool { state_read!(self.state, minimized) }
    fn maximize(&mut self) { state_write!(self.state, maximized, true); state_write!(self.state, minimized, false); self.ops.os_maximize(); }
    fn minimize(&mut self) { state_write!(self.state, minimized, true); state_write!(self.state, maximized, false); self.ops.os_minimize(); }
    fn restore(&mut self) { state_write!(self.state, maximized, false); state_write!(self.state, minimized, false); self.ops.os_restore(); }
    fn set_borderless(&mut self, b: bool) { state_write!(self.state, borderless, b); self.ops.os_set_borderless(b); }

    fn set_fullscreen(&mut self, f: bool) {
        if f == state_read!(self.state, fullscreen) { return; }
        state_write!(self.state, fullscreen, f);
        self.ops.os_set_fullscreen(f);
    }
    fn is_fullscreen(&self) -> bool { state_read!(self.state, fullscreen) }
    fn set_always_on_top(&mut self, on: bool) { state_write!(self.state, always_on_top, on); self.ops.os_set_always_on_top(on); }
    fn set_window_opacity(&mut self, opacity: f32) { state_write!(self.state, opacity, opacity); self.ops.os_set_opacity(opacity); }
    fn start_text_input(&mut self) { state_write!(self.state, text_input_active, true); self.ops.os_start_text_input(); }
    fn stop_text_input(&mut self) { state_write!(self.state, text_input_active, false); self.ops.os_stop_text_input(); }
    fn enable_file_drop(&mut self, enable: bool) { state_write!(self.state, file_drop_enabled, enable); self.ops.os_enable_file_drop(enable); }
}

// ════════════════════════════════════════════════════════════════════════════
// INativeHandle 共享实现
// ════════════════════════════════════════════════════════════════════════════

impl<O: WindowOps> INativeHandle for PlatformWindowCore<O> {
    fn native_window(&self) -> *mut std::ffi::c_void {
        self.ops.native_handle()
    }
}
