//! cocoa FFI 辅助。

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
const NS_WINDOW_OCCLUSION_STATE_VISIBLE: usize = 1 << 1;
const NSEVENT_MASK_ANY: usize = usize::MAX;
const KCGIMAGE_ALPHA_PREMULTIPLIED_FIRST: u32 = 2;
const KCGIMAGE_BYTE_ORDER_32_LITTLE: u32 = 2 << 12;
pub const NSEVENT_TYPE_LEFT_MOUSE_DOWN: isize = 1;
pub const NSEVENT_TYPE_LEFT_MOUSE_UP: isize = 2;
pub const NSEVENT_TYPE_RIGHT_MOUSE_DOWN: isize = 3;
pub const NSEVENT_TYPE_RIGHT_MOUSE_UP: isize = 4;
pub const NSEVENT_TYPE_MOUSE_MOVED: isize = 5;
pub const NSEVENT_TYPE_LEFT_MOUSE_DRAGGED: isize = 6;
pub const NSEVENT_TYPE_RIGHT_MOUSE_DRAGGED: isize = 7;
pub const NSEVENT_TYPE_KEY_DOWN: isize = 10;
pub const NSEVENT_TYPE_KEY_UP: isize = 11;
pub const NSEVENT_TYPE_SCROLL_WHEEL: isize = 22;
pub const NSEVENT_TYPE_OTHER_MOUSE_DOWN: isize = 25;
pub const NSEVENT_TYPE_OTHER_MOUSE_UP: isize = 26;
pub const NSEVENT_TYPE_OTHER_MOUSE_DRAGGED: isize = 27;
pub const NSEVENT_MODIFIER_FLAG_SHIFT: usize = 1 << 17;
pub const NSEVENT_MODIFIER_FLAG_CONTROL: usize = 1 << 18;
pub const NSEVENT_MODIFIER_FLAG_OPTION: usize = 1 << 19;
pub const NSEVENT_MODIFIER_FLAG_COMMAND: usize = 1 << 20;

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

pub struct ScreenInfo {
    pub bounds: Rect,
    pub scale: f64,
}

#[link(name = "objc")]
// SAFETY: 声明严格对应 Objective-C 运行时 C ABI，消息发送器只会在按选择器签名转型后调用。
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    #[cfg(target_arch = "x86_64")]
    fn objc_msgSend_stret();
}

#[link(name = "AppKit", kind = "framework")]
// SAFETY: 空链接块仅要求加载 AppKit 框架，不声明或调用任何符号。
unsafe extern "C" {}

#[link(name = "Foundation", kind = "framework")]
// SAFETY: 空链接块仅要求加载 Foundation 框架，不声明或调用任何符号。
unsafe extern "C" {}

#[link(name = "QuartzCore", kind = "framework")]
// SAFETY: 空链接块仅要求加载 QuartzCore 框架，不声明或调用任何符号。
unsafe extern "C" {}

#[link(name = "CoreFoundation", kind = "framework")]
// SAFETY: 声明对应 CoreFoundation C ABI，调用方维护对象所有权、字节范围和运行循环句柄有效性。
unsafe extern "C" {
    fn CFDataCreate(allocator: Id, bytes: *const u8, length: isize) -> Id;
    fn CFRelease(cf: Id);
    fn CFRunLoopGetMain() -> Id;
    fn CFRunLoopWakeUp(run_loop: Id);
}

#[link(name = "CoreGraphics", kind = "framework")]
// SAFETY: 声明对应 CoreGraphics C ABI，调用方负责图像尺寸、行跨度及 CF 对象的配对释放。
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

pub struct CreatedWindow {
    pub layer: Id,
}

