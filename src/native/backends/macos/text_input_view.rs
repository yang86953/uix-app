use std::cell::Cell;
use std::collections::VecDeque;
use std::ffi::{c_char, c_void, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};

use crate::native::shared::ime_events::{
    on_committed_text, on_marked_text, on_unmark_text, ImeCompositionState,
};
use crate::native::traits::event::UiEvent;

pub(crate) struct TextInputContext {
    pub events: Arc<Mutex<VecDeque<UiEvent>>>,
    pub composition: ImeCompositionState,
    pub marked_text: String,
    pub session_active: Arc<AtomicBool>,
}

pub(crate) struct MacosTextInput {
    view: Cell<cocoa::Id>,
    session_active: Arc<AtomicBool>,
}

impl MacosTextInput {
    pub(crate) fn new() -> Self {
        Self {
            view: Cell::new(std::ptr::null_mut()),
            session_active: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn session_active_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.session_active)
    }

    pub(crate) fn set_view(&self, view: cocoa::Id) {
        self.view.set(view);
    }

    pub(crate) fn suppress_keydown_text(&self) -> bool {
        self.session_active.load(Ordering::Relaxed)
    }
}

impl crate::native::traits::input::ITextInput for MacosTextInput {
    fn start(&mut self) {
        self.session_active.store(true, Ordering::Relaxed);
        let view = self.view.get();
        if view.is_null() {
            return;
        }
        // SAFETY: view is the window content view installed at creation time.
        unsafe {
            cocoa::make_view_first_responder(view);
        }
    }

    fn stop(&mut self) {
        self.session_active.store(false, Ordering::Relaxed);
        let view = self.view.get();
        if view.is_null() {
            return;
        }
        // SAFETY: view is the window content view installed at creation time.
        unsafe {
            cocoa::resign_view_first_responder(view);
        }
    }
}

pub(crate) unsafe fn create_content_view(
    width: f64,
    height: f64,
    events: Arc<Mutex<VecDeque<UiEvent>>>,
    session_active: Arc<AtomicBool>,
) -> cocoa::Id {
    let frame = cocoa::CGRect {
        origin: cocoa::CGPoint { x: 0.0, y: 0.0 },
        size: cocoa::CGSize { width, height },
    };
    let class = cocoa::content_view_class();
    let view = cocoa::msg_id(class, "alloc");
    let view = cocoa::msg_id_rect(view, "initWithFrame:", frame);
    if view.is_null() {
        return std::ptr::null_mut();
    }
    let context = Box::new(TextInputContext {
        events,
        composition: ImeCompositionState::default(),
        marked_text: String::new(),
        session_active,
    });
    cocoa::set_view_context(view, Box::into_raw(context));
    view
}

pub(crate) unsafe fn make_window_text_input_active(window: cocoa::Id) -> crate::core::Result<()> {
    let view = cocoa::msg_id(window, "contentView");
    if view.is_null() {
        return Err(crate::core::Error::new(
            crate::core::Errc::NotImplemented,
            "macOS text input: window has no content view",
        ));
    }
    cocoa::make_view_first_responder(view);
    Ok(())
}

pub(crate) unsafe fn make_window_text_input_inactive(window: cocoa::Id) -> crate::core::Result<()> {
    cocoa::resign_view_first_responder(cocoa::msg_id(window, "contentView"));
    Ok(())
}

mod cocoa {
    use super::*;

    pub type Id = *mut c_void;
    type Sel = *mut c_void;
    type Bool = i8;

    const YES: Bool = 1;
    const NO: Bool = 0;
    const OBJC_ASSOCIATION_RETAIN_NONATOMIC: usize = 1;
    const NS_NOT_FOUND: usize = usize::MAX;

