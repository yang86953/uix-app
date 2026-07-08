use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::{c_char, c_void, CString};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Once;
use std::time::Duration;

use crate::core::{Error, Rect, WindowId};
use crate::native::shared::{
    FileSystemCore, OsEventSource, PlatformWindowCore, SpecialDirProvider, WindowOps, WindowState,
};
use crate::native::traits::display::{DisplayInfo, IDisplay};
use crate::native::traits::event::{EventBus, EventLoopWaker, UiEvent};
use crate::native::traits::input::{
    CursorType, IClipboard, ICursor, IKeyboard, ITextInput, KeyCode,
};
use crate::native::traits::platform::Platform;
use crate::native::traits::present::{IPresenter, PresentDamage};
use crate::native::traits::system::{
    ConsoleColor, IConsole, IFileDialog, IFileSystem, INotification, ISystemInfo, ITimer,
    MemoryInfo, OsInfo, SpecialDir, TerminalCapabilities,
};
use crate::native::traits::window::{IWindowManager, PlatformWindow};

pub struct MacosPlatform {
    event_queue: VecDeque<UiEvent>,
    event_bus: EventBus,
    clipboard: MacosClipboard,
    cursor: MacosCursor,
    display: MacosDisplay,
    file_dialog: MacosFileDialog,
    file_system: MacosFileSystem,
    keyboard: MacosKeyboard,
    text_input: MacosTextInput,
    timer: MacosTimer,
    notification: MacosNotification,
    console: MacosConsole,
    system_info: MacosSystemInfo,
    next_window_id: u64,
}

impl MacosPlatform {
    pub fn new() -> Self {
        Self {
            event_queue: VecDeque::new(),
            event_bus: EventBus::new(),
            clipboard: MacosClipboard::new(),
            cursor: MacosCursor::new(),
            display: MacosDisplay,
            file_dialog: MacosFileDialog,
            file_system: MacosFileSystem::new(),
            keyboard: MacosKeyboard,
            text_input: MacosTextInput,
            timer: MacosTimer::new(),
            notification: MacosNotification,
            console: MacosConsole,
            system_info: MacosSystemInfo,
            next_window_id: 1,
        }
    }
}

impl Default for MacosPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl OsEventSource for MacosPlatform {
    fn dispatch_pending(&mut self) -> bool {
        // SAFETY: Cocoa event dispatch stays on the caller's UI thread and uses
        // AppKit-owned singleton objects; no Rust references cross the FFI boundary.
        unsafe { while cocoa::dispatch_one_event(cocoa::distant_past()) {} }
        true
    }

    fn dispatch_blocking(&mut self) -> bool {
        // SAFETY: See dispatch_pending; distantFuture is an autoreleased NSDate
        // owned by Foundation and valid for the duration of this message send.
        unsafe {
            let _ = cocoa::dispatch_one_event(cocoa::distant_future());
        }
        true
    }

    fn dispatch_timeout(&mut self, timeout: Duration) -> bool {
        // SAFETY: See dispatch_pending; the NSDate is only used synchronously by
        // nextEventMatchingMask and is not retained by Rust.
        unsafe {
            let until = cocoa::date_with_time_interval(timeout.as_secs_f64());
            let _ = cocoa::dispatch_one_event(until);
        }
        true
    }

    fn next_event(&mut self) -> Option<UiEvent> {
        self.event_queue.pop_front()
    }

    fn waker(&self) -> EventLoopWaker {
        EventLoopWaker::default()
    }
}

impl IWindowManager for MacosPlatform {
    fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> crate::core::Result<Box<dyn PlatformWindow>, Error> {
        let window_id = WindowId::new(self.next_window_id);
        self.next_window_id += 1;
        // SAFETY: create_window constructs AppKit objects on the current thread
        // and returns Objective-C object pointers managed by AppKit.
        let (window, content_layer) = unsafe { cocoa::create_window(title, width, height) };
        let state = Rc::new(RefCell::new(WindowState::with_id_and_size(
            window_id, width, height,
        )));
        let ops = MacosWindowOps::new(window);
        let presenter = MacosPresenter::new(content_layer, width, height);
        let core = PlatformWindowCore::new(state, ops, Box::new(presenter));
        Ok(Box::new(core))
    }
}

