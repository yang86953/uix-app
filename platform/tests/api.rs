use std::ffi::c_void;
use uix_platform::api::traits::*;
use uix_platform::api::types::*;
use uix_platform::presenter::NullPresenter;
use uix_platform::Error;

// ════════════════════════════════════════════════════════════════════════════
// 1. 模块重导出完整性 — 确保 api 模块正确暴露所有类型和 trait
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn api_module_re_exports_traits() {
    // 验证 trait 可通过 api::traits 访问
    let _: &dyn IPresenter = &NullPresenter;
    let _: &dyn IClipboard = &MockClipboard;
    let _: &dyn ICursor = &MockCursor;
    let _: &dyn IKeyboard = &MockKeyboard;
    let _: &dyn IDisplay = &MockDisplay;
    let _: &dyn IConsole = &MockConsole;
    let _: &dyn IFileDialog = &MockFileDialog;
    let _: &dyn IFileSystem = &MockFileSystem;
    let _: &dyn INotification = &MockNotification;
    let _: &dyn ITimer = &MockTimer;
    let _: &dyn IEventLoop = &MockEventLoop;
    let _: &dyn IWindowManager = &MockWindowManager;
    let _: &dyn ISystemInfo = &MockSystemInfo;
    let _: &dyn ITextInput = &MockTextInput;
    let _: &dyn Platform = &MockPlatformAggregate::new();
}

#[test]
fn api_module_re_exports_types() {
    let _p = Point::new(1.0, 2.0);
    let _s = Size::new(100.0, 200.0);
    let _r = Rect::new(0.0, 0.0, 100.0, 200.0);
    let _e = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
    let _ = Error::new(Errc::None, "test");
    let _ = StatusLevel::Info;
    let _ = ConsoleColor::Default;
    let _ = CursorType::Arrow;
    let _ = KeyCode::A;
    let _ = KeyMod::NONE;
    let _ = ScrollDirection::Both;
    let _ = ControlSize::Medium;
    let _ = SpecialDir::Home;
    let _ = DisplayInfo::default();
    let _ = OsInfo { name: "".into(), version: "".into(), build: "".into(), is_64bit: false };
    let _ = MemoryInfo { total_bytes: 0, available_bytes: 0, process_working_set: 0, process_private_bytes: 0 };
    let _ = TerminalCapabilities { has_color: false, has_raw_mode: false, has_cursor_control: false };
    let _bus = EventBus::new();
    let _ = UiEventType::WindowClose;
    let _ = UiEventPayload::None;
}

#[test]
fn api_prelude_via_star_import() {
    use uix_platform::api::*;
    // types
    let _p = Point::new(0.0, 0.0);
    let _err: Error = Error::new(Errc::None, "");
    let _ev = UiEvent::close();
    let _bus = EventBus::new();
    let _kc = KeyCode::Enter;
    // traits
    let _presenter: &dyn IPresenter = &NullPresenter;
}

// ════════════════════════════════════════════════════════════════════════════
// 2. Trait 编译验证 — 所有公开 trait 可实现
// ════════════════════════════════════════════════════════════════════════════

struct MockClipboard;
impl IClipboard for MockClipboard {
    fn text(&self) -> String { String::new() }
    fn set_text(&mut self, _text: &str) {}
    fn has_text(&self) -> bool { false }
}

struct MockCursor;
impl ICursor for MockCursor {
    fn set_cursor(&mut self, _cursor: CursorType) {}
    fn show_cursor(&mut self, _visible: bool) {}
    fn cursor_position(&self) -> Point { Point::zero() }
    fn set_cursor_position(&mut self, _x: i32, _y: i32) {}
    fn confine_cursor(&mut self, _confine: bool) {}
    fn capture_mouse(&mut self) {}
    fn release_mouse(&mut self) {}
}

struct MockKeyboard;
impl IKeyboard for MockKeyboard {
    fn is_down(&self, _key: KeyCode) -> bool { false }
    fn idle_ms(&self) -> u32 { 0 }
    fn double_click_ms(&self) -> u32 { 500 }
}

