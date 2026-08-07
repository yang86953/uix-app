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
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        #[cfg(target_arch = "x86_64")]
        fn objc_msgSend_stret();
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
        fn CFRunLoopGetMain() -> Id;
        fn CFRunLoopWakeUp(run_loop: Id);
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

    pub struct CreatedWindow {
        pub layer: Id,
    }

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
        // Vulkan/MoltenVK and Metal identity both consume a CAMetalLayer as the
        // native surface (VK_EXT_metal_surface / CPU setContents).
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
        // PixelUpload stages via TRANSFER_DST; MoltenVK needs non-framebufferOnly.
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

    pub unsafe fn show_window(window: Id) {
        msg_void_id(window, "makeKeyAndOrderFront:", std::ptr::null_mut());
        msg_void_bool(shared_application(), "activateIgnoringOtherApps:", YES);
    }

    pub unsafe fn hide_window(window: Id) {
        msg_void_id(window, "orderOut:", std::ptr::null_mut());
    }

    pub unsafe fn window_occlusion_state(window: Id) -> WindowOcclusionState {
        if msg_usize(window, "occlusionState") & NS_WINDOW_OCCLUSION_STATE_VISIBLE != 0 {
            WindowOcclusionState::Visible
        } else {
            WindowOcclusionState::Occluded
        }
    }

    pub unsafe fn close_window(window: Id) {
        msg_void(window, "close");
    }

    pub unsafe fn release_object(object: Id) {
        if !object.is_null() {
            msg_void(object, "release");
        }
    }

    pub unsafe fn close_and_release_window(window: Id) {
        if window.is_null() {
            return;
        }
        close_window(window);
        release_object(window);
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

    pub unsafe fn clipboard_text() -> String {
        let pasteboard = general_pasteboard();
        if pasteboard.is_null() {
            return String::new();
        }
        let string = msg_id_id(pasteboard, "stringForType:", pasteboard_string_type());
        ns_string_to_string(string).unwrap_or_default()
    }

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

    pub unsafe fn clipboard_has_text() -> bool {
        let pasteboard = general_pasteboard();
        if pasteboard.is_null() {
            return false;
        }
        !msg_id_id(pasteboard, "stringForType:", pasteboard_string_type()).is_null()
    }

    pub unsafe fn main_screen_scale() -> f64 {
        screen_scale(main_screen())
    }

    pub unsafe fn screen_count() -> usize {
        let screens = msg_id(class("NSScreen"), "screens");
        if screens.is_null() {
            return usize::from(!main_screen().is_null());
        }
        msg_usize(screens, "count")
    }

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
            .ok_or_else(|| {
                format!("CAMetalLayer pixel extent overflows usize: {width}x{height}")
            })?;
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

    pub unsafe fn distant_past() -> Id {
        msg_id(class("NSDate"), "distantPast")
    }

    pub unsafe fn wake_main_run_loop() {
        let run_loop = CFRunLoopGetMain();
        if !run_loop.is_null() {
            CFRunLoopWakeUp(run_loop);
        }
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

    unsafe fn main_screen() -> Id {
        msg_id(class("NSScreen"), "mainScreen")
    }

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

    unsafe fn screen_scale(screen: Id) -> f64 {
        if screen.is_null() {
            return 1.0;
        }
        msg_f64(screen, "backingScaleFactor").max(1.0)
    }

    unsafe fn general_pasteboard() -> Id {
        msg_id(class("NSPasteboard"), "generalPasteboard")
    }

    unsafe fn pasteboard_string_type() -> Id {
        ns_string("public.utf8-plain-text")
    }

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

    unsafe fn event_key_code(event: Id, kind: isize) -> u16 {
        match kind {
            NSEVENT_TYPE_KEY_DOWN | NSEVENT_TYPE_KEY_UP => msg_u16(event, "keyCode"),
            _ => 0,
        }
    }

    unsafe fn event_text(event: Id, kind: isize) -> String {
        match kind {
            NSEVENT_TYPE_KEY_DOWN => {
                ns_string_to_string(msg_id(event, "characters")).unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    unsafe fn event_f64(event: Id, kind: isize, selector: &str) -> f64 {
        match kind {
            NSEVENT_TYPE_SCROLL_WHEEL => msg_f64(event, selector),
            _ => 0.0,
        }
    }

    unsafe fn run_loop_mode() -> Id {
        if RUN_LOOP_MODE.is_null() {
            RUN_LOOP_MODE = ns_string("kCFRunLoopDefaultMode");
        }
        RUN_LOOP_MODE
    }

    unsafe fn class(name: &str) -> Id {
        CString::new(name)
            .ok()
            .map(|name| objc_getClass(name.as_ptr()))
            .unwrap_or(std::ptr::null_mut())
    }

    unsafe fn sel(name: &str) -> Sel {
        CString::new(name)
            .ok()
            .map(|name| sel_registerName(name.as_ptr()))
            .unwrap_or(std::ptr::null_mut())
    }

    unsafe fn ns_string(value: &str) -> Id {
        let string = msg_id(class("NSString"), "alloc");
        let c_string = CString::new(value).unwrap_or_default();
        msg_id_ptr(
            string,
            "initWithUTF8String:",
            c_string.as_ptr() as *const c_void,
        )
    }

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

    unsafe fn msg_id_id(receiver: Id, selector: &str, arg: Id) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, Id) -> Id;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), arg)
    }

    unsafe fn msg_id_usize(receiver: Id, selector: &str, arg: usize) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, usize) -> Id;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), arg)
    }

    unsafe fn msg_const_char_ptr(receiver: Id, selector: &str) -> *const c_char {
        type FnType = unsafe extern "C" fn(Id, Sel) -> *const c_char;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector))
    }

    unsafe fn msg_id_f64(receiver: Id, selector: &str, value: f64) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, f64) -> Id;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), value)
    }

    unsafe fn msg_f64(receiver: Id, selector: &str) -> f64 {
        type FnType = unsafe extern "C" fn(Id, Sel) -> f64;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector))
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

    unsafe fn msg_void_cgsize(receiver: Id, selector: &str, size: CGSize) {
        type FnType = unsafe extern "C" fn(Id, Sel, CGSize);
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), size);
    }

    unsafe fn msg_void_isize(receiver: Id, selector: &str, value: isize) {
        type FnType = unsafe extern "C" fn(Id, Sel, isize);
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), value);
    }

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

    unsafe fn msg_void_rect_bool(receiver: Id, selector: &str, rect: CGRect, value: Bool) {
        type FnType = unsafe extern "C" fn(Id, Sel, CGRect, Bool);
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), rect, value);
    }

    unsafe fn msg_isize(receiver: Id, selector: &str) -> isize {
        type FnType = unsafe extern "C" fn(Id, Sel) -> isize;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector))
    }

    unsafe fn msg_usize(receiver: Id, selector: &str) -> usize {
        type FnType = unsafe extern "C" fn(Id, Sel) -> usize;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector))
    }

    unsafe fn msg_u16(receiver: Id, selector: &str) -> u16 {
        type FnType = unsafe extern "C" fn(Id, Sel) -> u16;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector))
    }

    unsafe fn msg_point(receiver: Id, selector: &str) -> CGPoint {
        type FnType = unsafe extern "C" fn(Id, Sel) -> CGPoint;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector))
    }

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
            type FnType = unsafe extern "C" fn(Id, Sel) -> CGRect;
            let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            f(receiver, sel(selector))
        }
    }

    unsafe fn msg_bool_id_id(receiver: Id, selector: &str, first: Id, second: Id) -> Bool {
        type FnType = unsafe extern "C" fn(Id, Sel, Id, Id) -> Bool;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), first, second)
    }

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
