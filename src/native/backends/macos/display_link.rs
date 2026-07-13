//! Lazy per-window `CADisplayLink` one-shot frame pacing.
//!
//! Requests are pre-armed while a frame is assembled, but the display link is
//! unpaused only after that exact frame commits. A submitted request is
//! invalidated on cancellation before the link can be reused, preventing a
//! queued callback from completing a replacement token.

#![cfg(target_os = "macos")]

use std::collections::VecDeque;
use std::ffi::{c_char, c_void, CString};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use crate::core::{Errc, Error, Result, WindowId};
use crate::native::shared::native_frame_mailbox::{NativeFrameArmResult, NativeFrameMailbox};
use crate::native::traits::event::{FrameRequestToken, UiEvent};
use crate::native::traits::window::{NativeFrameRequest, NativeFrameRequestPhase};

use super::objc_runtime;

type Id = *mut c_void;
type SharedMailbox = Arc<Mutex<NativeFrameMailbox>>;

struct DisplayLinkTargetContext {
    events: Arc<Mutex<VecDeque<UiEvent>>>,
    mailbox: SharedMailbox,
    window_id: WindowId,
}

pub(crate) struct MacosFramePacer {
    window: Id,
    display_link: Id,
    events: Arc<Mutex<VecDeque<UiEvent>>>,
    mailbox: SharedMailbox,
    window_id: WindowId,
}

impl MacosFramePacer {
    pub(crate) fn new(
        window: Id,
        events: Arc<Mutex<VecDeque<UiEvent>>>,
        window_id: WindowId,
    ) -> Self {
        Self {
            window,
            display_link: std::ptr::null_mut(),
            events,
            mailbox: Arc::new(Mutex::new(NativeFrameMailbox::default())),
            window_id,
        }
    }

    pub(crate) fn request(&mut self, request: NativeFrameRequest) -> Result<bool> {
        if request.phase != NativeFrameRequestPhase::AfterPresent {
            return Ok(false);
        }
        if !self.ensure_display_link()? {
            return Ok(false);
        }

        let arm_result = self
            .mailbox
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .arm(request);
        if !matches!(
            arm_result,
            NativeFrameArmResult::Armed {
                replaced_submitted: true
            }
        ) {
            return Ok(true);
        }

        // `invalidate` removes a possibly queued callback and disassociates its
        // target. The replacement request remains unsubmitted in the mailbox.
        self.invalidate_display_link();
        match self.ensure_display_link() {
            Ok(true) => Ok(true),
            Ok(false) => {
                self.cancel_mailbox_request(request.token);
                Ok(false)
            }
            Err(error) => {
                self.cancel_mailbox_request(request.token);
                Err(error)
            }
        }
    }

    pub(crate) fn presented(&mut self, token: FrameRequestToken) -> Result<()> {
        let submitted = self
            .mailbox
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .mark_submitted(token);
        if !submitted {
            return Ok(());
        }
        if self.display_link.is_null() {
            self.cancel_mailbox_request(token);
            return Err(Error::new(
                Errc::InvalidState,
                "macOS display link disappeared before present confirmation",
            ));
        }
        // SAFETY: the link is retained by this pacer and all calls occur on the
        // AppKit UI thread that owns the window and its run loop.
        unsafe {
            cocoa::set_paused(self.display_link, false);
        }
        Ok(())
    }

    pub(crate) fn cancel(&mut self, token: FrameRequestToken) {
        let was_submitted = self
            .mailbox
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .cancel(token);
        if was_submitted {
            self.invalidate_display_link();
        }
    }

    pub(crate) fn shutdown(&mut self) {
        self.mailbox
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
        self.invalidate_display_link();
    }

    fn cancel_mailbox_request(&self, token: FrameRequestToken) {
        self.mailbox
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .cancel(token);
    }

    fn ensure_display_link(&mut self) -> Result<bool> {
        if !self.display_link.is_null() {
            return Ok(true);
        }
        // SAFETY: selector availability is checked before calling the modern
        // AppKit API. The returned link and target follow Cocoa retain rules.
        let display_link = unsafe {
            cocoa::create_window_display_link(
                self.window,
                DisplayLinkTargetContext {
                    events: Arc::clone(&self.events),
                    mailbox: Arc::clone(&self.mailbox),
                    window_id: self.window_id,
                },
            )?
        };
        let Some(display_link) = display_link else {
            return Ok(false);
        };
        self.display_link = display_link;
        Ok(true)
    }