struct MockDisplay;
impl IDisplay for MockDisplay {
    fn dpi_scale(&self) -> f32 { 1.0 }
    fn is_dark_mode(&self) -> bool { false }
    fn count(&self) -> i32 { 1 }
    fn info(&self, _index: i32) -> DisplayInfo { DisplayInfo::default() }
}

struct MockConsole;
impl IConsole for MockConsole {
    fn write(&mut self, _text: &str) {}
    fn write_line(&mut self, _text: &str) {}
    fn set_color(&mut self, _color: ConsoleColor) {}
    fn reset_color(&mut self) {}
    fn show_terminal_cursor(&mut self, _visible: bool) {}
    fn set_terminal_title(&mut self, _title: &str) {}
    fn capabilities(&self) -> TerminalCapabilities {
        TerminalCapabilities { has_color: true, has_raw_mode: false, has_cursor_control: true }
    }
}

struct MockFileDialog;
impl IFileDialog for MockFileDialog {
    fn open(&mut self, _title: &str, _filters: &str) -> Vec<String> { vec![] }
    fn save(&mut self, _title: &str, _filters: &str) -> String { String::new() }
    fn open_folder(&mut self, _title: &str) -> String { String::new() }
}

struct MockFileSystem;
impl IFileSystem for MockFileSystem {
    fn get_special_dir(&self, _dir: SpecialDir) -> String { String::new() }
    fn executable_path(&self) -> String { String::new() }
    fn executable_dir(&self) -> String { String::new() }
    fn read_file(&self, _path: &str) -> Result<Vec<u8>, Error> { Ok(vec![]) }
}

struct MockNotification;
impl INotification for MockNotification {
    fn show(&mut self, _title: &str, _message: &str) {}
}

struct MockTimer;
impl ITimer for MockTimer {
    fn set(&mut self, _interval_ms: u32, _repeating: bool) -> u32 { 1 }
    fn clear(&mut self, _id: u32) {}
}

struct MockEventLoop;
impl IEventLoop for MockEventLoop {
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        let _ = callback;
        false
    }
    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        let _ = callback;
        false
    }
    fn wait_timeout(
        &mut self,
        _timeout: std::time::Duration,
        callback: &dyn Fn(&UiEvent) -> bool,
    ) -> bool {
        let _ = callback;
        false
    }
}

struct MockWindowManager;
impl IWindowManager for MockWindowManager {
    fn create_window(
        &mut self,
        _title: &str,
        _width: i32,
        _height: i32,
    ) -> Result<Box<dyn PlatformWindow>, Error> {
        Err(Error::new(Errc::NotImplemented, "mock"))
    }
}

struct MockSystemInfo;
impl ISystemInfo for MockSystemInfo {
    fn os_info(&self) -> OsInfo {
        OsInfo { name: "MockOS".into(), version: "1.0".into(), build: "1".into(), is_64bit: false }
    }
    fn cpu_count(&self) -> u32 { 4 }
    fn memory_info(&self) -> MemoryInfo {
        MemoryInfo { total_bytes: 0, available_bytes: 0, process_working_set: 0, process_private_bytes: 0 }
    }
    fn hostname(&self) -> String { "mock".into() }
    fn username(&self) -> String { "mock".into() }
    fn up_time(&self) -> u64 { 0 }
    fn default_font_path(&self) -> Option<String> { None }
}

struct MockTextInput;
impl ITextInput for MockTextInput {
    fn start(&mut self) {}
    fn stop(&mut self) {}
}

struct MockPlatformAggregate {
    event_bus: EventBus,
    window_manager: MockWindowManager,
    event_loop: MockEventLoop,
    clipboard: MockClipboard,
    cursor: MockCursor,
    display: MockDisplay,
    file_dialog: MockFileDialog,
    keyboard: MockKeyboard,
    text_input: MockTextInput,
    timer: MockTimer,
    notification: MockNotification,
    console: MockConsole,
    file_system: MockFileSystem,
    system_info: MockSystemInfo,
}