// SAFETY: 必须在 AppKit UI 线程调用；返回的 NSWindow 为 +1 所有权，调用方负责关闭并释放。
pub unsafe fn create_window(
    title: &str,
    width: i32,
    height: i32,
    events: Arc<Mutex<VecDeque<UiEvent>>>,
    window_id: WindowId,
    text_input_owner: text_input_view::SharedImeOwner,
    pending_failures: PendingFailureSource,
) -> crate::core::Result<(Id, CreatedWindow)> {
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
    if window.is_null() {
        return Err(Error::new(
            Errc::PlatformError,
            "macOS create_window: NSWindow alloc returned null",
        ));
    }
    let window = msg_id_rect_usize_isize_bool(
        window,
        "initWithContentRect:styleMask:backing:defer:",
        rect,
        style,
        NS_BACKING_STORE_BUFFERED,
        NO,
    );
    if window.is_null() {
        return Err(Error::new(
            Errc::PlatformError,
            "macOS create_window: NSWindow initialization failed",
        ));
    }
    // Rust owns the alloc/init +1 reference until WindowOps releases it
    // after graphics shutdown. A user close only orders the window out.
    msg_void_bool(window, "setReleasedWhenClosed:", NO);
    set_window_title(window, title);
    let content_view = match super::text_input_view::create_content_view(
        rect.size.width,
        rect.size.height,
        events,
        window_id,
        text_input_owner,
        pending_failures,
    ) {
        Ok(view) => view,
        Err(error) => {
            msg_void(window, "release");
            return Err(error);
        }
    };
    msg_void_id(window, "setContentView:", content_view);
    // Vulkan/MoltenVK 与原生 Metal 都消费 CAMetalLayer 作为唯一窗口 surface。
    msg_void_bool(content_view, "setWantsLayer:", YES);
    let layer = msg_id(class("CAMetalLayer"), "layer");
    if layer.is_null() {
        // NSWindow retains contentView; balance our alloc/init ownership,
        // then release the window to tear the retained view back down.
        msg_void(content_view, "release");
        msg_void(window, "release");
        return Err(Error::new(
            Errc::PlatformError,
            "macOS create_window: CAMetalLayer factory returned null",
        ));
    }
    msg_void_bool(layer, "setNeedsDisplayOnBoundsChange:", YES);
    // MoltenVK 传输与 Metal 离屏合成都要求 layer 纹理不局限于 framebuffer。
    msg_void_bool(layer, "setFramebufferOnly:", NO);
    msg_void_cgsize(
        layer,
        "setDrawableSize:",
        CGSize {
            width: width as CGFloat,
            height: height as CGFloat,
        },
    );
    msg_void_id(content_view, "setLayer:", layer);
    // `NSWindow.contentView` is strong; balance create_content_view's +1 so
    // UixContentView dealloc follows the owning window exactly.
    msg_void(content_view, "release");
    Ok((window, CreatedWindow { layer }))
}

// SAFETY: window 必须是仍存活且由当前 UI 线程拥有的 NSWindow。
pub unsafe fn show_window(window: Id) {
    msg_void_id(window, "makeKeyAndOrderFront:", std::ptr::null_mut());
    msg_void_bool(shared_application(), "activateIgnoringOtherApps:", YES);
}

// SAFETY: window 必须是仍存活且由当前 UI 线程拥有的 NSWindow。
pub unsafe fn hide_window(window: Id) {
    msg_void_id(window, "orderOut:", std::ptr::null_mut());
}

// SAFETY: window 必须是调用期间仍存活的 NSWindow，查询必须发生在 AppKit UI 线程。
pub unsafe fn window_occlusion_state(window: Id) -> WindowOcclusionState {
    if msg_usize(window, "occlusionState") & NS_WINDOW_OCCLUSION_STATE_VISIBLE != 0 {
        WindowOcclusionState::Visible
    } else {
        WindowOcclusionState::Occluded
    }
}

// SAFETY: window 必须是仍存活且尚未关闭的 NSWindow，本函数不释放调用方的 +1 所有权。
pub unsafe fn close_window(window: Id) {
    msg_void(window, "close");
}

// 通过 AppKit 的标准用户关闭入口请求窗口关闭，让委托回调发布最终关闭事实。
// SAFETY: window 必须是仍存活且尚未关闭的 NSWindow，本函数不释放调用方的 +1 所有权。
pub unsafe fn request_window_close(window: Id) {
    // performClose: 会执行与标题栏关闭按钮相同的 AppKit 关闭语义，并同步触发窗口委托链。
    msg_void_id(window, "performClose:", std::ptr::null_mut());
}