    static VIEW_CLASS: Once = Once::new();
    static mut VIEW_CLASS_PTR: Id = std::ptr::null_mut();
    static mut VIEW_CONTEXT_KEY: Sel = std::ptr::null_mut();

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct CGPoint {
        pub x: f64,
        pub y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct CGSize {
        pub width: f64,
        pub height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct CGRect {
        pub origin: CGPoint,
        pub size: CGSize,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct NSRange {
        location: usize,
        length: usize,
    }

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra_bytes: usize) -> Id;
        fn class_addMethod(cls: Id, name: Sel, imp: *const c_void, types: *const c_char) -> u8;
        fn objc_registerClassPair(cls: Id);
        fn objc_setAssociatedObject(object: Id, key: Sel, value: Id, policy: usize);
        fn objc_getAssociatedObject(object: Id, key: Sel) -> Id;
    }

    pub unsafe fn content_view_class() -> Id {
        VIEW_CLASS.call_once(|| {
            let superclass = class("NSView");
            let name = CString::new("UixContentView").expect("view class name");
            let class_pair = objc_allocateClassPair(superclass, name.as_ptr(), 0);
            if class_pair.is_null() {
                return;
            }
            VIEW_CONTEXT_KEY = sel("uixTextInputContext");
            let enc_bool = CString::new("c@:").expect("type encoding");
            let enc_void = CString::new("v@:").expect("type encoding");
            let enc_insert = CString::new("v@:@{_NSRange=QQ}").expect("type encoding");
            let enc_marked = CString::new("v@:@{_NSRange=QQ}{_NSRange=QQ}").expect("type encoding");
            let enc_range = CString::new("{_NSRange=QQ}16@0:8").expect("type encoding");

            class_addMethod(
                class_pair,
                sel_registerName(CString::new("acceptsFirstResponder").unwrap().as_ptr()),
                accepts_first_responder as *const c_void,
                enc_bool.as_ptr(),
            );
            class_addMethod(
                class_pair,
                sel_registerName(
                    CString::new("insertText:replacementRange:")
                        .unwrap()
                        .as_ptr(),
                ),
                insert_text as *const c_void,
                enc_insert.as_ptr(),
            );
            class_addMethod(
                class_pair,
                sel_registerName(
                    CString::new("setMarkedText:selectedRange:replacementRange:")
                        .unwrap()
                        .as_ptr(),
                ),
                set_marked_text as *const c_void,
                enc_marked.as_ptr(),
            );
            class_addMethod(
                class_pair,
                sel_registerName(CString::new("unmarkText").unwrap().as_ptr()),
                unmark_text as *const c_void,
                enc_void.as_ptr(),
            );
            class_addMethod(
                class_pair,
                sel_registerName(CString::new("markedRange").unwrap().as_ptr()),
                marked_range as *const c_void,
                enc_range.as_ptr(),
            );
            class_addMethod(
                class_pair,
                sel_registerName(CString::new("selectedRange").unwrap().as_ptr()),
                selected_range as *const c_void,
                enc_range.as_ptr(),
            );
            class_addMethod(
                class_pair,
                sel_registerName(CString::new("hasMarkedText").unwrap().as_ptr()),
                has_marked_text as *const c_void,
                enc_bool.as_ptr(),
            );
            class_addMethod(
                class_pair,
                sel_registerName(CString::new("keyDown:").unwrap().as_ptr()),
                key_down as *const c_void,
                CString::new("v@:@").unwrap().as_ptr(),
            );
            objc_registerClassPair(class_pair);
            VIEW_CLASS_PTR = class_pair;
        });
        VIEW_CLASS_PTR
    }

    pub unsafe fn set_view_context(view: Id, context: *mut TextInputContext) {
        objc_setAssociatedObject(
            view,
            VIEW_CONTEXT_KEY,
            context as Id,
            OBJC_ASSOCIATION_RETAIN_NONATOMIC,
        );
    }

    unsafe fn view_context(view: Id) -> Option<&'static mut TextInputContext> {
        let ptr = objc_getAssociatedObject(view, VIEW_CONTEXT_KEY) as *mut TextInputContext;
        if ptr.is_null() {
            None
        } else {
            Some(&mut *ptr)
        }
    }