impl MockPlatformAggregate {
    fn new() -> Self {
        Self {
            event_bus: EventBus::new(),
            window_manager: MockWindowManager,
            event_loop: MockEventLoop,
            clipboard: MockClipboard,
            cursor: MockCursor,
            display: MockDisplay,
            file_dialog: MockFileDialog,
            keyboard: MockKeyboard,
            text_input: MockTextInput,
            timer: MockTimer,
            notification: MockNotification,
            console: MockConsole,
            file_system: MockFileSystem,
            system_info: MockSystemInfo,
        }
    }
}

impl Platform for MockPlatformAggregate {
    fn window_manager(&mut self) -> &mut dyn IWindowManager { &mut self.window_manager }
    fn event_loop(&mut self) -> &mut dyn IEventLoop { &mut self.event_loop }
    fn event_bus(&mut self) -> &mut EventBus { &mut self.event_bus }
    fn clipboard(&mut self) -> &mut dyn IClipboard { &mut self.clipboard }
    fn cursor(&mut self) -> &mut dyn ICursor { &mut self.cursor }
    fn display(&self) -> &dyn IDisplay { &self.display }
    fn file_dialog(&mut self) -> &mut dyn IFileDialog { &mut self.file_dialog }
    fn keyboard(&self) -> &dyn IKeyboard { &self.keyboard }
    fn text_input(&mut self) -> &mut dyn ITextInput { &mut self.text_input }
    fn timer(&mut self) -> &mut dyn ITimer { &mut self.timer }
    fn notification(&mut self) -> &mut dyn INotification { &mut self.notification }
    fn console(&mut self) -> &mut dyn IConsole { &mut self.console }
    fn file_system(&self) -> &dyn IFileSystem { &self.file_system }
    fn system_info(&self) -> &dyn ISystemInfo { &self.system_info }
}

// ════════════════════════════════════════════════════════════════════════════
// 3. NullPresenter — IPresenter 空实现行为验证
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn null_presenter_new_via_api() {
    let p = NullPresenter::new();
    let _ = p;
}

#[test]
fn null_presenter_present_discards_all_pixels() {
    let mut p = NullPresenter::new();
    assert!(IPresenter::present(&mut p, &[0u32; 100], 10, 10, None).is_ok());
    assert!(IPresenter::present(&mut p, &[], 0, 0, None).is_ok());
    assert!(IPresenter::present(&mut p, &[1, 2, 3], 1, 3, Some((0, 0, 1, 3))).is_ok());
}

#[test]
fn null_presenter_resize_noop() {
    let mut p = NullPresenter::new();
    assert!(IPresenter::resize(&mut p, 1920, 1080).is_ok());
    assert!(IPresenter::resize(&mut p, 0, 0).is_ok());
    assert!(IPresenter::resize(&mut p, -1, -1).is_ok());
}

#[test]
fn null_presenter_is_default() {
    let p1 = NullPresenter::new();
    let p2 = NullPresenter::default();
    let _ = p1;
    let _ = p2;
}

// ════════════════════════════════════════════════════════════════════════════
// 4. IWindowProperties — 属性状态测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn window_properties_position_roundtrip() {
    let mut w = MockWindowProperties::default();
    assert_eq!(w.position().x, 0.0);
    assert_eq!(w.position().y, 0.0);
    w.set_position(100, 200);
    assert_eq!(w.x, 100);
    assert_eq!(w.y, 200);
}

#[test]
fn window_properties_fullscreen_toggle() {
    let mut w = MockWindowProperties::default();
    assert!(!w.is_fullscreen());
    w.set_fullscreen(true);
    assert!(w.is_fullscreen());
    w.set_fullscreen(false);
    assert!(!w.is_fullscreen());
}

#[test]
fn window_properties_maximize_minimize_restore() {
    let mut w = MockWindowProperties::default();
    assert!(!w.is_maximized());
    assert!(!w.is_minimized());
    w.maximize();
    assert!(w.is_maximized());
    w.minimize();
    assert!(w.is_minimized());
    assert!(!w.is_maximized());
    w.restore();
    assert!(!w.is_maximized());
    assert!(!w.is_minimized());
}

