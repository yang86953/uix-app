use std::collections::VecDeque;
use std::ffi::{CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard, Once};

use crate::core::{Errc, Error, WindowId};
use crate::diagnostics::PendingFailureSource;
use crate::native::windowing::event::UiEvent;
use crate::native::windowing::shared::ime_events::{
    ImeCompositionState, on_committed_text_for_window, on_marked_text_for_window,
    on_unmark_text_for_window,
};
use crate::native::windowing::shared::ime_owner::{
    NativeImeOwner, NativeImeSession, NativeImeTarget,
};

use super::objc_runtime;

pub(crate) type SharedImeOwner = Arc<Mutex<NativeImeOwner>>;

pub(crate) struct TextInputContext {
    events: Arc<Mutex<VecDeque<UiEvent>>>,
    window_id: WindowId,
    owner: SharedImeOwner,
    pending_failures: PendingFailureSource,
    composition: ImeCompositionState,
    marked_text: String,
    session_generation: Option<u64>,
}

impl TextInputContext {
    fn target(&self, view: cocoa::Id) -> NativeImeTarget {
        NativeImeTarget::new(self.window_id, view as usize)
    }

    fn accepts_callback(&self, view: cocoa::Id) -> bool {
        let Some(generation) = self.session_generation else {
            return false;
        };
        lock_owner(&self.owner).matches(self.target(view), generation)
    }
}

pub(crate) struct MacosTextInput {
    owner: SharedImeOwner,
}

impl MacosTextInput {
    pub(crate) fn new() -> Self {
        Self {
            owner: Arc::new(Mutex::new(NativeImeOwner::default())),
        }
    }

    pub(crate) fn owner_handle(&self) -> SharedImeOwner {
        Arc::clone(&self.owner)
    }

    pub(crate) fn suppress_keydown_text(&self, window_id: Option<WindowId>) -> bool {
        let Some(window_id) = window_id else {
            return false;
        };
        lock_owner(&self.owner)
            .active()
            .is_some_and(|session| session.target.window_id == window_id)
    }
}

impl Drop for MacosTextInput {
    fn drop(&mut self) {
        // SAFETY: owner targets are registered UixContentView instances. Their
        // dealloc callback removes the target before its pointer becomes stale.
        unsafe {
            shutdown_owner(&self.owner);
        }
    }
}

impl crate::native::windowing::input::ITextInput for MacosTextInput {
    fn set_target_window(
        &mut self,
        window_id: WindowId,
        native_window: *mut c_void,
    ) -> crate::core::Result<()> {
        // SAFETY: the app coordinator supplies the live NSWindow paired with
        // `window_id`; select_window verifies the content-view context agrees.
        unsafe { select_window(&self.owner, window_id, native_window).map(|_| ()) }
    }

    fn start(&mut self) -> crate::core::Result<()> {
        // SAFETY: selection verifies and records a live UixContentView target.
        unsafe { start_selected(&self.owner) }
    }

    fn stop(&mut self) -> crate::core::Result<()> {
        // SAFETY: only the selected active generation can be stopped.
        unsafe { stop_selected(&self.owner) }
    }

    fn set_cursor_rect(&mut self, _rect: crate::core::Rect) -> crate::core::Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "macOS text input cursor rect is not connected to NSTextInputClient",
        ))
    }
}

pub(crate) unsafe fn create_content_view(
    width: f64,
    height: f64,
    events: Arc<Mutex<VecDeque<UiEvent>>>,
    window_id: WindowId,
    owner: SharedImeOwner,
    pending_failures: PendingFailureSource,
) -> crate::core::Result<cocoa::Id> {
    let frame = cocoa::CGRect {
        origin: cocoa::CGPoint { x: 0.0, y: 0.0 },
        size: cocoa::CGSize { width, height },
    };
    let class = cocoa::content_view_class();
    if class.is_null() {
        return Err(platform_error(
            "failed to register UixContentView Objective-C class",
        ));
    }
    let view = cocoa::msg_id(class, "alloc");
    let view = cocoa::msg_id_rect(view, "initWithFrame:", frame);
    if view.is_null() {
        return Err(platform_error("failed to allocate UixContentView"));
    }
    let context = Box::new(TextInputContext {
        events,
        window_id,
        owner,
        pending_failures,
        composition: ImeCompositionState::default(),
        marked_text: String::new(),
        session_generation: None,
    });
    if let Err(context) = cocoa::install_view_context(view, context) {
        cocoa::msg_void(view, "release");
        drop(context);
        return Err(platform_error(
            "failed to install UixContentView Rust context ivar",
        ));
    }
    Ok(view)
}

