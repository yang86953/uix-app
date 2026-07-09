//! 窗口协议 — 窗口创建、属性与生命周期。

use super::present::{IGraphicsContext, IPresenter};
use crate::core::error::{Error, Result};
use crate::core::geometry::Point;
use crate::core::WindowId;

pub trait IWindowProperties {
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    fn set_size(&mut self, w: i32, h: i32) -> Result<()>;
    fn set_minimum_size(&mut self, w: i32, h: i32) -> Result<()>;
    fn set_maximum_size(&mut self, w: i32, h: i32) -> Result<()>;
    fn position(&self) -> Point;
    fn set_position(&mut self, x: i32, y: i32) -> Result<()>;
    fn set_resizable(&mut self, resizable: bool) -> Result<()>;
    fn is_maximized(&self) -> bool;
    fn is_minimized(&self) -> bool;
    fn maximize(&mut self) -> Result<()>;
    fn minimize(&mut self) -> Result<()>;
    fn restore(&mut self) -> Result<()>;
    fn set_borderless(&mut self, borderless: bool) -> Result<()>;
    fn set_fullscreen(&mut self, fullscreen: bool) -> Result<()>;
    fn is_fullscreen(&self) -> bool;
    fn set_always_on_top(&mut self, on: bool) -> Result<()>;
    fn set_window_opacity(&mut self, opacity: f32) -> Result<()>;
    fn start_text_input(&mut self) -> Result<()>;
    fn stop_text_input(&mut self) -> Result<()>;
    fn enable_file_drop(&mut self, enable: bool) -> Result<()>;
}

pub trait INativeHandle {
    fn native_window(&self) -> *mut std::ffi::c_void;
}

pub trait IWindowManager {
    fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<Box<dyn PlatformWindow>, Error>;
}

/// 平台窗口 — 可见性、标题、层级与呈现器访问。
pub trait PlatformWindow {
    fn window_id(&self) -> WindowId;
    fn show(&mut self) -> Result<()>;
    fn hide(&mut self) -> Result<()>;
    fn close(&mut self) -> Result<()>;
    fn is_visible(&self) -> bool;
    fn set_title(&mut self, title: &str) -> Result<()>;
    fn center_on_screen(&mut self) -> Result<()>;
    fn raise(&mut self) -> Result<()>;
    fn lower(&mut self) -> Result<()>;
    fn set_window_icon(&mut self, icon_path: &str) -> Result<()>;
    fn flash_window(&mut self) -> Result<()>;
    fn resize_notify(&mut self, width: i32, height: i32) -> Result<()>;
    fn properties(&self) -> &dyn IWindowProperties;
    fn properties_mut(&mut self) -> &mut dyn IWindowProperties;
    fn presenter(&mut self) -> &mut dyn IPresenter;
    fn native_handle(&self) -> &dyn INativeHandle;

    fn graphics_context(&mut self) -> Option<&mut dyn IGraphicsContext> {
        None
    }

    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}
