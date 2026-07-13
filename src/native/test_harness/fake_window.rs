//! Fake 窗口系统 — 窗口管理器、窗口实例、窗口属性、原生句柄。

use crate::core::error::Result;
use crate::core::geometry::Point;
use crate::core::WindowId;
use crate::native::test_harness::fake_graphics_context::FakeGraphicsContext;
use crate::native::test_harness::fake_presenter::FakePresenter;
use crate::native::traits::present::{IGraphicsContext, IPresenter};
use crate::native::traits::window::{
    INativeHandle, IWindowManager, IWindowProperties, PlatformWindow,
};

// ════════════════════════════════════════════════════════════════════════════
// FakeWindowProperties
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct FakeWindowPropertiesState {
    pub width: i32,
    pub height: i32,
    pub min_w: i32,
    pub min_h: i32,
    pub max_w: i32,
    pub max_h: i32,
    pub pos_x: i32,
    pub pos_y: i32,
    pub resizable: bool,
    pub maximized: bool,
    pub minimized: bool,
    pub borderless: bool,
    pub fullscreen: bool,
    pub always_on_top: bool,
    pub opacity: f32,
    pub text_input_active: bool,
    pub file_drop_enabled: bool,
    pub set_size_calls: Vec<(i32, i32)>,
    pub set_position_calls: Vec<(i32, i32)>,
}

impl Default for FakeWindowPropertiesState {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            min_w: 100,
            min_h: 100,
            max_w: 0,
            max_h: 0,
            pos_x: 0,
            pos_y: 0,
            resizable: true,
            maximized: false,
            minimized: false,
            borderless: false,
            fullscreen: false,
            always_on_top: false,
            opacity: 1.0,
            text_input_active: false,
            file_drop_enabled: false,
            set_size_calls: Vec::new(),
            set_position_calls: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct FakeWindowProperties {
    pub state: FakeWindowPropertiesState,
}

impl FakeWindowProperties {
    pub fn new() -> Self {
        Self {
            state: FakeWindowPropertiesState::default(),
        }
    }

    /// 清除调用记录（保留当前属性值）
    pub fn clear_history(&mut self) {
        self.state.set_size_calls.clear();
        self.state.set_position_calls.clear();
    }
}

impl Default for FakeWindowProperties {
    fn default() -> Self {
        Self::new()
    }
}

impl IWindowProperties for FakeWindowProperties {
    fn width(&self) -> i32 {
        self.state.width
    }
    fn height(&self) -> i32 {
        self.state.height
    }
    fn set_size(&mut self, w: i32, h: i32) -> Result<()> {
        self.state.width = w;
        self.state.height = h;
        self.state.set_size_calls.push((w, h));
        Ok(())
    }
    fn set_minimum_size(&mut self, w: i32, h: i32) -> Result<()> {
        self.state.min_w = w;
        self.state.min_h = h;
        Ok(())
    }
    fn set_maximum_size(&mut self, w: i32, h: i32) -> Result<()> {
        self.state.max_w = w;
        self.state.max_h = h;
        Ok(())
    }
    fn position(&self) -> Point {
        Point::new(self.state.pos_x as f32, self.state.pos_y as f32)
    }
    fn set_position(&mut self, x: i32, y: i32) -> Result<()> {
        self.state.pos_x = x;
        self.state.pos_y = y;
        self.state.set_position_calls.push((x, y));
        Ok(())
    }
    fn set_resizable(&mut self, r: bool) -> Result<()> {
        self.state.resizable = r;
        Ok(())
    }
    fn is_maximized(&self) -> bool {
        self.state.maximized
    }
    fn is_minimized(&self) -> bool {
        self.state.minimized
    }
    fn maximize(&mut self) -> Result<()> {
        self.state.maximized = true;
        self.state.minimized = false;
        Ok(())
    }
    fn minimize(&mut self) -> Result<()> {
        self.state.minimized = true;
        self.state.maximized = false;
        Ok(())
    }
    fn restore(&mut self) -> Result<()> {
        self.state.maximized = false;
        self.state.minimized = false;
        Ok(())
    }
    fn set_borderless(&mut self, b: bool) -> Result<()> {
        self.state.borderless = b;
        Ok(())
    }
    fn set_fullscreen(&mut self, f: bool) -> Result<()> {
        self.state.fullscreen = f;
        Ok(())
    }
    fn is_fullscreen(&self) -> bool {
        self.state.fullscreen
    }
    fn set_always_on_top(&mut self, on: bool) -> Result<()> {
        self.state.always_on_top = on;
        Ok(())
    }
    fn set_window_opacity(&mut self, o: f32) -> Result<()> {
        self.state.opacity = o;
        Ok(())
    }
    fn start_text_input(&mut self) -> Result<()> {
        self.state.text_input_active = true;
        Ok(())
    }
    fn stop_text_input(&mut self) -> Result<()> {
        self.state.text_input_active = false;
        Ok(())
    }
    fn enable_file_drop(&mut self, e: bool) -> Result<()> {
        self.state.file_drop_enabled = e;
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// FakeNativeHandle
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct FakeNativeHandle {
    pub ptr: *mut std::ffi::c_void,
}

impl FakeNativeHandle {
    pub fn new() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
        }
    }
    pub fn with_ptr(ptr: *mut std::ffi::c_void) -> Self {
        Self { ptr }
    }
}

impl Default for FakeNativeHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl INativeHandle for FakeNativeHandle {
    fn native_window(&self) -> *mut std::ffi::c_void {
        self.ptr
    }
}

// ════════════════════════════════════════════════════════════════════════════
// FakeWindow
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct FakeWindowState {
    pub id: WindowId,
    pub title: String,
    pub visible: bool,
    pub show_calls: usize,
    pub hide_calls: usize,
    pub close_called: bool,
    pub center_called: bool,
    pub raise_calls: usize,
    pub lower_calls: usize,
    pub flash_calls: usize,
    pub icon_path: String,
    pub resize_notify_calls: Vec<(i32, i32)>,
    pub set_title_calls: Vec<String>,
    pub has_gpu: bool,
}

#[derive(Debug)]
pub struct FakeWindow {
    pub id: WindowId,
    pub props: FakeWindowProperties,
    pub presenter: FakePresenter,
    pub native_handle: FakeNativeHandle,
    pub gpu_ctx: Option<FakeGraphicsContext>,
    pub state: FakeWindowState,
}

impl FakeWindow {
    pub fn new(id: u64, title: &str, width: i32, height: i32) -> Self {
        let id = WindowId::new(id);
        let mut props = FakeWindowProperties::new();
        props.state.width = width;
        props.state.height = height;
        Self {
            id,
            props,
            presenter: FakePresenter::new(),
            native_handle: FakeNativeHandle::new(),
            gpu_ctx: None,
            state: FakeWindowState {
                id,
                title: title.to_string(),
                visible: false,
                show_calls: 0,
                hide_calls: 0,
                close_called: false,
                center_called: false,
                raise_calls: 0,
                lower_calls: 0,
                flash_calls: 0,
                icon_path: String::new(),
                resize_notify_calls: Vec::new(),
                set_title_calls: Vec::new(),
                has_gpu: false,
            },
        }
    }