pub(crate) unsafe fn make_window_text_input_active(
    owner: &SharedImeOwner,
    window_id: WindowId,
    window: cocoa::Id,
) -> crate::core::Result<()> {
    select_window(owner, window_id, window)?;
    start_selected(owner)
}

pub(crate) unsafe fn make_window_text_input_inactive(
    owner: &SharedImeOwner,
    window_id: WindowId,
    window: cocoa::Id,
) -> crate::core::Result<()> {
    select_window(owner, window_id, window)?;
    stop_selected(owner)
}

unsafe fn select_window(
    owner: &SharedImeOwner,
    window_id: WindowId,
    native_window: cocoa::Id,
) -> crate::core::Result<NativeImeTarget> {
    if native_window.is_null() {
        return Err(Error::new(
            Errc::InvalidOperation,
            "macOS text input: target window is null",
        ));
    }
    let view = cocoa::msg_id(native_window, "contentView");
    if view.is_null() {
        return Err(Error::new(
            Errc::InvalidOperation,
            "macOS text input: target window has no content view",
        ));
    }
    if cocoa::view_window_id(view) != Some(window_id) {
        return Err(Error::new(
            Errc::InvalidOperation,
            "macOS text input: target window and UixContentView identity disagree",
        ));
    }
    let target = NativeImeTarget::new(window_id, view as usize);
    lock_owner(owner).select(target);
    Ok(target)
}

unsafe fn start_selected(owner: &SharedImeOwner) -> crate::core::Result<()> {
    let activation = lock_owner(owner).activate_selected().ok_or_else(|| {
        Error::new(
            Errc::InvalidOperation,
            "macOS text input: no target window selected",
        )
    })?;

    if let Some(previous) = activation.previous {
        cocoa::finish_view_session(previous);
        cocoa::resign_view_first_responder(previous.target.native_id as cocoa::Id);
    }
    if !cocoa::begin_view_session(activation.current) {
        lock_owner(owner).fail_activation(activation.current);
        return Err(platform_error(
            "selected UixContentView disappeared before IME activation",
        ));
    }
    if !cocoa::make_view_first_responder(activation.current.target.native_id as cocoa::Id) {
        cocoa::finish_view_session(activation.current);
        lock_owner(owner).fail_activation(activation.current);
        return Err(Error::new(
            Errc::InvalidOperation,
            "macOS text input: NSWindow rejected UixContentView as first responder",
        ));
    }
    Ok(())
}

unsafe fn stop_selected(owner: &SharedImeOwner) -> crate::core::Result<()> {
    let Some(session) = lock_owner(owner).deactivate_selected() else {
        // A delayed blur may select an old window after another window became
        // active. It must not stop the new owner's session.
        return Ok(());
    };
    cocoa::finish_view_session(session);
    cocoa::resign_view_first_responder(session.target.native_id as cocoa::Id);
    Ok(())
}

unsafe fn shutdown_owner(owner: &SharedImeOwner) {
    let Some(session) = lock_owner(owner).deactivate_active() else {
        return;
    };
    cocoa::finish_view_session(session);
    cocoa::resign_view_first_responder(session.target.native_id as cocoa::Id);
}

fn lock_owner(owner: &SharedImeOwner) -> MutexGuard<'_, NativeImeOwner> {
    owner.lock().unwrap_or_else(|error| error.into_inner())
}

fn platform_error(message: impl Into<String>) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("macOS text input: {}", message.into()),
    )
}

mod cocoa {
    use super::*;