impl Platform for MacosPlatform {
    fn window_manager(&mut self) -> &mut dyn IWindowManager {
        self
    }

    fn event_loop(&mut self) -> &mut dyn crate::native::traits::event::IEventLoop {
        self
    }

    fn event_bus(&mut self) -> &mut EventBus {
        &mut self.event_bus
    }

    fn clipboard(&mut self) -> &mut dyn IClipboard {
        &mut self.clipboard
    }

    fn cursor(&mut self) -> &mut dyn ICursor {
        &mut self.cursor
    }

    fn display(&self) -> &dyn IDisplay {
        &self.display
    }

    fn file_dialog(&mut self) -> &mut dyn IFileDialog {
        &mut self.file_dialog
    }

    fn keyboard(&self) -> &dyn IKeyboard {
        &self.keyboard
    }

    fn text_input(&mut self) -> &mut dyn ITextInput {
        &mut self.text_input
    }

    fn timer(&mut self) -> &mut dyn ITimer {
        &mut self.timer
    }

    fn notification(&mut self) -> &mut dyn INotification {
        &mut self.notification
    }

    fn console(&mut self) -> &mut dyn IConsole {
        &mut self.console
    }

    fn file_system(&self) -> &dyn IFileSystem {
        &self.file_system
    }

    fn system_info(&self) -> &dyn ISystemInfo {
        &self.system_info
    }
}

struct MacosWindowOps {
    window: cocoa::Id,
}

impl MacosWindowOps {
    fn new(window: cocoa::Id) -> Self {
        Self { window }
    }
}

impl WindowOps for MacosWindowOps {
    fn os_show(&mut self) {
        // SAFETY: self.window is the NSWindow pointer returned by create_window.
        unsafe {
            cocoa::show_window(self.window);
        }
    }

    fn os_hide(&mut self) {
        // SAFETY: self.window is the NSWindow pointer returned by create_window.
        unsafe {
            cocoa::hide_window(self.window);
        }
    }

    fn os_close(&mut self) {
        // SAFETY: self.window is the NSWindow pointer returned by create_window.
        unsafe {
            cocoa::close_window(self.window);
        }
    }

    fn os_set_title(&mut self, title: &str) {
        // SAFETY: self.window is valid and title is converted to a temporary
        // NSString before the synchronous AppKit setter call.
        unsafe {
            cocoa::set_window_title(self.window, title);
        }
    }

    fn os_set_size(&mut self, w: i32, h: i32) {
        // SAFETY: self.window is valid and CGRect is repr(C), matching AppKit ABI.
        unsafe {
            cocoa::set_window_size(self.window, w, h);
        }
    }

    fn native_handle(&self) -> *mut c_void {
        self.window
    }

    fn os_center_on_screen(&mut self) -> crate::core::Result<()> {
        // SAFETY: self.window is the NSWindow pointer returned by create_window.
        unsafe {
            cocoa::msg_void(self.window, "center");
        }
        Ok(())
    }

    fn os_raise(&mut self) -> crate::core::Result<()> {
        unsafe {
            cocoa::show_window(self.window);
        }
        Ok(())
    }

    fn os_lower(&mut self) -> crate::core::Result<()> {
        unsafe {
            cocoa::hide_window(self.window);
        }
        Ok(())
    }
}

struct MacosPresenter {
    layer: cocoa::Id,
    width: i32,
    height: i32,
}

impl MacosPresenter {
    fn new(layer: cocoa::Id, width: i32, height: i32) -> Self {
        Self {
            layer,
            width,
            height,
        }
    }
}

impl IPresenter for MacosPresenter {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        _damage: PresentDamage,
    ) -> crate::core::Result<(), Error> {
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        if width != self.width || height != self.height {
            self.resize(width, height)?;
        }
        // SAFETY: pixels is a live Rust slice for the duration of this call, and
        // set_layer_pixels copies it into CFData/CGImage before returning.
        unsafe {
            cocoa::set_layer_pixels(self.layer, pixels, width, height);
        }
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> crate::core::Result<(), Error> {
        if width > 0 && height > 0 {
            self.width = width;
            self.height = height;
        }
        Ok(())
    }
}