// SAFETY: 非空 object 必须持有可由当前调用精确消费一次的 Objective-C +1 引用。
pub unsafe fn release_object(object: Id) {
    if !object.is_null() {
        msg_void(object, "release");
    }
}

// SAFETY: 非空 window 必须是仍存活的 NSWindow，并持有由本函数关闭后精确释放的唯一 +1 引用。
pub unsafe fn close_and_release_window(window: Id) {
    if window.is_null() {
        return;
    }
    close_window(window);
    release_object(window);
}

// SAFETY: window 必须是存活 NSWindow；临时 NSString 在同步 setter 返回前保持有效。
pub unsafe fn set_window_title(window: Id, title: &str) {
    let ns_title = ns_string(title);
    msg_void_id(window, "setTitle:", ns_title);
}

// SAFETY: window 必须是存活 NSWindow，CGRect 布局与当前架构的 AppKit ABI 一致。
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

// SAFETY: 必须在可访问 AppKit 的线程调用，所有返回对象仅在同步消息链中读取。
pub unsafe fn clipboard_text() -> String {
    let pasteboard = general_pasteboard();
    if pasteboard.is_null() {
        return String::new();
    }
    let string = msg_id_id(pasteboard, "stringForType:", pasteboard_string_type());
    ns_string_to_string(string).unwrap_or_default()
}

// SAFETY: 必须在可访问 AppKit 的线程调用，临时 NSString 在同步剪贴板写入返回前有效。
pub unsafe fn set_clipboard_text(text: &str) {
    let pasteboard = general_pasteboard();
    if pasteboard.is_null() {
        return;
    }
    let _ = msg_isize(pasteboard, "clearContents");
    let string = ns_string(text);
    if string.is_null() {
        return;
    }
    let _ = msg_bool_id_id(
        pasteboard,
        "setString:forType:",
        string,
        pasteboard_string_type(),
    );
}

// SAFETY: 必须在可访问 AppKit 的线程调用，粘贴板与类型对象由 AppKit 在查询期间保持存活。
pub unsafe fn clipboard_has_text() -> bool {
    let pasteboard = general_pasteboard();
    if pasteboard.is_null() {
        return false;
    }
    !msg_id_id(pasteboard, "stringForType:", pasteboard_string_type()).is_null()
}

// SAFETY: 必须在 AppKit UI 线程调用，NSScreen 单例在同步查询期间有效。
pub unsafe fn main_screen_scale() -> f64 {
    screen_scale(main_screen())
}

// SAFETY: 必须在 AppKit UI 线程调用，NSScreen 数组在同步消息链期间由 AppKit 持有。
pub unsafe fn screen_count() -> usize {
    let screens = msg_id(class("NSScreen"), "screens");
    if screens.is_null() {
        return usize::from(!main_screen().is_null());
    }
    msg_usize(screens, "count")
}

// SAFETY: 必须在 AppKit UI 线程调用，选中的 NSScreen 在全部属性查询完成前保持有效。
pub unsafe fn screen_info(index: usize) -> ScreenInfo {
    let screen = screen_at(index);
    let frame = if screen.is_null() {
        CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: 1440.0,
                height: 900.0,
            },
        }
    } else {
        msg_rect(screen, "frame")
    };
    ScreenInfo {
        bounds: Rect::new(
            frame.origin.x as f32,
            frame.origin.y as f32,
            frame.size.width as f32,
            frame.size.height as f32,
        ),
        scale: screen_scale(screen),
    }
}

// SAFETY: 必须在可访问 Foundation 的线程调用，NSUserDefaults 返回对象仅在同步查询期间使用。
pub unsafe fn is_dark_mode() -> bool {
    let defaults = msg_id(class("NSUserDefaults"), "standardUserDefaults");
    if defaults.is_null() {
        return false;
    }
    let style = msg_id_id(defaults, "stringForKey:", ns_string("AppleInterfaceStyle"));
    ns_string_to_string(style)
        .map(|value| value.eq_ignore_ascii_case("dark"))
        .unwrap_or(false)
}