    pub unsafe fn make_view_first_responder(view: Id) {
        if view.is_null() {
            return;
        }
        let window = msg_id(view, "window");
        if window.is_null() {
            return;
        }
        msg_void_id(window, "makeFirstResponder:", view);
    }

    pub unsafe fn resign_view_first_responder(view: Id) {
        if view.is_null() {
            return;
        }
        let window = msg_id(view, "window");
        if window.is_null() {
            return;
        }
        let responder = msg_id(window, "firstResponder");
        if responder == view {
            msg_void_id(window, "makeFirstResponder:", std::ptr::null_mut());
        }
    }

    unsafe extern "C" fn accepts_first_responder(_view: Id, _cmd: Sel) -> Bool {
        YES
    }

    unsafe extern "C" fn has_marked_text(view: Id, _cmd: Sel) -> Bool {
        match view_context(view) {
            Some(ctx) if !ctx.marked_text.is_empty() => YES,
            _ => NO,
        }
    }

    unsafe extern "C" fn marked_range(view: Id, _cmd: Sel) -> NSRange {
        match view_context(view) {
            Some(ctx) if !ctx.marked_text.is_empty() => NSRange {
                location: 0,
                length: ctx.marked_text.chars().count(),
            },
            _ => NSRange {
                location: NS_NOT_FOUND,
                length: 0,
            },
        }
    }

    unsafe extern "C" fn selected_range(_view: Id, _cmd: Sel) -> NSRange {
        NSRange {
            location: NS_NOT_FOUND,
            length: 0,
        }
    }

    unsafe extern "C" fn set_marked_text(
        view: Id,
        _cmd: Sel,
        string: Id,
        _selected_range: NSRange,
        _replacement_range: NSRange,
    ) {
        let Some(ctx) = view_context(view) else {
            return;
        };
        let text = id_to_string(string).unwrap_or_default();
        ctx.marked_text = text.clone();
        on_marked_text(&ctx.events, &mut ctx.composition, &text);
    }

    unsafe extern "C" fn insert_text(view: Id, _cmd: Sel, string: Id, _replacement_range: NSRange) {
        let Some(ctx) = view_context(view) else {
            return;
        };
        let text = id_to_string(string).unwrap_or_default();
        ctx.marked_text.clear();
        on_committed_text(&ctx.events, &mut ctx.composition, &text);
    }

    unsafe extern "C" fn unmark_text(view: Id, _cmd: Sel) {
        let Some(ctx) = view_context(view) else {
            return;
        };
        ctx.marked_text.clear();
        on_unmark_text(&ctx.events, &mut ctx.composition);
    }

    unsafe extern "C" fn key_down(view: Id, _cmd: Sel, event: Id) {
        if view.is_null() || event.is_null() {
            return;
        }
        let array = msg_id(class("NSArray"), "alloc");
        let array = msg_id_id(array, "initWithObject:", event);
        if array.is_null() {
            return;
        }
        msg_void_id(view, "interpretKeyEvents:", array);
    }

    unsafe fn id_to_string(value: Id) -> Option<String> {
        if value.is_null() {
            return None;
        }
        let ptr = msg_const_char_ptr(value, "UTF8String");
        if ptr.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned())
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

    pub unsafe fn msg_id_rect(receiver: Id, selector: &str, rect: CGRect) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, CGRect) -> Id;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), rect)
    }

    unsafe fn msg_id_id(receiver: Id, selector: &str, arg: Id) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, Id) -> Id;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), arg)
    }

    unsafe fn msg_void_id(receiver: Id, selector: &str, arg: Id) {
        type FnType = unsafe extern "C" fn(Id, Sel, Id);
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector), arg);
    }

    unsafe fn msg_const_char_ptr(receiver: Id, selector: &str) -> *const c_char {
        type FnType = unsafe extern "C" fn(Id, Sel) -> *const c_char;
        let f: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        f(receiver, sel(selector))
    }
}