#[test]
fn window_properties_resizable_borderless_opacity() {
    let mut w = MockWindowProperties::default();
    assert!(w.is_maximized() == false || w.is_maximized() == true); // 确保不 panic
    w.set_resizable(false);
    w.set_borderless(true);
    w.set_window_opacity(0.5);
    w.set_always_on_top(true);
    w.start_text_input();
    w.stop_text_input();
    w.enable_file_drop(true);
}

// ════════════════════════════════════════════════════════════════════════════
// 5. IGraphicsContext 默认方法
// ════════════════════════════════════════════════════════════════════════════

struct MockGraphicsContext;

impl IGraphicsContext for MockGraphicsContext {
    fn initialize(&mut self, _: *mut c_void, _: i32, _: i32) -> Result<(), Error> { Ok(()) }
    fn resize(&mut self, _: i32, _: i32) {}
    fn make_current(&mut self) {}
    fn swap_buffers(&mut self) {}
    fn shutdown(&mut self) {}
    fn read_pixels(&mut self, _: i32, _: i32, _: i32, _: i32) -> Vec<u32> { vec![] }
    fn width(&self) -> i32 { 800 }
    fn height(&self) -> i32 { 600 }
}

#[test]
fn graphics_context_get_proc_address_default_returns_none() {
    assert!(MockGraphicsContext.get_proc_address("glClear").is_none());
    assert!(MockGraphicsContext.get_proc_address("").is_none());
}

// ════════════════════════════════════════════════════════════════════════════
// 6. PlatformWindow 默认方法
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn platform_window_graphics_context_default_returns_none() {
    let mut win = MockPlatformWindow::new();
    assert!(win.graphics_context().is_none());
}

#[test]
fn platform_window_native_surface_ptr_default_returns_null() {
    let win = MockPlatformWindow::new();
    assert!(win.native_surface_ptr().is_null());
}

// ════════════════════════════════════════════════════════════════════════════
// 7. ISystemInfo 默认方法
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn system_info_default_font_paths_empty_when_default_returns_none() {
    assert!(MockSystemInfo.default_font_paths().is_empty());
}

#[test]
fn system_info_default_font_paths_single_item_when_default_returns_some() {
    let info = InfoWithFont;
    let paths = info.default_font_paths();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0], "/mock/font.ttf");
}

#[test]
fn system_info_default_methods_return_defaults() {
    assert!(MockSystemInfo.probe_cjk_font_path().is_none());
    assert!(MockSystemInfo.probe_family_font_path("Arial").is_none());
    assert!(MockSystemInfo.scan_fallback_font_path().is_none());
    assert_eq!(MockSystemInfo.process_memory(), (0, 0));
}

struct InfoWithFont;
impl ISystemInfo for InfoWithFont {
    fn os_info(&self) -> OsInfo {
        OsInfo { name: "MockOS".into(), version: "1.0".into(), build: "1".into(), is_64bit: false }
    }
    fn cpu_count(&self) -> u32 { 2 }
    fn memory_info(&self) -> MemoryInfo {
        MemoryInfo { total_bytes: 0, available_bytes: 0, process_working_set: 0, process_private_bytes: 0 }
    }
    fn hostname(&self) -> String { String::new() }
    fn username(&self) -> String { String::new() }
    fn up_time(&self) -> u64 { 0 }
    fn default_font_path(&self) -> Option<String> { Some("/mock/font.ttf".into()) }
}

// ════════════════════════════════════════════════════════════════════════════
// 8. Platform 工厂函数
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn create_gpu_context_on_windows_returns_error() {
    let result = uix_platform::create_gpu_context(
        std::ptr::null_mut(),
        800,
        600,
    );
    assert!(result.is_err());
}