// SAFETY: layer 必须是存活 CAMetalLayer；调用方须在允许更新该层的线程调用，像素切片由本函数校验并同步复制。
pub unsafe fn set_layer_pixels(
    layer: Id,
    pixels: &[u32],
    width: i32,
    height: i32,
) -> std::result::Result<(), String> {
    if layer.is_null() || width <= 0 || height <= 0 || pixels.is_empty() {
        return Err("invalid CAMetalLayer pixel payload".to_owned());
    }
    let len = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| format!("CAMetalLayer pixel extent overflows usize: {width}x{height}"))?;
    if pixels.len() < len {
        return Err(format!(
            "CAMetalLayer pixel payload too short: got {}, need {len} for {width}x{height}",
            pixels.len()
        ));
    }
    let byte_len = len
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| format!("CAMetalLayer byte length overflows for {width}x{height}"))?;
    let byte_len = isize::try_from(byte_len)
        .map_err(|_| format!("CAMetalLayer byte length exceeds CFData limit: {byte_len}"))?;
    let data = CFDataCreate(std::ptr::null_mut(), pixels.as_ptr() as *const u8, byte_len);
    if data.is_null() {
        return Err("CFDataCreate for CAMetalLayer pixels failed".to_owned());
    }
    let provider = CGDataProviderCreateWithCFData(data);
    if provider.is_null() {
        CFRelease(data);
        return Err("CGDataProviderCreateWithCFData for CAMetalLayer pixels failed".to_owned());
    }
    let color_space = CGColorSpaceCreateDeviceRGB();
    if color_space.is_null() {
        CFRelease(provider);
        CFRelease(data);
        return Err("CGColorSpaceCreateDeviceRGB for CAMetalLayer pixels failed".to_owned());
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
    if image.is_null() {
        CFRelease(color_space);
        CFRelease(provider);
        CFRelease(data);
        return Err("CGImageCreate for CAMetalLayer pixels failed".to_owned());
    }
    msg_void_id(layer, "setContents:", image);
    msg_void(layer, "setNeedsDisplay");
    CFRelease(image);
    CFRelease(color_space);
    CFRelease(provider);
    CFRelease(data);
    Ok(())
}

// SAFETY: until 必须是存活 NSDate，且整个取出、转换和发送过程必须在 AppKit UI 线程执行。
pub unsafe fn dispatch_one_event(until: Id) -> Option<MacosAppEvent> {
    let event = msg_id_usize_id_id_bool(
        shared_application(),
        "nextEventMatchingMask:untilDate:inMode:dequeue:",
        NSEVENT_MASK_ANY,
        until,
        run_loop_mode(),
        YES,
    );
    if event.is_null() {
        return None;
    }
    let app_event = macos_app_event_from_ns_event(event);
    msg_void_id(shared_application(), "sendEvent:", event);
    msg_void(shared_application(), "updateWindows");
    Some(app_event)
}

// SAFETY: 必须在可访问 Foundation 的线程调用，返回的 NSDate 由框架管理生命周期。
pub unsafe fn distant_past() -> Id {
    msg_id(class("NSDate"), "distantPast")
}

// SAFETY: CFRunLoopGetMain 返回进程主循环，CFRunLoopWakeUp 允许从任意线程同步唤醒该句柄。
pub unsafe fn wake_main_run_loop() {
    let run_loop = CFRunLoopGetMain();
    if !run_loop.is_null() {
        CFRunLoopWakeUp(run_loop);
    }
}

// SAFETY: 必须在可访问 Foundation 的线程调用，返回的 NSDate 由框架管理生命周期。
pub unsafe fn distant_future() -> Id {
    msg_id(class("NSDate"), "distantFuture")
}

// SAFETY: 必须在可访问 Foundation 的线程调用，返回的 NSDate 由框架管理生命周期。
pub unsafe fn date_with_time_interval(seconds: f64) -> Id {
    msg_id_f64(class("NSDate"), "dateWithTimeIntervalSinceNow:", seconds)
}