    pub fn with_gpu(mut self) -> Self {
        self.gpu_ctx = Some(FakeGraphicsContext::new());
        self.state.has_gpu = true;
        self
    }

    /// 清除调用记录（保留当前窗口状态）
    pub fn clear_history(&mut self) {
        self.state.show_calls = 0;
        self.state.hide_calls = 0;
        self.state.close_called = false;
        self.state.center_called = false;
        self.state.raise_calls = 0;
        self.state.lower_calls = 0;
        self.state.flash_calls = 0;
        self.state.icon_path.clear();
        self.state.resize_notify_calls.clear();
        self.state.set_title_calls.clear();
        self.props.clear_history();
        self.presenter.clear_history();
    }
}

impl PlatformWindow for FakeWindow {
    fn window_id(&self) -> WindowId {
        self.id
    }

    fn show(&mut self) -> Result<()> {
        self.state.visible = true;
        self.state.show_calls += 1;
        Ok(())
    }
    fn hide(&mut self) -> Result<()> {
        self.state.visible = false;
        self.state.hide_calls += 1;
        Ok(())
    }
    fn close(&mut self) -> Result<()> {
        self.state.visible = false;
        self.state.close_called = true;
        Ok(())
    }
    fn is_visible(&self) -> bool {
        self.state.visible
    }
    fn set_title(&mut self, title: &str) -> Result<()> {
        self.state.title = title.to_string();
        self.state.set_title_calls.push(title.to_string());
        Ok(())
    }
    fn center_on_screen(&mut self) -> Result<()> {
        self.state.center_called = true;
        Ok(())
    }
    fn raise(&mut self) -> Result<()> {
        self.state.raise_calls += 1;
        Ok(())
    }
    fn lower(&mut self) -> Result<()> {
        self.state.lower_calls += 1;
        Ok(())
    }
    fn set_window_icon(&mut self, path: &str) -> Result<()> {
        self.state.icon_path = path.to_string();
        Ok(())
    }
    fn flash_window(&mut self) -> Result<()> {
        self.state.flash_calls += 1;
        Ok(())
    }
    fn resize_notify(&mut self, w: i32, h: i32) -> Result<()> {
        self.props.state.width = w;
        self.props.state.height = h;
        self.state.resize_notify_calls.push((w, h));
        Ok(())
    }
    fn properties(&self) -> &dyn IWindowProperties {
        &self.props
    }
    fn properties_mut(&mut self) -> &mut dyn IWindowProperties {
        &mut self.props
    }
    fn presenter(&mut self) -> &mut dyn IPresenter {
        &mut self.presenter
    }
    fn native_handle(&self) -> &dyn INativeHandle {
        &self.native_handle
    }
    fn graphics_context(&mut self) -> Option<&mut dyn IGraphicsContext> {
        self.gpu_ctx
            .as_mut()
            .map(|g| g as &mut dyn IGraphicsContext)
    }
    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// FakeWindowManager
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct FakeWindowManager {
    /// `create_window` 调用记录
    pub create_calls: Vec<(String, i32, i32)>,
    next_id: u64,
}

impl FakeWindowManager {
    pub fn new() -> Self {
        Self {
            create_calls: Vec::new(),
            next_id: 1,
        }
    }

    pub fn clear_history(&mut self) {
        self.create_calls.clear();
    }
}

impl Default for FakeWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl IWindowManager for FakeWindowManager {
    fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<Box<dyn PlatformWindow>> {
        let id = self.next_id;
        self.next_id += 1;
        self.create_calls.push((title.to_string(), width, height));
        Ok(Box::new(FakeWindow::new(id, title, width, height)))
    }
}