    pub type Id = *mut c_void;
    type Sel = *mut c_void;
    type Bool = i8;

    const YES: Bool = 1;
    const NO: Bool = 0;
    const NS_NOT_FOUND: usize = usize::MAX;

    static VIEW_CLASS: Once = Once::new();
    static mut VIEW_CLASS_PTR: Id = std::ptr::null_mut();
    static mut VIEW_CONTEXT_OFFSET: isize = objc_runtime::INVALID_IVAR_OFFSET;

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
        fn objc_disposeClassPair(cls: Id);
        fn class_addMethod(cls: Id, name: Sel, imp: *const c_void, types: *const c_char) -> u8;
        fn objc_registerClassPair(cls: Id);
    }

    pub unsafe fn content_view_class() -> Id {
        VIEW_CLASS.call_once(|| {
            let superclass = class("NSView");
            let Ok(name) = CString::new("UixContentView") else {
                return;
            };
            let class_pair = objc_allocateClassPair(superclass, name.as_ptr(), 0);
            if class_pair.is_null() {
                return;
            }
            let Some(context_offset) =
                objc_runtime::add_raw_pointer_ivar(class_pair, "_uixTextInputContext")
            else {
                objc_disposeClassPair(class_pair);
                return;
            };
            let callbacks_ok =
                add_method(
                    class_pair,
                    "acceptsFirstResponder",
                    accepts_first_responder as *const c_void,
                    "c@:",
                ) && add_method(
                    class_pair,
                    "insertText:replacementRange:",
                    insert_text as *const c_void,
                    "v@:@{_NSRange=QQ}",
                ) && add_method(
                    class_pair,
                    "setMarkedText:selectedRange:replacementRange:",
                    set_marked_text as *const c_void,
                    "v@:@{_NSRange=QQ}{_NSRange=QQ}",
                ) && add_method(
                    class_pair,
                    "unmarkText",
                    unmark_text as *const c_void,
                    "v@:",
                ) && add_method(
                    class_pair,
                    "markedRange",
                    marked_range as *const c_void,
                    "{_NSRange=QQ}16@0:8",
                ) && add_method(
                    class_pair,
                    "selectedRange",
                    selected_range as *const c_void,
                    "{_NSRange=QQ}16@0:8",
                ) && add_method(
                    class_pair,
                    "hasMarkedText",
                    has_marked_text as *const c_void,
                    "c@:",
                ) && add_method(class_pair, "keyDown:", key_down as *const c_void, "v@:@")
                    && add_method(class_pair, "dealloc", view_dealloc as *const c_void, "v@:");
            if !callbacks_ok {
                objc_disposeClassPair(class_pair);
                return;
            }
            objc_registerClassPair(class_pair);
            VIEW_CONTEXT_OFFSET = context_offset;
            VIEW_CLASS_PTR = class_pair;
        });
        VIEW_CLASS_PTR
    }

    pub unsafe fn install_view_context(
        view: Id,
        context: Box<TextInputContext>,
    ) -> Result<(), Box<TextInputContext>> {
        objc_runtime::install_box(view, VIEW_CONTEXT_OFFSET, context)
    }

    unsafe fn view_context(view: Id) -> *mut TextInputContext {
        objc_runtime::box_ptr(view, VIEW_CONTEXT_OFFSET)
    }

    pub unsafe fn view_window_id(view: Id) -> Option<WindowId> {
        let context = view_context(view);
        (!context.is_null()).then(|| (*context).window_id)
    }

    pub unsafe fn begin_view_session(session: NativeImeSession) -> bool {
        let view = session.target.native_id as Id;
        let context = view_context(view);
        if context.is_null() || (*context).target(view) != session.target {
            return false;
        }
        (*context).session_generation = Some(session.generation);
        true
    }

    pub unsafe fn finish_view_session(session: NativeImeSession) {
        let view = session.target.native_id as Id;
        let context = view_context(view);
        if context.is_null()
            || (*context).target(view) != session.target
            || (*context).session_generation != Some(session.generation)
        {
            return;
        }
        (*context).session_generation = None;
        (*context).marked_text.clear();
        on_unmark_text_for_window(
            &(*context).events,
            &mut (*context).composition,
            (*context).window_id,
        );
    }

    pub unsafe fn make_view_first_responder(view: Id) -> bool {
        if view.is_null() {
            return false;
        }
        let window = msg_id(view, "window");
        if window.is_null() {
            return false;
        }
        msg_bool_id(window, "makeFirstResponder:", view) != NO
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
            let _ = msg_bool_id(window, "makeFirstResponder:", std::ptr::null_mut());
        }
    }

    fn report_callback_panic(pending_failures: Option<PendingFailureSource>, callback_name: &str) {
        if let Some(pending_failures) = pending_failures {
            let _ = pending_failures.enqueue(Error::new(
                Errc::PlatformError,
                format!("macOS text input {callback_name} callback panicked"),
            ));
        }
    }

    unsafe fn callback_failure_source(view: Id) -> Option<PendingFailureSource> {
        let context = view_context(view);
        (!context.is_null()).then(|| (*context).pending_failures.clone())
    }

    unsafe fn with_callback<T, F>(view: Id, callback_name: &str, fallback: T, callback: F) -> T
    where
        F: FnOnce() -> T,
    {
        let pending_failures = callback_failure_source(view);
        match catch_unwind(AssertUnwindSafe(callback)) {
            Ok(value) => value,
            Err(_) => {
                report_callback_panic(pending_failures, callback_name);
                fallback
            }
        }
    }

    unsafe extern "C" fn view_dealloc(view: Id, _cmd: Sel) {
        // SAFETY: UixContentView installs the Box once before publication and
        // clears the ivar here, making this the unique Rust reclaim point.
        let context = objc_runtime::take_box::<TextInputContext>(view, VIEW_CONTEXT_OFFSET);
        let pending_failures = context
            .as_ref()
            .map(|context| context.pending_failures.clone());
        let cleanup = catch_unwind(AssertUnwindSafe(|| {
            if let Some(mut context) = context {
                let target = context.target(view);
                lock_owner(&context.owner).forget_target(target);
                context.session_generation = None;
                context.marked_text.clear();
                on_unmark_text_for_window(
                    &context.events,
                    &mut context.composition,
                    context.window_id,
                );
                drop(context);
            }
        }));
        if cleanup.is_err() {
            report_callback_panic(pending_failures.clone(), "view dealloc");
        }
        let superclass_dealloc = catch_unwind(AssertUnwindSafe(|| {
            objc_runtime::call_super_dealloc(view, VIEW_CLASS_PTR);
        }));
        if superclass_dealloc.is_err() {
            report_callback_panic(pending_failures, "view superclass dealloc");
        }
    }

    unsafe extern "C" fn accepts_first_responder(view: Id, _cmd: Sel) -> Bool {
        with_callback(view, "acceptsFirstResponder", NO, || YES)
    }

    unsafe extern "C" fn has_marked_text(view: Id, _cmd: Sel) -> Bool {
        with_callback(view, "hasMarkedText", NO, || {
            let context = view_context(view);
            if context.is_null() {
                return NO;
            }
            let context = &mut *context;
            if context.accepts_callback(view) && !context.marked_text.is_empty() {
                YES
            } else {
                NO
            }
        })
    }

    unsafe extern "C" fn marked_range(view: Id, _cmd: Sel) -> NSRange {
        with_callback(view, "markedRange", not_found_range(), || {
            let context = view_context(view);
            if context.is_null() {
                return not_found_range();
            }
            let context = &mut *context;
            if context.accepts_callback(view) && !context.marked_text.is_empty() {
                NSRange {
                    location: 0,
                    length: context.marked_text.chars().count(),
                }
            } else {
                not_found_range()
            }
        })
    }

    unsafe extern "C" fn selected_range(view: Id, _cmd: Sel) -> NSRange {
        with_callback(view, "selectedRange", not_found_range(), not_found_range)
    }

    unsafe extern "C" fn set_marked_text(
        view: Id,
        _cmd: Sel,
        string: Id,
        _selected_range: NSRange,
        _replacement_range: NSRange,
    ) {
        with_callback(view, "setMarkedText", (), || {
            let context = view_context(view);
            if context.is_null() {
                return;
            }
            let context = &mut *context;
            if !context.accepts_callback(view) {
                return;
            }
            let text = id_to_string(string).unwrap_or_default();
            context.marked_text = text.clone();
            on_marked_text_for_window(
                &context.events,
                &mut context.composition,
                &text,
                context.window_id,
            );
        });
    }

    unsafe extern "C" fn insert_text(view: Id, _cmd: Sel, string: Id, _replacement_range: NSRange) {
        with_callback(view, "insertText", (), || {
            let context = view_context(view);
            if context.is_null() {
                return;
            }
            let context = &mut *context;
            if !context.accepts_callback(view) {
                return;
            }
            let text = id_to_string(string).unwrap_or_default();
            context.marked_text.clear();
            on_committed_text_for_window(
                &context.events,
                &mut context.composition,
                &text,
                context.window_id,
            );
        });
    }

    unsafe extern "C" fn unmark_text(view: Id, _cmd: Sel) {
        with_callback(view, "unmarkText", (), || {
            let context = view_context(view);
            if context.is_null() {
                return;
            }
            let context = &mut *context;
            if !context.accepts_callback(view) {
                return;
            }
            context.marked_text.clear();
            on_unmark_text_for_window(&context.events, &mut context.composition, context.window_id);
        });
    }

    unsafe extern "C" fn key_down(view: Id, _cmd: Sel, event: Id) {
        with_callback(view, "keyDown", (), || {
            let context = view_context(view);
            if context.is_null() || event.is_null() {
                return;
            }
            let context = &mut *context;
            if !context.accepts_callback(view) {
                return;
            }
            let array = msg_id(class("NSArray"), "alloc");
            let array = msg_id_id(array, "initWithObject:", event);
            if array.is_null() {
                return;
            }
            msg_void_id(view, "interpretKeyEvents:", array);
            msg_void(array, "release");
        });
    }

    fn not_found_range() -> NSRange {
        NSRange {
            location: NS_NOT_FOUND,
            length: 0,
        }
    }

    unsafe fn id_to_string(value: Id) -> Option<String> {
        if value.is_null() {
            return None;
        }
        let utf8_selector = sel("UTF8String");
        if msg_bool_sel(value, "respondsToSelector:", utf8_selector) != NO {
            let pointer = msg_const_char_ptr(value, "UTF8String");
            if !pointer.is_null() {
                return Some(
                    std::ffi::CStr::from_ptr(pointer)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        let string_selector = sel("string");
        if msg_bool_sel(value, "respondsToSelector:", string_selector) != NO {
            let plain = msg_id(value, "string");
            if plain != value {
                return id_to_string(plain);
            }
        }
        None
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

    pub unsafe fn msg_id_rect(receiver: Id, selector: &str, rect: CGRect) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, CGRect) -> Id;
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), rect)
    }

    unsafe fn msg_id_id(receiver: Id, selector: &str, arg: Id) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, Id) -> Id;
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), arg)
    }

    unsafe fn msg_bool_id(receiver: Id, selector: &str, arg: Id) -> Bool {
        type FnType = unsafe extern "C" fn(Id, Sel, Id) -> Bool;
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), arg)
    }

    unsafe fn msg_bool_sel(receiver: Id, selector: &str, arg: Sel) -> Bool {
        type FnType = unsafe extern "C" fn(Id, Sel, Sel) -> Bool;
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), arg)
    }

    pub unsafe fn msg_void(receiver: Id, selector: &str) {
        type FnType = unsafe extern "C" fn(Id, Sel);
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector));
    }

    unsafe fn msg_void_id(receiver: Id, selector: &str, arg: Id) {
        type FnType = unsafe extern "C" fn(Id, Sel, Id);
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), arg);
    }

    unsafe fn msg_const_char_ptr(receiver: Id, selector: &str) -> *const c_char {
        type FnType = unsafe extern "C" fn(Id, Sel) -> *const c_char;
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector))
    }
}