// SAFETY: receiver 必须响应签名为 void(id, SEL) 的 selector；转型严格匹配该 Objective-C 方法 ABI。
pub unsafe fn msg_void(receiver: Id, selector: &str) {
    type FnType = unsafe extern "C" fn(Id, Sel);
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector));
}

fn initialize_app() {
    // SAFETY: Once 保证初始化只执行一次，调用路径限定在 AppKit UI 线程且单例对象由框架持有。
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

// SAFETY: 必须在 AppKit UI 线程调用，返回的 NSApplication 单例由框架管理生命周期。
unsafe fn shared_application() -> Id {
    msg_id(class("NSApplication"), "sharedApplication")
}

// SAFETY: 必须在 AppKit UI 线程调用，返回的 NSScreen 由框架在同步使用期间保持存活。
unsafe fn main_screen() -> Id {
    msg_id(class("NSScreen"), "mainScreen")
}

// SAFETY: 必须在 AppKit UI 线程调用，NSScreen 数组和元素均由框架管理生命周期。
unsafe fn screen_at(index: usize) -> Id {
    let screens = msg_id(class("NSScreen"), "screens");
    if screens.is_null() {
        return main_screen();
    }
    let count = msg_usize(screens, "count");
    if count == 0 {
        return std::ptr::null_mut();
    }
    msg_id_usize(screens, "objectAtIndex:", index.min(count - 1))
}

// SAFETY: 非空 screen 必须是存活 NSScreen，并在同步属性查询期间保持有效。
unsafe fn screen_scale(screen: Id) -> f64 {
    if screen.is_null() {
        return 1.0;
    }
    msg_f64(screen, "backingScaleFactor").max(1.0)
}

// SAFETY: 必须在可访问 AppKit 的线程调用，返回的全局粘贴板由框架管理生命周期。
unsafe fn general_pasteboard() -> Id {
    msg_id(class("NSPasteboard"), "generalPasteboard")
}

// SAFETY: 必须在可访问 Foundation 的线程调用，返回 NSString 只用于同步粘贴板消息。
unsafe fn pasteboard_string_type() -> Id {
    ns_string("public.utf8-plain-text")
}

// SAFETY: event 必须是当前分派周期内存活的 NSEvent，其关联窗口和属性在转换完成前有效。
unsafe fn macos_app_event_from_ns_event(event: Id) -> MacosAppEvent {
    let kind = msg_isize(event, "type");
    let event_window = msg_id(event, "window");
    MacosAppEvent {
        window_id: window_delegate::window_id(event_window),
        kind,
        location: event_location(event),
        button_number: event_button_number(event, kind),
        delta_x: event_f64(event, kind, "scrollingDeltaX"),
        delta_y: event_f64(event, kind, "scrollingDeltaY"),
        key_code: event_key_code(event, kind),
        modifiers: msg_usize(event, "modifierFlags"),
        text: event_text(event, kind),
    }
}

// SAFETY: event 必须是存活 NSEvent，关联 NSWindow/NSView 只在本次同步坐标查询期间使用。
unsafe fn event_location(event: Id) -> crate::core::Point {
    let location = msg_point(event, "locationInWindow");
    let mut x = location.x as f32;
    let mut y = location.y as f32;
    let window = msg_id(event, "window");
    if !window.is_null() {
        let content_view = msg_id(window, "contentView");
        if !content_view.is_null() {
            let frame = msg_rect(content_view, "frame");
            x = x.clamp(0.0, frame.size.width as f32);
            y = (frame.size.height as f32 - y).clamp(0.0, frame.size.height as f32);
        }
    }
    crate::core::Point::new(x, y)
}

// SAFETY: event 必须是存活 NSEvent，只有鼠标事件种类才会发送 buttonNumber 选择器。
unsafe fn event_button_number(event: Id, kind: isize) -> isize {
    match kind {
        NSEVENT_TYPE_LEFT_MOUSE_DOWN
        | NSEVENT_TYPE_LEFT_MOUSE_UP
        | NSEVENT_TYPE_RIGHT_MOUSE_DOWN
        | NSEVENT_TYPE_RIGHT_MOUSE_UP
        | NSEVENT_TYPE_OTHER_MOUSE_DOWN
        | NSEVENT_TYPE_OTHER_MOUSE_UP
        | NSEVENT_TYPE_OTHER_MOUSE_DRAGGED => msg_isize(event, "buttonNumber"),
        _ => 0,
    }
}

// SAFETY: event 必须是存活 NSEvent，只有键盘事件种类才会发送 keyCode 选择器。
unsafe fn event_key_code(event: Id, kind: isize) -> u16 {
    match kind {
        NSEVENT_TYPE_KEY_DOWN | NSEVENT_TYPE_KEY_UP => msg_u16(event, "keyCode"),
        _ => 0,
    }
}

// SAFETY: event 必须是存活 NSEvent，只有按键事件才会读取其 characters NSString。
unsafe fn event_text(event: Id, kind: isize) -> String {
    match kind {
        NSEVENT_TYPE_KEY_DOWN => {
            ns_string_to_string(msg_id(event, "characters")).unwrap_or_default()
        }
        _ => String::new(),
    }
}

// SAFETY: event 必须是存活 NSEvent，滚轮分支的 selector 必须对应返回 CGFloat 的无参属性。
unsafe fn event_f64(event: Id, kind: isize, selector: &str) -> f64 {
    match kind {
        NSEVENT_TYPE_SCROLL_WHEEL => msg_f64(event, selector),
        _ => 0.0,
    }
}

// SAFETY: 仅在 AppKit UI 线程访问可变静态缓存，写入的 NSString 在进程生命周期内保持可用。
unsafe fn run_loop_mode() -> Id {
    if RUN_LOOP_MODE.is_null() {
        RUN_LOOP_MODE = ns_string("kCFRunLoopDefaultMode");
    }
    RUN_LOOP_MODE
}

// SAFETY: 生成的 C 字符串在 objc_getClass 同步调用期间有效，返回类对象由运行时永久持有。
unsafe fn class(name: &str) -> Id {
    CString::new(name)
        .ok()
        .map(|name| objc_getClass(name.as_ptr()))
        .unwrap_or(std::ptr::null_mut())
}

// SAFETY: 生成的 C 字符串在 sel_registerName 同步调用期间有效，返回选择器由运行时永久持有。
unsafe fn sel(name: &str) -> Sel {
    CString::new(name)
        .ok()
        .map(|name| sel_registerName(name.as_ptr()))
        .unwrap_or(std::ptr::null_mut())
}

// SAFETY: UTF-8 C 字符串在初始化消息返回前有效，NSString 类和选择器签名与消息包装器匹配。
unsafe fn ns_string(value: &str) -> Id {
    let string = msg_id(class("NSString"), "alloc");
    let c_string = CString::new(value).unwrap_or_default();
    msg_id_ptr(
        string,
        "initWithUTF8String:",
        c_string.as_ptr() as *const c_void,
    )
}

// SAFETY: 非空 value 必须是存活 NSString，UTF8String 指针只在立即复制为 Rust String 时读取。
unsafe fn ns_string_to_string(value: Id) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let ptr = msg_const_char_ptr(value, "UTF8String");
    if ptr.is_null() {
        return None;
    }
    Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
}