#[derive(Default)]
struct MacosClipboard {
    text: String,
}

impl MacosClipboard {
    fn new() -> Self {
        Self::default()
    }
}

impl IClipboard for MacosClipboard {
    fn text(&self) -> String {
        self.text.clone()
    }

    fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
    }

    fn has_text(&self) -> bool {
        !self.text.is_empty()
    }
}

struct MacosCursor {
    cursor: CursorType,
    position: crate::core::Point,
}

impl MacosCursor {
    fn new() -> Self {
        Self {
            cursor: CursorType::Arrow,
            position: crate::core::Point::zero(),
        }
    }
}

impl ICursor for MacosCursor {
    fn set_cursor(&mut self, cursor: CursorType) {
        self.cursor = cursor;
    }

    fn show_cursor(&mut self, _visible: bool) {}

    fn cursor_position(&self) -> crate::core::Point {
        self.position
    }

    fn set_cursor_position(&mut self, x: i32, y: i32) {
        self.position = crate::core::Point::new(x as f32, y as f32);
    }

    fn confine_cursor(&mut self, _confine: bool) {}

    fn capture_mouse(&mut self) {}

    fn release_mouse(&mut self) {}
}

struct MacosDisplay;

impl IDisplay for MacosDisplay {
    fn dpi_scale(&self) -> f32 {
        1.0
    }

    fn is_dark_mode(&self) -> bool {
        false
    }

    fn count(&self) -> i32 {
        1
    }

    fn info(&self, _index: i32) -> DisplayInfo {
        DisplayInfo {
            bounds: Rect::new(0.0, 0.0, 1440.0, 900.0),
            dpi_scale: self.dpi_scale(),
            is_primary: true,
        }
    }
}

struct MacosFileDialog;

impl IFileDialog for MacosFileDialog {
    fn open(&mut self, _title: &str, _filters: &str) -> Vec<String> {
        Vec::new()
    }

    fn save(&mut self, _title: &str, _filters: &str) -> String {
        String::new()
    }

    fn open_folder(&mut self, _title: &str) -> String {
        String::new()
    }
}

type MacosFileSystem = FileSystemCore<MacosSpecialDirs>;

#[derive(Debug, Clone, Default)]
struct MacosSpecialDirs;

impl SpecialDirProvider for MacosSpecialDirs {
    fn special_dir(&self, dir: SpecialDir) -> String {
        match dir {
            SpecialDir::Home => home_dir(),
            SpecialDir::Temp => std::env::temp_dir().to_string_lossy().to_string(),
            SpecialDir::AppData | SpecialDir::LocalAppData => {
                home_child("Library/Application Support")
            }
            SpecialDir::Documents => home_child("Documents"),
            SpecialDir::Desktop => home_child("Desktop"),
            SpecialDir::Downloads => home_child("Downloads"),
            SpecialDir::Current | SpecialDir::Executable => String::new(),
        }
    }
}

struct MacosKeyboard;

impl IKeyboard for MacosKeyboard {
    fn is_down(&self, _key: KeyCode) -> bool {
        false
    }

    fn idle_ms(&self) -> u32 {
        0
    }

    fn double_click_ms(&self) -> u32 {
        500
    }
}

struct MacosTextInput;

impl ITextInput for MacosTextInput {
    fn start(&mut self) {}

    fn stop(&mut self) {}
}

struct MacosTimer {
    next_id: u32,
}

impl MacosTimer {
    fn new() -> Self {
        Self { next_id: 1 }
    }
}

impl ITimer for MacosTimer {
    fn set(&mut self, _interval_ms: u32, _repeating: bool) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    fn clear(&mut self, _id: u32) {}
}

struct MacosNotification;

impl INotification for MacosNotification {
    fn show(&mut self, _title: &str, _message: &str) {}
}

struct MacosConsole;

impl IConsole for MacosConsole {
    fn write(&mut self, text: &str) {
        print!("{text}");
    }

    fn write_line(&mut self, text: &str) {
        println!("{text}");
    }

