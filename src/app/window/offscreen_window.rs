//! Agent 专属离屏窗口：只拥有内存视口与私有剪贴板，不创建任何原生对象。

use crate::core::{Errc, Error, Point, PresentDamage, Result, WindowId};
use crate::draw::SurfaceReadback;
use crate::draw::renderer::test_harness::GraphicsFaultSignal;
use crate::platform::presentation::IPresenter;
use crate::platform::windowing::event::{FrameRequestToken, PointerActivationId};
use crate::platform::windowing::window::{
    INativeHandle, IWindowProperties, NativeFrameRequest, PlatformWindow,
};
use crate::platform::windowing::{IClipboard, WindowCapabilities, WindowResizeEdge};

// 把像素和内存预算固定在同一个入口；协议中的 resize 也必须经过这里。
pub(crate) fn validate_extent(width: i32, height: i32) -> Result<()> {
    if width <= 0
        || height <= 0
        || width > 4096
        || height > 4096
        || i64::from(width) * i64::from(height) > 8_388_608
    {
        return Err(Error::new(
            Errc::InvalidArgument,
            "background viewport exceeds 4096 per axis or 8388608 pixels",
        ));
    }
    Ok(())
}

fn desktop_unavailable() -> Result<()> {
    Err(Error::new(
        Errc::NotImplemented,
        "desktop window operations are unavailable in an isolated agent workspace",
    ))
}

pub(crate) struct OffscreenWindow {
    id: WindowId,
    width: i32,
    height: i32,
    pub(crate) closed: bool,
    readback: GraphicsFaultSignal,
    readback_ready: bool,
}

impl OffscreenWindow {
    pub(crate) fn new(
        id: WindowId,
        width: i32,
        height: i32,
        readback: GraphicsFaultSignal,
    ) -> Result<Self> {
        validate_extent(width, height)?;
        readback.attach_surface_readback();
        Ok(Self {
            id,
            width,
            height,
            closed: false,
            readback,
            readback_ready: false,
        })
    }
}

impl IWindowProperties for OffscreenWindow {
    fn width(&self) -> i32 {
        self.width
    }
    fn height(&self) -> i32 {
        self.height
    }
    fn set_size(&mut self, width: i32, height: i32) -> Result<()> {
        validate_extent(width, height)?;
        self.width = width;
        self.height = height;
        Ok(())
    }
    fn set_minimum_size(&mut self, _: i32, _: i32) -> Result<()> {
        desktop_unavailable()
    }
    fn set_maximum_size(&mut self, _: i32, _: i32) -> Result<()> {
        desktop_unavailable()
    }
    fn position(&self) -> Point {
        Point::new(0.0, 0.0)
    }
    fn set_position(&mut self, _: i32, _: i32) -> Result<()> {
        desktop_unavailable()
    }
    fn set_resizable(&mut self, _: bool) -> Result<()> {
        desktop_unavailable()
    }
    fn is_maximized(&self) -> bool {
        false
    }
    fn is_minimized(&self) -> bool {
        false
    }
    fn maximize(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn minimize(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn restore(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn set_system_title_bar_visible(&mut self, _: bool) -> Result<()> {
        desktop_unavailable()
    }
    fn set_borderless(&mut self, _: bool) -> Result<()> {
        desktop_unavailable()
    }
    fn set_fullscreen(&mut self, _: bool) -> Result<()> {
        desktop_unavailable()
    }
    fn is_fullscreen(&self) -> bool {
        false
    }
    fn set_always_on_top(&mut self, _: bool) -> Result<()> {
        desktop_unavailable()
    }
    fn set_window_opacity(&mut self, _: f32) -> Result<()> {
        desktop_unavailable()
    }
    fn start_text_input(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn stop_text_input(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn enable_file_drop(&mut self, _: bool) -> Result<()> {
        desktop_unavailable()
    }
}

impl INativeHandle for OffscreenWindow {
    fn native_window(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

impl PlatformWindow for OffscreenWindow {
    fn window_id(&self) -> WindowId {
        self.id
    }
    fn is_offscreen(&self) -> bool {
        true
    }
    fn capabilities(&self) -> WindowCapabilities {
        WindowCapabilities::EMPTY
    }
    fn show(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn hide(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn close(&mut self) -> Result<()> {
        self.closed = true;
        Ok(())
    }
    fn request_close(&mut self) -> Result<()> {
        self.close()
    }
    fn begin_move_drag(&mut self, _: Option<PointerActivationId>) -> Result<()> {
        desktop_unavailable()
    }
    fn begin_resize_drag(
        &mut self,
        _: WindowResizeEdge,
        _: Option<PointerActivationId>,
    ) -> Result<()> {
        desktop_unavailable()
    }
    fn show_system_menu(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn is_visible(&self) -> bool {
        false
    }
    fn set_title(&mut self, _: &str) -> Result<()> {
        desktop_unavailable()
    }
    fn center_on_screen(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn raise(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn lower(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn set_window_icon(&mut self, _: &str) -> Result<()> {
        desktop_unavailable()
    }
    fn flash_window(&mut self) -> Result<()> {
        desktop_unavailable()
    }
    fn resize_notify(&mut self, width: i32, height: i32) -> Result<()> {
        self.set_size(width, height)
    }
    fn properties(&self) -> &dyn IWindowProperties {
        self
    }
    fn properties_mut(&mut self) -> &mut dyn IWindowProperties {
        self
    }
    fn presenter(&mut self) -> &mut dyn IPresenter {
        self
    }
    fn native_handle(&self) -> &dyn INativeHandle {
        self
    }
    fn prepare_agent_readback(&mut self) {
        self.readback_ready = true;
    }
    fn request_native_frame(&mut self, _: NativeFrameRequest) -> Result<bool> {
        Ok(false)
    }
    fn native_frame_presented(&mut self, _: FrameRequestToken) -> Result<()> {
        desktop_unavailable()
    }
    fn cancel_native_frame(&mut self, _: FrameRequestToken) -> Result<()> {
        Ok(())
    }
    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

impl IPresenter for OffscreenWindow {
    fn present(&mut self, pixels: &[u32], width: i32, height: i32, _: PresentDamage) -> Result<()> {
        validate_extent(width, height)?;
        if self.closed || pixels.len() != width as usize * height as usize {
            return Err(Error::new(
                Errc::InvalidState,
                "background frame is closed or has invalid pixels",
            ));
        }
        // 只在本次真实 CPU 绘制完成后复制像素；没有读取桌面或历史截图的回退路径。
        if std::mem::take(&mut self.readback_ready) {
            if let Some(request) = self.readback.take_surface_readback() {
                request.complete(Ok(SurfaceReadback::from_argb(
                    width,
                    height,
                    pixels.to_vec(),
                )));
            }
        }
        Ok(())
    }
    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.set_size(width, height)
    }
}

impl Drop for OffscreenWindow {
    fn drop(&mut self) {
        self.readback.cancel_surface_readback();
    }
}

#[derive(Default)]
pub(crate) struct WorkspaceClipboard(String);

impl IClipboard for WorkspaceClipboard {
    fn text(&self) -> Result<String> {
        Ok(self.0.clone())
    }
    fn set_text(&mut self, text: &str) -> Result<()> {
        if text.len() > 1024 * 1024 {
            return Err(Error::new(
                Errc::InvalidArgument,
                "background clipboard exceeds 1 MiB",
            ));
        }
        self.0 = text.to_owned();
        Ok(())
    }
    fn has_text(&self) -> Result<bool> {
        Ok(!self.0.is_empty())
    }
}
