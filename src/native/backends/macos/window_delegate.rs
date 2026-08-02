use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::{c_char, c_void, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::{Arc, Mutex, Once};

use crate::core::{Errc, Error, WindowId};
use crate::diagnostics::PendingFailureSource;
use crate::native::windowing::event::{UiEvent, UiEventPayload, UiEventType};
use crate::native::windowing::shared::{push_window_close, push_window_resize, WindowState};

use super::objc_runtime;

pub(crate) struct WindowDelegateContext {
    pub events: Arc<Mutex<VecDeque<UiEvent>>>,
    pub window_id: WindowId,
    pub state: Rc<RefCell<WindowState>>,
    pub open: Rc<Cell<bool>>,
    pub pending_failures: PendingFailureSource,
}

/// Installs a delegate that is strongly owned by `window` and owns its Rust
/// context through a raw-pointer ivar.
pub(crate) unsafe fn install(
    window: cocoa::Id,
    context: WindowDelegateContext,
) -> crate::core::Result<()> {
    if window.is_null() {
        return Err(Error::new(
            Errc::InvalidOperation,
            "macOS window delegate: target NSWindow is null",
        ));
    }
    let class = cocoa::delegate_class();
    if class.is_null() {
        return Err(Error::new(
            Errc::PlatformError,
            "macOS window delegate: failed to register UixWindowDelegate",
        ));
    }
    let delegate = cocoa::msg_id(class, "new");
    if delegate.is_null() {
        return Err(Error::new(
            Errc::PlatformError,
            "macOS window delegate: failed to allocate UixWindowDelegate",
        ));
    }
    if let Err(context) = cocoa::install_delegate_context(delegate, Box::new(context)) {
        cocoa::msg_void(delegate, "release");
        drop(context);
        return Err(Error::new(
            Errc::PlatformError,
            "macOS window delegate: failed to install Rust context ivar",
        ));
    }

    // `NSWindow.delegate` is weak. The associated value is a real Objective-C
    // object (never the Rust pointer), so the window safely retains the delegate.
    cocoa::retain_delegate_for_window(window, delegate);
    cocoa::msg_void_id(window, "setDelegate:", delegate);
    // Balance `new`; the window association is now the sole Objective-C owner.
    cocoa::msg_void(delegate, "release");
    Ok(())
}

/// Resolves a live UIX NSWindow to its stable framework window identity.
pub(crate) unsafe fn window_id(window: cocoa::Id) -> Option<WindowId> {
    let delegate = cocoa::owned_delegate(window);
    let context = cocoa::delegate_context(delegate);
    (!context.is_null()).then(|| (*context).window_id)
}

mod cocoa {
    use super::*;

    pub type Id = *mut c_void;
    type Sel = *mut c_void;

    const OBJC_ASSOCIATION_RETAIN_NONATOMIC: usize = 1;

    static DELEGATE_CLASS: Once = Once::new();
    static mut DELEGATE_CLASS_PTR: Id = std::ptr::null_mut();
    static mut DELEGATE_CONTEXT_OFFSET: isize = objc_runtime::INVALID_IVAR_OFFSET;
    static mut WINDOW_DELEGATE_OWNER_KEY: u8 = 0;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        #[cfg(target_arch = "x86_64")]
        fn objc_msgSend_stret();
        fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra_bytes: usize) -> Id;
        fn objc_disposeClassPair(cls: Id);
        fn class_addMethod(cls: Id, name: Sel, imp: *const c_void, types: *const c_char) -> u8;
        fn objc_registerClassPair(cls: Id);
        fn objc_setAssociatedObject(object: Id, key: *const c_void, value: Id, policy: usize);
        fn objc_getAssociatedObject(object: Id, key: *const c_void) -> Id;
    }

    pub unsafe fn delegate_class() -> Id {
        DELEGATE_CLASS.call_once(|| {
            let superclass = class("NSObject");
            let Ok(name) = CString::new("UixWindowDelegate") else {
                return;
            };
            let class_pair = objc_allocateClassPair(superclass, name.as_ptr(), 0);
            if class_pair.is_null() {
                return;
            }
            let Some(context_offset) =
                objc_runtime::add_raw_pointer_ivar(class_pair, "_uixDelegateContext")
            else {
                objc_disposeClassPair(class_pair);
                return;
            };
            let callbacks_ok = add_method(
                class_pair,
                "windowWillClose:",
                window_will_close as *const c_void,
                "v@:@",
            ) && add_method(
                class_pair,
                "windowDidResize:",
                window_did_resize as *const c_void,
                "v@:@",
            ) && add_method(
                class_pair,
                "windowDidBecomeKey:",
                window_did_become_key as *const c_void,
                "v@:@",
            ) && add_method(
                class_pair,
                "windowDidResignKey:",
                window_did_resign_key as *const c_void,
                "v@:@",
            ) && add_method(
                class_pair,
                "windowDidChangeOcclusionState:",
                window_did_change_occlusion_state as *const c_void,
                "v@:@",
            ) && add_method(
                class_pair,
                "dealloc",
                delegate_dealloc as *const c_void,
                "v@:",
            );
            if !callbacks_ok {
                objc_disposeClassPair(class_pair);
                return;
            }
            objc_registerClassPair(class_pair);
            DELEGATE_CONTEXT_OFFSET = context_offset;
            DELEGATE_CLASS_PTR = class_pair;
        });
        DELEGATE_CLASS_PTR
    }

    pub unsafe fn install_delegate_context(
        delegate: Id,
        context: Box<WindowDelegateContext>,
    ) -> Result<(), Box<WindowDelegateContext>> {
        objc_runtime::install_box(delegate, DELEGATE_CONTEXT_OFFSET, context)
    }

    pub unsafe fn delegate_context(delegate: Id) -> *mut WindowDelegateContext {
        objc_runtime::box_ptr(delegate, DELEGATE_CONTEXT_OFFSET)
    }

    pub unsafe fn retain_delegate_for_window(window: Id, delegate: Id) {
        objc_setAssociatedObject(
            window,
            delegate_owner_key(),
            delegate,
            OBJC_ASSOCIATION_RETAIN_NONATOMIC,
        );
    }

    pub unsafe fn owned_delegate(window: Id) -> Id {
        if window.is_null() {
            return std::ptr::null_mut();
        }
        objc_getAssociatedObject(window, delegate_owner_key())
    }

    unsafe fn delegate_owner_key() -> *const c_void {
        std::ptr::addr_of!(WINDOW_DELEGATE_OWNER_KEY).cast::<c_void>()
    }

    unsafe extern "C" fn delegate_dealloc(delegate: Id, _cmd: Sel) {
        // SAFETY: this class installs the Box exactly once before publication;
        // replacing the ivar with null makes dealloc the unique reclaim point.
        let context =
            objc_runtime::take_box::<WindowDelegateContext>(delegate, DELEGATE_CONTEXT_OFFSET);
        let pending_failures = context
            .as_ref()
            .map(|context| context.pending_failures.clone());
        drop(context);
        let result = catch_unwind(AssertUnwindSafe(|| unsafe {
            objc_runtime::call_super_dealloc(delegate, DELEGATE_CLASS_PTR);
        }));
        if result.is_err() {
            if let Some(pending_failures) = pending_failures {
                let _ = pending_failures.enqueue(Error::new(
                    Errc::PlatformError,
                    "macOS window delegate dealloc callback panicked",
                ));
            }
        }
    }

    unsafe extern "C" fn window_will_close(delegate: Id, _cmd: Sel, _notification: Id) {
        with_context(delegate, "windowWillClose", |context| {
            context.open.set(false);
            push_window_close(&context.events, context.window_id);
        });
    }

    unsafe extern "C" fn window_did_resize(delegate: Id, _cmd: Sel, notification: Id) {
        with_context(delegate, "windowDidResize", |context| {
            let window = window_from_notification(notification);
            let (width, height) = content_view_size(window);
            push_window_resize(
                &context.events,
                context.window_id,
                &context.state,
                width,
                height,
            );
        });
    }

    unsafe extern "C" fn window_did_become_key(delegate: Id, _cmd: Sel, _notification: Id) {
        push_focus_event(delegate, UiEventType::WindowFocus);
    }

    unsafe extern "C" fn window_did_resign_key(delegate: Id, _cmd: Sel, _notification: Id) {
        push_focus_event(delegate, UiEventType::WindowBlur);
    }

    unsafe extern "C" fn window_did_change_occlusion_state(
        delegate: Id,
        _cmd: Sel,
        _notification: Id,
    ) {
        push_window_event(delegate, UiEvent::window_occlusion_changed());
    }

    unsafe fn push_focus_event(delegate: Id, event_type: UiEventType) {
        push_window_event(delegate, UiEvent::new(event_type, UiEventPayload::None));
    }

    unsafe fn push_window_event(delegate: Id, event: UiEvent) {
        with_context(delegate, "window event", |context| {
            let event = event.for_window(context.window_id);
            let _ = context
                .events
                .lock()
                .map(|mut events| events.push_back(event));
        });
    }

    unsafe fn with_context<F>(delegate: Id, callback_name: &str, callback: F)
    where
        F: FnOnce(&WindowDelegateContext),
    {
        let context = delegate_context(delegate);
        if context.is_null() {
            return;
        }
        let result = catch_unwind(AssertUnwindSafe(|| callback(&*context)));
        if result.is_err() {
            let _ = (*context).pending_failures.enqueue(Error::new(
                Errc::PlatformError,
                format!("macOS {callback_name} callback panicked"),
            ));
        }
    }

    unsafe fn window_from_notification(notification: Id) -> Id {
        if notification.is_null() {
            return std::ptr::null_mut();
        }
        msg_id(notification, "object")
    }

    unsafe fn content_view_size(window: Id) -> (i32, i32) {
        if window.is_null() {
            return (0, 0);
        }
        let content_view = msg_id(window, "contentView");
        if content_view.is_null() {
            return (0, 0);
        }
        let frame = msg_rect(content_view, "frame");
        (
            frame.size.width.max(0.0) as i32,
            frame.size.height.max(0.0) as i32,
        )
    }

    unsafe fn add_method(class: Id, name: &str, imp: *const c_void, encoding: &str) -> bool {
        let Ok(encoding) = CString::new(encoding) else {
            return false;
        };
        let selector = sel(name);
        !selector.is_null() && class_addMethod(class, selector, imp, encoding.as_ptr()) != 0
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

    pub unsafe fn msg_id(receiver: Id, selector: &str) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel) -> Id;
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector))
    }

    pub unsafe fn msg_void(receiver: Id, selector: &str) {
        type FnType = unsafe extern "C" fn(Id, Sel);
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector));
    }

    pub unsafe fn msg_void_id(receiver: Id, selector: &str, arg: Id) {
        type FnType = unsafe extern "C" fn(Id, Sel, Id);
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), arg);
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        origin: [f64; 2],
        size: CGSize,
    }

    unsafe fn msg_rect(receiver: Id, selector: &str) -> CGRect {
        #[cfg(target_arch = "x86_64")]
        {
            type FnType = unsafe extern "C" fn(*mut CGRect, Id, Sel);
            let function: FnType =
                std::mem::transmute(objc_msgSend_stret as unsafe extern "C" fn());
            let mut result = std::mem::MaybeUninit::<CGRect>::uninit();
            function(result.as_mut_ptr(), receiver, sel(selector));
            result.assume_init()
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            type FnType = unsafe extern "C" fn(Id, Sel) -> CGRect;
            let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            function(receiver, sel(selector))
        }
    }
}
