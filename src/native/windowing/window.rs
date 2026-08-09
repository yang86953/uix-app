//! 窗口协议 — 窗口创建、属性与生命周期。

use super::event::FrameRequestToken;
use crate::core::error::{Error, Result};
use crate::core::geometry::Point;
use crate::core::WindowId;
use crate::native::present::IPresenter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFrameRequestPhase {
    /// The callback becomes eligible after the frame currently being
    /// assembled is committed. Display-paced APIs such as wl_surface.frame
    /// implement this phase.
    AfterPresent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFrameRequest {
    pub token: FrameRequestToken,
    pub phase: NativeFrameRequestPhase,
}

/// Current compositor knowledge about whether this window can contribute
/// visible pixels. `Unknown` means the backend has no exact per-window query;
/// callers must retain any backend-specific recovery mechanism.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WindowOcclusionState {
    #[default]
    Unknown,
    Visible,
    Occluded,
}

impl NativeFrameRequest {
    pub const fn after_present(token: FrameRequestToken) -> Self {
        Self {
            token,
            phase: NativeFrameRequestPhase::AfterPresent,
        }
    }
}

pub trait IWindowProperties {
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    /// 设置正的 logical 客户区尺寸；已有最小/最大约束时必须落在其内。
    fn set_size(&mut self, w: i32, h: i32) -> Result<()>;
    /// 设置正的 logical 客户区最小尺寸，且不得超过已登记的最大尺寸。
    fn set_minimum_size(&mut self, w: i32, h: i32) -> Result<()>;
    /// 设置正的 logical 客户区最大尺寸，且不得小于已登记的最小尺寸。
    fn set_maximum_size(&mut self, w: i32, h: i32) -> Result<()>;
    fn position(&self) -> Point;
    fn set_position(&mut self, x: i32, y: i32) -> Result<()>;
    fn set_resizable(&mut self, resizable: bool) -> Result<()>;
    fn is_maximized(&self) -> bool;
    fn is_minimized(&self) -> bool;
    fn maximize(&mut self) -> Result<()>;
    fn minimize(&mut self) -> Result<()>;
    fn restore(&mut self) -> Result<()>;
    /// 显示或隐藏系统非客户区标题栏，同时保留后端可用的窗口缩放边框。
    fn set_system_title_bar_visible(&mut self, visible: bool) -> Result<()>;
    fn set_borderless(&mut self, borderless: bool) -> Result<()>;
    /// 切换全屏；退出时恢复进入前的位置与窗口样式。
    fn set_fullscreen(&mut self, fullscreen: bool) -> Result<()>;
    fn is_fullscreen(&self) -> bool;
    fn set_always_on_top(&mut self, on: bool) -> Result<()>;
    /// 设置窗口整体不透明度，取值必须为有限的 `0.0..=1.0`。
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
    /// 请求走平台正常关闭事件，使应用先释放图形与会话资源。
    fn request_close(&mut self) -> Result<()>;
    /// 将当前鼠标手势移交给窗口管理器执行原生窗口拖动。
    fn begin_move_drag(&mut self) -> Result<()>;
    /// 在当前指针位置显示窗口管理器提供的原生系统菜单。
    fn show_system_menu(&mut self) -> Result<()>;
    fn is_visible(&self) -> bool;

    /// Returns the latest exact compositor visibility when the backend can
    /// query it. This is intentionally distinct from `is_visible`: a window
    /// may be onscreen while fully covered by other windows.
    fn occlusion_state(&self) -> WindowOcclusionState {
        WindowOcclusionState::Unknown
    }

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

    /// Arms a native one-shot callback. `Ok(false)` means this window does
    /// not support the requested phase and the scheduler must use fallback.
    fn request_native_frame(&mut self, _request: NativeFrameRequest) -> Result<bool> {
        Ok(false)
    }

    /// Confirms that the frame associated with `token` was committed.
    /// Backends whose native wait must begin after presentation use this hook;
    /// pre-commit protocols can keep the default no-op.
    fn native_frame_presented(&mut self, _token: FrameRequestToken) -> Result<()> {
        Ok(())
    }

    /// Invalidates a still-in-flight native request. Backends that cannot
    /// cancel the OS object must at least suppress delivery for this token.
    fn cancel_native_frame(&mut self, _token: FrameRequestToken) -> Result<()> {
        Ok(())
    }

    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}