#[test]
fn available_memory_bytes_returns_non_zero() {
    let mem = uix_platform::available_memory_bytes();
    assert!(mem > 0, "可用内存应大于 0，实际为 {}", mem);
}

#[test]
fn create_platform_returns_ok() {
    let mut platform = uix_platform::create_platform().unwrap();
    // 验证 Platform trait 所有 accessor 不 panic
    let _ = platform.event_bus();
    let _ = platform.display();
    let _ = platform.keyboard();
    let _ = platform.file_system();
    let _ = platform.system_info();
    let _ = platform.window_manager();
    let _ = platform.event_loop();
    let _ = platform.clipboard();
    let _ = platform.cursor();
    let _ = platform.file_dialog();
    let _ = platform.text_input();
    let _ = platform.timer();
    let _ = platform.notification();
    let _ = platform.console();
}

// ════════════════════════════════════════════════════════════════════════════
// 9. Platform Mock — 聚合 trait 行为测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn platform_mock_all_accessors_work() {
    let mut p = MockPlatformAggregate::new();
    // &mut self accessors
    let _ = p.window_manager();
    let _ = p.event_loop();
    let _ = p.event_bus();
    let _ = p.clipboard();
    let _ = p.cursor();
    let _ = p.file_dialog();
    let _ = p.text_input();
    let _ = p.timer();
    let _ = p.notification();
    let _ = p.console();
    // &self accessors
    let _ = p.display();
    let _ = p.keyboard();
    let _ = p.file_system();
    let _ = p.system_info();
}

// ════════════════════════════════════════════════════════════════════════════
// 11. Error 类型 — 通过 api 模块构造和使用
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn error_construction_via_api() {
    let e = Error::new(Errc::NotFound, "file not found");
    assert_eq!(e.code(), Errc::NotFound);
    assert!(e.what().contains("file not found"));
}

#[test]
fn error_severity_via_api() {
    let info = Error::info(Errc::None, "info msg");
    assert_eq!(info.severity(), ErrorSeverity::Info);
    let warn = Error::warn(Errc::None, "warn msg");
    assert_eq!(warn.severity(), ErrorSeverity::Warning);
    let err = Error::new(Errc::Unknown, "error msg");
    assert_eq!(err.severity(), ErrorSeverity::Error);
    let fatal = Error::fatal(Errc::Unknown, "fatal msg");
    assert_eq!(fatal.severity(), ErrorSeverity::Fatal);
}

#[test]
fn error_chaining_via_api() {
    let inner = Error::new(Errc::NotFound, "inner");
    let outer = Error::new(Errc::InvalidOperation, "outer").with_source(inner);
    assert_eq!(outer.depth(), 1);
    assert!(outer.what().contains("outer"));
    assert!(outer.root_cause().what().contains("inner"));
}

#[test]
fn error_factory_methods_via_api() {
    assert_eq!(Error::not_found("test").code(), Errc::NotFound);
    assert_eq!(Error::invalid_arg("test").code(), Errc::InvalidArgument);
    assert_eq!(Error::invalid_state("test").code(), Errc::InvalidState);
    assert_eq!(Error::io_error("test").code(), Errc::IoError);
    assert_eq!(Error::not_implemented("test").code(), Errc::NotImplemented);
    assert_eq!(Error::unknown("test").code(), Errc::Unknown);
    assert_eq!(Error::write_failure("test").code(), Errc::WriteFailure);
}

// ════════════════════════════════════════════════════════════════════════════
// 10. EventBus — 通过 api 模块访问
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn event_bus_via_api_module() {
    let mut bus = EventBus::new();
    assert_eq!(bus.subscriber_count(), 0);
    bus.subscribe(UiEventType::WindowClose, |_| true);
    assert_eq!(bus.subscriber_count(), 1);
}

// ════════════════════════════════════════════════════════════════════════════
// 11. 几何类型 — 通过 api 模块访问
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn geometry_types_via_api() {
    let p1 = Point::new(3.0, 4.0);
    let p2 = Point::new(1.0, 2.0);
    let mid = Point::midpoint(p1, p2);
    assert_eq!(mid.x, 2.0);
    assert_eq!(mid.y, 3.0);
    assert_eq!(Point::zero(), Point::default());
}