    fn invalidate_display_link(&mut self) {
        let display_link = std::mem::replace(&mut self.display_link, std::ptr::null_mut());
        if display_link.is_null() {
            return;
        }
        // SAFETY: this is the unique +1 retain owned by the pacer. Invalidate
        // first removes the link from every run-loop mode and its target.
        unsafe {
            cocoa::set_paused(display_link, true);
            cocoa::msg_void(display_link, "invalidate");
            cocoa::msg_void(display_link, "release");
        }
    }
}

impl Drop for MacosFramePacer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

mod cocoa {
    use super::*;

    type Sel = *mut c_void;
    type Bool = u8;

    const YES: Bool = 1;
    const NO: Bool = 0;

    static TARGET_CLASS: Once = Once::new();
    static mut TARGET_CLASS_PTR: Id = std::ptr::null_mut();
    static mut TARGET_CONTEXT_OFFSET: isize = objc_runtime::INVALID_IVAR_OFFSET;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra_bytes: usize) -> Id;
        fn objc_disposeClassPair(cls: Id);
        fn class_addMethod(cls: Id, name: Sel, imp: *const c_void, types: *const c_char) -> Bool;
        fn objc_registerClassPair(cls: Id);
    }

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {
        static NSRunLoopCommonModes: Id;
    }

    #[link(name = "QuartzCore", kind = "framework")]
    unsafe extern "C" {
        fn CACurrentMediaTime() -> f64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRunLoopGetMain() -> Id;
        fn CFRunLoopWakeUp(run_loop: Id);
    }

    pub(super) unsafe fn create_window_display_link(
        window: Id,
        context: DisplayLinkTargetContext,
    ) -> Result<Option<Id>> {
        if window.is_null() || !responds_to_selector(window, "displayLinkWithTarget:selector:") {
            return Ok(None);
        }
        let target_class = target_class();
        if target_class.is_null() {
            return Err(platform_error(
                "failed to register UixDisplayLinkTarget Objective-C class",
            ));
        }
        let target = msg_id(target_class, "new");
        if target.is_null() {
            return Err(platform_error("failed to allocate display-link target"));
        }
        if let Err(context) =
            objc_runtime::install_box(target, TARGET_CONTEXT_OFFSET, Box::new(context))
        {
            msg_void(target, "release");
            drop(context);
            return Err(platform_error(
                "failed to install display-link target Rust context",
            ));
        }

        let display_link = msg_id_id_sel(
            window,
            "displayLinkWithTarget:selector:",
            target,
            sel("uixDisplayLinkFired:"),
        );
        if display_link.is_null() {
            msg_void(target, "release");
            return Err(platform_error("NSWindow returned a null CADisplayLink"));
        }

        // Own one retain independently from the run loop. CADisplayLink retains
        // `target`, so balance the target's `new` after link construction.
        msg_void(display_link, "retain");
        set_paused(display_link, true);
        let run_loop = msg_id(class("NSRunLoop"), "mainRunLoop");
        if run_loop.is_null() || NSRunLoopCommonModes.is_null() {
            msg_void(display_link, "invalidate");
            msg_void(display_link, "release");
            msg_void(target, "release");
            return Err(platform_error(
                "main NSRunLoop or common mode is unavailable",
            ));
        }
        msg_void_id_id(
            display_link,
            "addToRunLoop:forMode:",
            run_loop,
            NSRunLoopCommonModes,
        );
        msg_void(target, "release");
        Ok(Some(display_link))
    }

    pub(super) unsafe fn set_paused(display_link: Id, paused: bool) {
        msg_void_bool(display_link, "setPaused:", if paused { YES } else { NO });
    }

    unsafe fn target_class() -> Id {
        TARGET_CLASS.call_once(|| {
            let superclass = class("NSObject");
            let Ok(name) = CString::new("UixDisplayLinkTarget") else {
                return;
            };
            let class_pair = objc_allocateClassPair(superclass, name.as_ptr(), 0);
            if class_pair.is_null() {
                return;
            }
            let Some(context_offset) =
                objc_runtime::add_raw_pointer_ivar(class_pair, "_uixDisplayLinkContext")
            else {
                objc_disposeClassPair(class_pair);
                return;
            };
            let callbacks_ok = add_method(
                class_pair,
                "uixDisplayLinkFired:",
                display_link_fired as *const c_void,
                "v@:@",
            ) && add_method(
                class_pair,
                "dealloc",
                target_dealloc as *const c_void,
                "v@:",
            );
            if !callbacks_ok {
                objc_disposeClassPair(class_pair);
                return;
            }
            objc_registerClassPair(class_pair);
            TARGET_CONTEXT_OFFSET = context_offset;
            TARGET_CLASS_PTR = class_pair;
        });
        TARGET_CLASS_PTR
    }

    unsafe extern "C" fn display_link_fired(target: Id, _cmd: Sel, display_link: Id) {
        if display_link.is_null() {
            return;
        }
        set_paused(display_link, true);
        let context =
            objc_runtime::box_ptr::<DisplayLinkTargetContext>(target, TARGET_CONTEXT_OFFSET);
        if context.is_null() {
            return;
        }
        let request = (*context)
            .mailbox
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take_submitted();
        let Some(request) = request else {
            return;
        };

        let frame_time = Instant::now();
        let target_present_time = target_present_time(display_link, frame_time);
        let event = UiEvent::frame_opportunity(request.token, frame_time, target_present_time)
            .for_window((*context).window_id);
        (*context)
            .events
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push_back(event);
        let run_loop = CFRunLoopGetMain();
        if !run_loop.is_null() {
            CFRunLoopWakeUp(run_loop);
        }
    }

    unsafe extern "C" fn target_dealloc(target: Id, _cmd: Sel) {
        drop(objc_runtime::take_box::<DisplayLinkTargetContext>(
            target,
            TARGET_CONTEXT_OFFSET,
        ));
        objc_runtime::call_super_dealloc(target, TARGET_CLASS_PTR);
    }

    unsafe fn target_present_time(display_link: Id, frame_time: Instant) -> Option<Instant> {
        let target_timestamp = msg_f64(display_link, "targetTimestamp");
        let media_time = CACurrentMediaTime();
        if !target_timestamp.is_finite() || !media_time.is_finite() {
            return None;
        }
        let remaining = target_timestamp - media_time;
        if !(0.0..=1.0).contains(&remaining) {
            return None;
        }
        frame_time.checked_add(Duration::from_secs_f64(remaining))
    }

    unsafe fn responds_to_selector(receiver: Id, selector: &str) -> bool {
        type FnType = unsafe extern "C" fn(Id, Sel, Sel) -> Bool;
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel("respondsToSelector:"), sel(selector)) != 0
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

    unsafe fn msg_id(receiver: Id, selector: &str) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel) -> Id;
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector))
    }

    unsafe fn msg_id_id_sel(receiver: Id, selector: &str, value: Id, action: Sel) -> Id {
        type FnType = unsafe extern "C" fn(Id, Sel, Id, Sel) -> Id;
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), value, action)
    }

    pub(super) unsafe fn msg_void(receiver: Id, selector: &str) {
        type FnType = unsafe extern "C" fn(Id, Sel);
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector));
    }

    unsafe fn msg_void_bool(receiver: Id, selector: &str, value: Bool) {
        type FnType = unsafe extern "C" fn(Id, Sel, Bool);
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), value);
    }

    unsafe fn msg_void_id_id(receiver: Id, selector: &str, first: Id, second: Id) {
        type FnType = unsafe extern "C" fn(Id, Sel, Id, Id);
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), first, second);
    }

    unsafe fn msg_f64(receiver: Id, selector: &str) -> f64 {
        type FnType = unsafe extern "C" fn(Id, Sel) -> f64;
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector))
    }

    fn platform_error(message: impl Into<String>) -> Error {
        Error::new(
            Errc::PlatformError,
            format!("macOS display link: {}", message.into()),
        )
    }
}