    fn set_color(&mut self, _color: ConsoleColor) {}

    fn reset_color(&mut self) {}

    fn show_terminal_cursor(&mut self, _visible: bool) {}

    fn set_terminal_title(&mut self, title: &str) {
        print!("\x1b]0;{title}\x07");
    }

    fn capabilities(&self) -> TerminalCapabilities {
        TerminalCapabilities {
            has_color: true,
            has_raw_mode: false,
            has_cursor_control: false,
        }
    }
}

struct MacosSystemInfo;

impl ISystemInfo for MacosSystemInfo {
    fn os_info(&self) -> OsInfo {
        OsInfo {
            name: "macOS".to_string(),
            version: String::new(),
            build: String::new(),
            is_64bit: cfg!(target_pointer_width = "64"),
        }
    }

    fn cpu_count(&self) -> u32 {
        std::thread::available_parallelism()
            .map(|count| count.get() as u32)
            .unwrap_or(1)
    }

    fn memory_info(&self) -> MemoryInfo {
        MemoryInfo {
            total_bytes: 512 * 1024 * 1024,
            available_bytes: 512 * 1024 * 1024,
            process_working_set: 0,
            process_private_bytes: 0,
        }
    }

    fn hostname(&self) -> String {
        std::env::var("HOSTNAME").unwrap_or_default()
    }

    fn username(&self) -> String {
        std::env::var("USER").unwrap_or_default()
    }

    fn up_time(&self) -> u64 {
        0
    }

    fn default_font_paths(&self) -> Vec<String> {
        vec![
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf".to_string(),
            "/System/Library/Fonts/Helvetica.ttc".to_string(),
        ]
    }

    fn probe_cjk_font_path(&self) -> Option<String> {
        find_existing_path(&[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
        ])
    }

    fn probe_family_font_path(&self, family: &str) -> Option<String> {
        match family.to_ascii_lowercase().as_str() {
            "helvetica" => find_existing_path(&["/System/Library/Fonts/Helvetica.ttc"]),
            "pingfang" | "pingfang sc" => {
                find_existing_path(&["/System/Library/Fonts/PingFang.ttc"])
            }
            _ => None,
        }
    }

    fn scan_fallback_font_path(&self) -> Option<String> {
        find_existing_path(&[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Helvetica.ttc",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        ])
    }
}

fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_default()
}

fn home_child(child: &str) -> String {
    let home = home_dir();
    if home.is_empty() {
        return String::new();
    }
    PathBuf::from(home)
        .join(child)
        .to_string_lossy()
        .to_string()
}

fn find_existing_path(paths: &[&str]) -> Option<String> {
    paths
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .map(|path| (*path).to_string())
}

mod cocoa {
    use super::*;

    pub type Id = *mut c_void;
    type Sel = *mut c_void;
    type Bool = i8;
    type CGFloat = f64;

    const YES: Bool = 1;
    const NO: Bool = 0;
    const NS_APPLICATION_ACTIVATION_POLICY_REGULAR: isize = 0;
    const NS_BACKING_STORE_BUFFERED: isize = 2;
    const NS_WINDOW_STYLE_TITLED: usize = 1 << 0;
    const NS_WINDOW_STYLE_CLOSABLE: usize = 1 << 1;
    const NS_WINDOW_STYLE_MINIATURIZABLE: usize = 1 << 2;
    const NS_WINDOW_STYLE_RESIZABLE: usize = 1 << 3;
    const NSEVENT_MASK_ANY: usize = usize::MAX;
    const KCGIMAGE_ALPHA_PREMULTIPLIED_FIRST: u32 = 2;
    const KCGIMAGE_BYTE_ORDER_32_LITTLE: u32 = 2 << 12;

    static INIT_APP: Once = Once::new();
    static mut RUN_LOOP_MODE: Id = std::ptr::null_mut();

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: CGFloat,
        y: CGFloat,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGSize {
        width: CGFloat,
        height: CGFloat,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
    }

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {}

