//! 窗口协议 — 窗口创建、属性与生命周期。

use crate::core::error::{Error, Result};
use crate::core::geometry::Point;
use super::present::{IGraphicsContext, IPresenter};

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
    fn show(&mut self);
    fn hide(&mut self);
    fn close(&mut self);
    fn is_visible(&self) -> bool;
    fn set_title(&mut self, title: &str);
    fn center_on_screen(&mut self);
    fn raise(&mut self);
    fn lower(&mut self);
    fn set_window_icon(&mut self, icon_path: &str);
    fn flash_window(&mut self);
    fn resize_notify(&mut self, width: i32, height: i32);
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