// ════════════════════════════════════════════════════════════════════════════
// 12. UiEvent 工厂 — 通过 api 模块访问
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn ui_event_factories_via_api() {
    let ev = UiEvent::close();
    assert_eq!(ev.type_, UiEventType::WindowClose);
    let ev = UiEvent::mouse_down(Point::new(10.0, 10.0), MouseButton::Left);
    assert_eq!(ev.type_, UiEventType::MouseDown);
    let ev = UiEvent::key_down(KeyCode::Escape, KeyMod::NONE);
    assert_eq!(ev.type_, UiEventType::KeyDown);
}

// ════════════════════════════════════════════════════════════════════════════
// 辅助结构体
// ════════════════════════════════════════════════════════════════════════════

struct MockWindowProperties {
    x: i32, y: i32, w: i32, h: i32,
    maximized: bool, minimized: bool, fullscreen: bool,
}

impl Default for MockWindowProperties {
    fn default() -> Self {
        Self { x: 0, y: 0, w: 800, h: 600, maximized: false, minimized: false, fullscreen: false }
    }
}

impl IWindowProperties for MockWindowProperties {
    fn width(&self) -> i32 { self.w }
    fn height(&self) -> i32 { self.h }
    fn set_size(&mut self, w: i32, h: i32) { self.w = w; self.h = h; }
    fn set_minimum_size(&mut self, _: i32, _: i32) {}
    fn set_maximum_size(&mut self, _: i32, _: i32) {}
    fn position(&self) -> Point { Point::new(self.x as f32, self.y as f32) }
    fn set_position(&mut self, x: i32, y: i32) { self.x = x; self.y = y; }
    fn set_resizable(&mut self, _: bool) {}
    fn is_maximized(&self) -> bool { self.maximized }
    fn is_minimized(&self) -> bool { self.minimized }
    fn maximize(&mut self) { self.maximized = true; self.minimized = false; }
    fn minimize(&mut self) { self.minimized = true; self.maximized = false; }
    fn restore(&mut self) { self.maximized = false; self.minimized = false; }
    fn set_borderless(&mut self, _: bool) {}
    fn set_fullscreen(&mut self, v: bool) { self.fullscreen = v; }
    fn is_fullscreen(&self) -> bool { self.fullscreen }
    fn set_always_on_top(&mut self, _: bool) {}
    fn set_window_opacity(&mut self, _: f32) {}
    fn start_text_input(&mut self) {}
    fn stop_text_input(&mut self) {}
    fn enable_file_drop(&mut self, _: bool) {}
}

struct MockPlatformWindow {
    props: MockWindowProperties,
    pres: NullPresenter,
}

impl MockPlatformWindow {
    fn new() -> Self {
        Self { props: MockWindowProperties::default(), pres: NullPresenter }
    }
}

impl PlatformWindow for MockPlatformWindow {
    fn show(&mut self) {}
    fn hide(&mut self) {}
    fn close(&mut self) {}
    fn is_visible(&self) -> bool { false }
    fn set_title(&mut self, _: &str) {}
    fn center_on_screen(&mut self) {}
    fn raise(&mut self) {}
    fn lower(&mut self) {}
    fn set_window_icon(&mut self, _: &str) {}
    fn flash_window(&mut self) {}
    fn resize_notify(&mut self, _: i32, _: i32) {}
    fn properties(&self) -> &dyn IWindowProperties { &self.props }
    fn properties_mut(&mut self) -> &mut dyn IWindowProperties { &mut self.props }
    fn presenter(&mut self) -> &mut dyn IPresenter { &mut self.pres }
    fn native_handle(&self) -> &dyn INativeHandle { &MockNativeHandle }
}

struct MockNativeHandle;
impl INativeHandle for MockNativeHandle {
    fn native_window(&self) -> *mut c_void { std::ptr::null_mut() }
}