    #[link(name = "QuartzCore", kind = "framework")]
    unsafe extern "C" {}

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFDataCreate(allocator: Id, bytes: *const u8, length: isize) -> Id;
        fn CFRelease(cf: Id);
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGColorSpaceCreateDeviceRGB() -> Id;
        fn CGDataProviderCreateWithCFData(data: Id) -> Id;
        fn CGImageCreate(
            width: usize,
            height: usize,
            bits_per_component: usize,
            bits_per_pixel: usize,
            bytes_per_row: usize,
            color_space: Id,
            bitmap_info: u32,
            provider: Id,
            decode: *const CGFloat,
            should_interpolate: Bool,
            intent: i32,
        ) -> Id;
    }

    pub unsafe fn create_window(title: &str, width: i32, height: i32) -> (Id, Id) {
        initialize_app();
        let width = width.max(1);
        let height = height.max(1);
        let rect = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: width as CGFloat,
                height: height as CGFloat,
            },
        };
        let style = NS_WINDOW_STYLE_TITLED
            | NS_WINDOW_STYLE_CLOSABLE
            | NS_WINDOW_STYLE_MINIATURIZABLE
            | NS_WINDOW_STYLE_RESIZABLE;

        let window = msg_id(class("NSWindow"), "alloc");
        let window = msg_id_rect_usize_isize_bool(
            window,
            "initWithContentRect:styleMask:backing:defer:",
            rect,
            style,
            NS_BACKING_STORE_BUFFERED,
            NO,
        );
        set_window_title(window, title);
        let content_view = msg_id(window, "contentView");
        msg_void_bool(content_view, "setWantsLayer:", YES);
        let layer = msg_id(content_view, "layer");
        msg_void_bool(layer, "setNeedsDisplayOnBoundsChange:", YES);
        (window, layer)
    }

    pub unsafe fn show_window(window: Id) {
        msg_void_id(window, "makeKeyAndOrderFront:", std::ptr::null_mut());
        msg_void_bool(shared_application(), "activateIgnoringOtherApps:", YES);
    }

    pub unsafe fn hide_window(window: Id) {
        msg_void_id(window, "orderOut:", std::ptr::null_mut());
    }

    pub unsafe fn close_window(window: Id) {
        msg_void(window, "close");
    }

    pub unsafe fn set_window_title(window: Id, title: &str) {
        let ns_title = ns_string(title);
        msg_void_id(window, "setTitle:", ns_title);
    }

    pub unsafe fn set_window_size(window: Id, width: i32, height: i32) {
        let frame = CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: width.max(1) as CGFloat,
                height: height.max(1) as CGFloat,
            },
        };
        msg_void_rect_bool(window, "setFrame:display:", frame, YES);
    }

    pub unsafe fn set_layer_pixels(layer: Id, pixels: &[u32], width: i32, height: i32) {
        if layer.is_null() || width <= 0 || height <= 0 || pixels.is_empty() {
            return;
        }
        let len = (width as usize)
            .saturating_mul(height as usize)
            .min(pixels.len());
        let byte_len = len.saturating_mul(std::mem::size_of::<u32>());
        let data = CFDataCreate(
            std::ptr::null_mut(),
            pixels.as_ptr() as *const u8,
            byte_len as isize,
        );
        if data.is_null() {
            return;
        }
        let provider = CGDataProviderCreateWithCFData(data);
        if provider.is_null() {
            CFRelease(data);
            return;
        }
        let color_space = CGColorSpaceCreateDeviceRGB();
        if color_space.is_null() {
            CFRelease(provider);
            CFRelease(data);
            return;
        }
        let image = CGImageCreate(
            width as usize,
            height as usize,
            8,
            32,
            width as usize * std::mem::size_of::<u32>(),
            color_space,
            KCGIMAGE_ALPHA_PREMULTIPLIED_FIRST | KCGIMAGE_BYTE_ORDER_32_LITTLE,
            provider,
            std::ptr::null(),
            NO,
            0,
        );
        if !image.is_null() {
            msg_void_id(layer, "setContents:", image);
            msg_void(layer, "setNeedsDisplay");
            CFRelease(image);
        }
        CFRelease(color_space);
        CFRelease(provider);
        CFRelease(data);
    }

    pub unsafe fn dispatch_one_event(until: Id) -> bool {
        let event = msg_id_usize_id_id_bool(
            shared_application(),
            "nextEventMatchingMask:untilDate:inMode:dequeue:",
            NSEVENT_MASK_ANY,
            until,
            run_loop_mode(),
            YES,
        );
        if event.is_null() {
            return false;
        }
        msg_void_id(shared_application(), "sendEvent:", event);
        msg_void(shared_application(), "updateWindows");
        true
    }

    pub unsafe fn distant_past() -> Id {
        msg_id(class("NSDate"), "distantPast")
    }

    pub unsafe fn distant_future() -> Id {
        msg_id(class("NSDate"), "distantFuture")
    }

    pub unsafe fn date_with_time_interval(seconds: f64) -> Id {
        msg_id_f64(class("NSDate"), "dateWithTimeIntervalSinceNow:", seconds)
    }

    pub unsafe fn msg_void(receiver: Id, selector: &str) {
        type FnType = unsafe extern "C" fn(Id, Sel);
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector));
    }

    fn initialize_app() {
        INIT_APP.call_once(|| unsafe {
            let app = shared_application();
            msg_void_isize(
                app,
                "setActivationPolicy:",
                NS_APPLICATION_ACTIVATION_POLICY_REGULAR,
            );
            msg_void(app, "finishLaunching");
        });
    }

    unsafe fn shared_application() -> Id {
        msg_id(class("NSApplication"), "sharedApplication")
    }

    unsafe fn run_loop_mode() -> Id {
        if RUN_LOOP_MODE.is_null() {
            RUN_LOOP_MODE = ns_string("kCFRunLoopDefaultMode");
        }
        RUN_LOOP_MODE
    }

    unsafe fn class(name: &str) -> Id {
        let name = CString::new(name).expect("Objective-C class name contains no nul");
        objc_getClass(name.as_ptr())
    }

    unsafe fn sel(name: &str) -> Sel {
        let name = CString::new(name).expect("Objective-C selector contains no nul");
        sel_registerName(name.as_ptr())
    }

    unsafe fn ns_string(value: &str) -> Id {
        let string = msg_id(class("NSString"), "alloc");
        let c_string = CString::new(value).unwrap_or_else(|_| CString::new("").unwrap());
        msg_id_ptr(
            string,
            "initWithUTF8String:",
            c_string.as_ptr() as *const c_void,
        )
    }

    unsafe fn msg_id(receiver: Id, selector: &str) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel) -> Id;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector))
    }

    unsafe fn msg_id_ptr(receiver: Id, selector: &str, ptr: *const c_void) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, *const c_void) -> Id;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), ptr)
    }

    unsafe fn msg_id_f64(receiver: Id, selector: &str, value: f64) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, f64) -> Id;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), value)
    }

    unsafe fn msg_id_rect_usize_isize_bool(
        receiver: Id,
        selector: &str,
        rect: CGRect,
        style: usize,
        backing: isize,
        defer: Bool,
    ) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, CGRect, usize, isize, Bool) -> Id;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), rect, style, backing, defer)
    }

    unsafe fn msg_id_usize_id_id_bool(
        receiver: Id,
        selector: &str,
        mask: usize,
        until: Id,
        mode: Id,
        dequeue: Bool,
    ) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, usize, Id, Id, Bool) -> Id;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), mask, until, mode, dequeue)
    }

    unsafe fn msg_void_id(receiver: Id, selector: &str, arg: Id) {
        type FnType = unsafe extern "C" fn(Id, Sel, Id);
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), arg);
    }

    unsafe fn msg_void_bool(receiver: Id, selector: &str, value: Bool) {
        type FnType = unsafe extern "C" fn(Id, Sel, Bool);
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), value);
    }

    unsafe fn msg_void_isize(receiver: Id, selector: &str, value: isize) {
        type FnType = unsafe extern "C" fn(Id, Sel, isize);
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), value);
    }

    unsafe fn msg_void_rect_bool(receiver: Id, selector: &str, rect: CGRect, value: Bool) {
        type FnType = unsafe extern "C" fn(Id, Sel, CGRect, Bool);
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), rect, value);
    }
}