// SAFETY: receiver 必须响应返回对象且无额外参数的 selector，函数指针转型严格匹配 Objective-C ABI。
unsafe fn msg_id(receiver: Id, selector: &str) -> Id {
    type FnType = unsafe extern "C" fn(Id, Sel) -> Id;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector))
}

// SAFETY: receiver 必须响应接收一个原始指针并返回对象的 selector，ptr 须在同步调用期间有效。
unsafe fn msg_id_ptr(receiver: Id, selector: &str, ptr: *const c_void) -> Id {
    type FnType = unsafe extern "C" fn(Id, Sel, *const c_void) -> Id;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), ptr)
}

// SAFETY: receiver 必须响应接收一个对象并返回对象的 selector，两个对象均须在同步调用期间有效。
unsafe fn msg_id_id(receiver: Id, selector: &str, arg: Id) -> Id {
    type FnType = unsafe extern "C" fn(Id, Sel, Id) -> Id;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), arg)
}

// SAFETY: receiver 必须响应接收 NSUInteger 并返回对象的 selector，转型匹配当前架构 Objective-C ABI。
unsafe fn msg_id_usize(receiver: Id, selector: &str, arg: usize) -> Id {
    type FnType = unsafe extern "C" fn(Id, Sel, usize) -> Id;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), arg)
}

