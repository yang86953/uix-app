use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::{c_char, c_void, CString};
use std::rc::Rc;
use std::sync::{Arc, Mutex, Once};

use crate::core::WindowId;
use crate::native::shared::{push_window_close, push_window_resize, WindowState};
use crate::native::traits::event::UiEvent;

pub(crate) struct WindowDelegateContext {
    pub events: Arc<Mutex<VecDeque<UiEvent>>>,
    pub window_id: WindowId,
    pub state: Rc<RefCell<WindowState>>,
}

pub(crate) unsafe fn install(window: cocoa::Id, context: WindowDelegateContext) {
    let delegate = cocoa::msg_id(cocoa::delegate_class(), "new");
    if delegate.is_null() {
        return;
    }
    let context = Box::new(context);
    cocoa::set_delegate_context(delegate, Box::into_raw(context));
    cocoa::msg_void_id(window, "setDelegate:", delegate);
}

mod cocoa {
    use super::*;

    pub type Id = *mut c_void;
    type Sel = *mut c_void;

    const OBJC_ASSOCIATION_RETAIN_NONATOMIC: usize = 1;

    static DELEGATE_CLASS: Once = Once::new();
    static mut DELEGATE_CLASS_PTR: Id = std::ptr::null_mut();
    static mut DELEGATE_CONTEXT_KEY: Sel = std::ptr::null_mut();

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra_bytes: usize) -> Id;
        fn class_addMethod(
            cls: Id,
            name: Sel,
            imp: *const c_void,
            types: *const c_char,
        ) -> u8;
        fn objc_registerClassPair(cls: Id);
        fn objc_setAssociatedObject(object: Id, key: Sel, value: Id, policy: usize);
        fn objc_getAssociatedObject(object: Id, key: Sel) -> Id;
    }

    pub unsafe fn delegate_class() -> Id {
        DELEGATE_CLASS.call_once(|| {
            let superclass = class("NSObject");
            let name = CString::new("UixWindowDelegate").expect("delegate class name");
            let class_pair = objc_allocateClassPair(superclass, name.as_ptr(), 0);
            if class_pair.is_null() {
                return;
            }
            DELEGATE_CONTEXT_KEY = sel("uixDelegateContext");
            let will_close = CString::new("windowWillClose:").expect("selector");
            let did_resize = CString::new("windowDidResize:").expect("selector");
            let enc = CString::new("v@:@").expect("type encoding");
            class_addMethod(
                class_pair,
                sel_registerName(will_close.as_ptr()),
                window_will_close as *const c_void,
                enc.as_ptr(),
            );
            class_addMethod(
                class_pair,
                sel_registerName(did_resize.as_ptr()),
                window_did_resize as *const c_void,
                enc.as_ptr(),
            );
            objc_registerClassPair(class_pair);
            DELEGATE_CLASS_PTR = class_pair;
        });
        DELEGATE_CLASS_PTR
    }

    pub unsafe fn set_delegate_context(delegate: Id, context: *mut WindowDelegateContext) {
        objc_setAssociatedObject(
            delegate,
            DELEGATE_CONTEXT_KEY,
            context as Id,
            OBJC_ASSOCIATION_RETAIN_NONATOMIC,
        );
    }

    unsafe fn delegate_context(delegate: Id) -> Option<&'static WindowDelegateContext> {
        let ptr = objc_getAssociatedObject(delegate, DELEGATE_CONTEXT_KEY) as *mut WindowDelegateContext;
        if ptr.is_null() {
            None
        } else {
            Some(&*ptr)
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

    unsafe extern "C" fn window_will_close(delegate: Id, _cmd: Sel, notification: Id) {
        let Some(ctx) = delegate_context(delegate) else {
            return;
        };
        push_window_close(&ctx.events, ctx.window_id);
        let _window = window_from_notification(notification);
    }

    unsafe extern "C" fn window_did_resize(delegate: Id, _cmd: Sel, notification: Id) {
        let Some(ctx) = delegate_context(delegate) else {
            return;
        };
        let window = window_from_notification(notification);
        let (width, height) = content_view_size(window);
        push_window_resize(&ctx.events, ctx.window_id, &ctx.state, width, height);
    }

    unsafe fn class(name: &str) -> Id {
        let name = CString::new(name).expect("class name");
        objc_getClass(name.as_ptr())
    }

    unsafe fn sel(name: &str) -> Sel {
        let name = CString::new(name).expect("selector");
        sel_registerName(name.as_ptr())
    }

    pub unsafe fn msg_id(receiver: Id, selector: &str) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel) -> Id;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector))
    }

    pub unsafe fn msg_void_id(receiver: Id, selector: &str, arg: Id) {
        type FnType = unsafe extern "C" fn(Id, Sel, Id);
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), arg);
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
        type FnType = unsafe extern "C" fn(Id, Sel) -> CGRect;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector))
    }
}