// SAFETY: receiver 必须响应返回 const char 指针的无参 selector，调用方负责限制返回指针的读取生命周期。
unsafe fn msg_const_char_ptr(receiver: Id, selector: &str) -> *const c_char {
    type FnType = unsafe extern "C" fn(Id, Sel) -> *const c_char;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector))
}

// SAFETY: receiver 必须响应接收 CGFloat 并返回对象的 selector，转型匹配 64 位 macOS ABI。
unsafe fn msg_id_f64(receiver: Id, selector: &str, value: f64) -> Id {
    type FnType = unsafe extern "C" fn(Id, Sel, f64) -> Id;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), value)
}

// SAFETY: receiver 必须响应返回 CGFloat 的无参 selector，转型匹配 64 位 macOS ABI。
unsafe fn msg_f64(receiver: Id, selector: &str) -> f64 {
    type FnType = unsafe extern "C" fn(Id, Sel) -> f64;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector))
}

// SAFETY: receiver 与 selector 必须匹配 NSWindow 指定初始化器的 CGRect/NSUInteger/NSInteger/BOOL 参数 ABI。
unsafe fn msg_id_rect_usize_isize_bool(
    receiver: Id,
    selector: &str,
    rect: CGRect,
    style: usize,
    backing: isize,
    defer: Bool,
) -> Id {
    // SAFETY: FnType 精确描述该选择器的完整参数与对象返回值，objc_msgSend 仅以此签名调用。
    type FnType = unsafe extern "C" fn(Id, Sel, CGRect, usize, isize, Bool) -> Id;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), rect, style, backing, defer)
}

// SAFETY: receiver 与 selector 必须匹配事件获取方法的 NSUInteger/对象/对象/BOOL 参数 ABI。
unsafe fn msg_id_usize_id_id_bool(
    receiver: Id,
    selector: &str,
    mask: usize,
    until: Id,
    mode: Id,
    dequeue: Bool,
) -> Id {
    // SAFETY: FnType 精确描述该选择器的完整参数与对象返回值，objc_msgSend 仅以此签名调用。
    type FnType = unsafe extern "C" fn(Id, Sel, usize, Id, Id, Bool) -> Id;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), mask, until, mode, dequeue)
}

// SAFETY: receiver 必须响应接收一个对象且无返回值的 selector，arg 在同步调用期间有效。
unsafe fn msg_void_id(receiver: Id, selector: &str, arg: Id) {
    type FnType = unsafe extern "C" fn(Id, Sel, Id);
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), arg);
}

// SAFETY: receiver 必须响应接收 BOOL 且无返回值的 selector，转型匹配 Objective-C ABI。
unsafe fn msg_void_bool(receiver: Id, selector: &str, value: Bool) {
    type FnType = unsafe extern "C" fn(Id, Sel, Bool);
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), value);
}

// SAFETY: receiver 必须响应接收 CGSize 且无返回值的 selector，结构布局匹配 64 位 macOS ABI。
unsafe fn msg_void_cgsize(receiver: Id, selector: &str, size: CGSize) {
    type FnType = unsafe extern "C" fn(Id, Sel, CGSize);
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), size);
}

// SAFETY: receiver 必须响应接收 NSInteger 且无返回值的 selector，转型匹配当前架构 Objective-C ABI。
unsafe fn msg_void_isize(receiver: Id, selector: &str, value: isize) {
    type FnType = unsafe extern "C" fn(Id, Sel, isize);
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), value);
}

// SAFETY: 非空 layer 必须是存活 CAMetalLayer，调用必须发生在允许修改该层的 UI 线程。
pub unsafe fn set_metal_layer_drawable_size(layer: Id, width: i32, height: i32) {
    if layer.is_null() {
        return;
    }
    msg_void_cgsize(
        layer,
        "setDrawableSize:",
        CGSize {
            width: width.max(1) as CGFloat,
            height: height.max(1) as CGFloat,
        },
    );
}

// SAFETY: receiver 必须响应接收 CGRect/BOOL 且无返回值的 selector，结构布局与方法 ABI 一致。
unsafe fn msg_void_rect_bool(receiver: Id, selector: &str, rect: CGRect, value: Bool) {
    type FnType = unsafe extern "C" fn(Id, Sel, CGRect, Bool);
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), rect, value);
}

// SAFETY: receiver 必须响应返回 NSInteger 的无参 selector，转型匹配当前架构 Objective-C ABI。
unsafe fn msg_isize(receiver: Id, selector: &str) -> isize {
    type FnType = unsafe extern "C" fn(Id, Sel) -> isize;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector))
}

// SAFETY: receiver 必须响应返回 NSUInteger 的无参 selector，转型匹配当前架构 Objective-C ABI。
unsafe fn msg_usize(receiver: Id, selector: &str) -> usize {
    type FnType = unsafe extern "C" fn(Id, Sel) -> usize;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector))
}

// SAFETY: receiver 必须响应返回 unsigned short 的无参 selector，转型匹配 Objective-C 方法 ABI。
unsafe fn msg_u16(receiver: Id, selector: &str) -> u16 {
    type FnType = unsafe extern "C" fn(Id, Sel) -> u16;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector))
}

// SAFETY: receiver 必须响应返回 CGPoint 的无参 selector，结构返回 ABI 与当前架构一致。
unsafe fn msg_point(receiver: Id, selector: &str) -> CGPoint {
    type FnType = unsafe extern "C" fn(Id, Sel) -> CGPoint;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector))
}

// SAFETY: receiver 必须响应返回 CGRect 的无参 selector；下方按目标架构选择正确的结构返回 ABI。
unsafe fn msg_rect(receiver: Id, selector: &str) -> CGRect {
    #[cfg(target_arch = "x86_64")]
    {
        type FnType = unsafe extern "C" fn(*mut CGRect, Id, Sel);
        let f: FnType = std::mem::transmute(objc_msgSend_stret as unsafe extern "C" fn());
        let mut result = std::mem::MaybeUninit::<CGRect>::uninit();
        f(result.as_mut_ptr(), receiver, sel(selector));
        result.assume_init()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        // SAFETY: 非 x86_64 macOS 直接返回 CGRect，FnType 精确匹配该架构的 objc_msgSend 结构返回 ABI。
        type FnType = unsafe extern "C" fn(Id, Sel) -> CGRect;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector))
    }
}

// SAFETY: receiver 必须响应接收两个对象并返回 BOOL 的 selector，所有对象在同步调用期间有效。
unsafe fn msg_bool_id_id(receiver: Id, selector: &str, first: Id, second: Id) -> Bool {
    type FnType = unsafe extern "C" fn(Id, Sel, Id, Id) -> Bool;
    let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(receiver, sel(selector), first, second)
}

// SAFETY: layer 必须是存活 CAMetalLayer，像素切片须覆盖给定尺寸且调用线程允许更新该层。
pub(crate) unsafe fn present_layer_pixels(
    layer: cocoa::Id,
    pixels: &[u32],
    width: i32,
    height: i32,
    _damage: PresentDamage,
) -> crate::core::Result<(), Error> {
    cocoa::set_layer_pixels(layer, pixels, width, height).map_err(|message| {
        Error::new(
            Errc::PlatformError,
            format!("MacosPresenter: CAMetalLayer pixel present failed: {message}"),
        )
    })
}
