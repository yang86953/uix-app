//! Lazy per-window `CADisplayLink` one-shot frame pacing.
//!
//! Requests are pre-armed while a frame is assembled, but the display link is
//! unpaused only after that exact frame commits. A submitted request is
//! invalidated on cancellation before the link can be reused, preventing a
//! queued callback from completing a replacement token.

#![cfg(target_os = "macos")]

use std::collections::VecDeque;
use std::ffi::{CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use crate::core::{Errc, Error, Result, WindowId};
use crate::diagnostics::PendingFailureSource;
use crate::native::windowing::shared::native_frame_mailbox::{
    NativeFrameArmResult, NativeFrameMailbox,
};
use crate::platform::windowing::event::{FrameRequestToken, UiEvent};
use crate::platform::windowing::window::{NativeFrameRequest, NativeFrameRequestPhase};

use super::objc_runtime;

type Id = *mut c_void;
type SharedMailbox = Arc<Mutex<NativeFrameMailbox>>;

struct DisplayLinkTargetContext {
    events: Arc<Mutex<VecDeque<UiEvent>>>,
    mailbox: SharedMailbox,
    window_id: WindowId,
    pending_failures: PendingFailureSource,
}

pub(crate) struct MacosFramePacer {
    window: Id,
    display_link: Id,
    events: Arc<Mutex<VecDeque<UiEvent>>>,
    mailbox: SharedMailbox,
    window_id: WindowId,
    pending_failures: PendingFailureSource,
}

impl MacosFramePacer {
    pub(crate) fn new(
        window: Id,
        events: Arc<Mutex<VecDeque<UiEvent>>>,
        window_id: WindowId,
        pending_failures: PendingFailureSource,
    ) -> Self {
        Self {
            window,
            display_link: std::ptr::null_mut(),
            events,
            mailbox: Arc::new(Mutex::new(NativeFrameMailbox::default())),
            window_id,
            pending_failures,
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
                    pending_failures: self.pending_failures.clone(),
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
    // SAFETY: 这些声明与 Objective-C runtime C ABI 一致；动态类、selector、IMP 与名称指针均由下方窄封装校验。
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
    // SAFETY: 该空声明只请求链接 AppKit framework，不向 Rust 暴露可直接调用的符号。
    unsafe extern "C" {}

    #[link(name = "Foundation", kind = "framework")]
    // SAFETY: Foundation 全局对象声明与 framework ABI 一致，读取点只借用系统拥有的 NSRunLoopCommonModes。
    unsafe extern "C" {
        static NSRunLoopCommonModes: Id;
    }

    #[link(name = "QuartzCore", kind = "framework")]
    // SAFETY: CACurrentMediaTime 声明与 QuartzCore C ABI 一致，无参数且只返回单调媒体时间快照。
    unsafe extern "C" {
        fn CACurrentMediaTime() -> f64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    // SAFETY: 两个运行循环函数声明与 CoreFoundation C ABI 一致，调用点只传入系统返回的主循环对象。
    unsafe extern "C" {
        fn CFRunLoopGetMain() -> Id;
        fn CFRunLoopWakeUp(run_loop: Id);
    }

    ///
    /// # Safety
    /// 必须在 macOS 主线程传入存活的 NSWindow；`context` 中的队列与邮箱必须至少存活到 target dealloc 接管并释放。
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

    ///
    /// # Safety
    /// `display_link` 必须是仍存活且响应 `setPaused:` 的 CADisplayLink 实例，并由当前线程合法访问。
    pub(super) unsafe fn set_paused(display_link: Id, paused: bool) {
        msg_void_bool(display_link, "setPaused:", if paused { YES } else { NO });
    }

    // SAFETY: 调用方必须位于 macOS 主线程；Once 保证动态类只注册一次，返回类由 Objective-C runtime 永久拥有。
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

    // SAFETY: 仅作为已登记的 CADisplayLink IMP 调用，runtime 保证 target、selector 与 display_link 参数遵循 v@:@ ABI。
    unsafe extern "C" fn display_link_fired(target: Id, _cmd: Sel, display_link: Id) {
        let pending_failures = if target.is_null() {
            None
        } else {
            let context =
                objc_runtime::box_ptr::<DisplayLinkTargetContext>(target, TARGET_CONTEXT_OFFSET);
            (!context.is_null()).then(|| (*context).pending_failures.clone())
        };
        // SAFETY: 回调入口已经取得 runtime 提供的存活 target/display_link；闭包被 catch_unwind 包围，不跨 FFI 展开。
        let result = catch_unwind(AssertUnwindSafe(|| unsafe {
            display_link_fired_inner(target, display_link);
        }));
        if result.is_err() {
            if let Some(pending_failures) = pending_failures {
                let _ = pending_failures.enqueue(Error::new(
                    Errc::PlatformError,
                    "macOS CADisplayLink callback panicked",
                ));
            }
        }
    }

    // SAFETY: 参数必须来自 display_link_fired；target 的 Rust context 槽在 target dealloc 前保持唯一存活。
    unsafe fn display_link_fired_inner(target: Id, display_link: Id) {
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

    // SAFETY: 仅作为已登记的 dealloc IMP 调用；runtime 保证 target 是当前动态类实例且该释放链只执行一次。
    unsafe extern "C" fn target_dealloc(target: Id, _cmd: Sel) {
        let context =
            objc_runtime::take_box::<DisplayLinkTargetContext>(target, TARGET_CONTEXT_OFFSET);
        let pending_failures = context
            .as_ref()
            .map(|context| context.pending_failures.clone());
        drop(context);
        // SAFETY: target 正处于当前类 dealloc 链，Rust Box 已先清空；super 调用被 catch_unwind 隔离在 FFI 边界内。
        let result = catch_unwind(AssertUnwindSafe(|| unsafe {
            objc_runtime::call_super_dealloc(target, TARGET_CLASS_PTR);
        }));
        if result.is_err() {
            if let Some(pending_failures) = pending_failures {
                let _ = pending_failures.enqueue(Error::new(
                    Errc::PlatformError,
                    "macOS CADisplayLink target dealloc callback panicked",
                ));
            }
        }
    }

    // SAFETY: display_link 必须是存活的 CADisplayLink，并响应返回 double 的 targetTimestamp selector。
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

    // SAFETY: receiver 必须是存活 Objective-C 对象；respondsToSelector: 的 BOOL 返回签名由 runtime 固定。
    unsafe fn responds_to_selector(receiver: Id, selector: &str) -> bool {
        // SAFETY: FnType 精确匹配 respondsToSelector: 的 Objective-C C ABI 参数与 BOOL 返回值。
        type FnType = unsafe extern "C" fn(Id, Sel, Sel) -> Bool;
        // SAFETY: objc_msgSend 只在此处转换为上方已经核对的 selector 查询签名。
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel("respondsToSelector:"), sel(selector)) != 0
    }

    // SAFETY: class 必须是尚未注册的存活动态类，imp 与 encoding 必须准确描述回调的 extern C ABI。
    unsafe fn add_method(class: Id, name: &str, imp: *const c_void, encoding: &str) -> bool {
        let Ok(encoding) = CString::new(encoding) else {
            return false;
        };
        let selector = sel(name);
        !selector.is_null() && class_addMethod(class, selector, imp, encoding.as_ptr()) != 0
    }

    // SAFETY: objc_getClass 同步读取零结尾名称且不保留 Rust 指针；返回类对象由 runtime 拥有。
    unsafe fn class(name: &str) -> Id {
        CString::new(name)
            .ok()
            .map(|name| objc_getClass(name.as_ptr()))
            .unwrap_or(std::ptr::null_mut())
    }

    // SAFETY: sel_registerName 同步复制零结尾名称；返回 selector 由 runtime 永久拥有。
    unsafe fn sel(name: &str) -> Sel {
        CString::new(name)
            .ok()
            .map(|name| sel_registerName(name.as_ptr()))
            .unwrap_or(std::ptr::null_mut())
    }

    // SAFETY: receiver 必须响应无额外参数且返回对象的 selector，调用方负责返回对象的约定所有权。
    unsafe fn msg_id(receiver: Id, selector: &str) -> Id {
        // SAFETY: FnType 精确描述该 Objective-C 消息的对象返回 ABI。
        type FnType = unsafe extern "C" fn(Id, Sel) -> Id;
        // SAFETY: objc_msgSend 只转换为本函数已经约束的对象返回签名。
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector))
    }

    // SAFETY: receiver 必须响应接收一个对象和一个 selector 并返回对象的消息，所有参数在同步调用期间存活。
    unsafe fn msg_id_id_sel(receiver: Id, selector: &str, value: Id, action: Sel) -> Id {
        // SAFETY: FnType 精确描述 displayLinkWithTarget:selector: 的对象返回 ABI。
        type FnType = unsafe extern "C" fn(Id, Sel, Id, Sel) -> Id;
        // SAFETY: objc_msgSend 只转换为本函数已经约束的双参数对象返回签名。
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), value, action)
    }

    ///
    /// # Safety
    /// `receiver` 必须响应无额外参数且无返回值的 `selector`，并在同步消息发送期间保持存活。
    pub(super) unsafe fn msg_void(receiver: Id, selector: &str) {
        // SAFETY: FnType 精确描述无参数、无返回值的 Objective-C 消息 ABI。
        type FnType = unsafe extern "C" fn(Id, Sel);
        // SAFETY: objc_msgSend 只转换为本函数已经约束的 void 签名。
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector));
    }

    // SAFETY: receiver 必须响应接收 BOOL 且无返回值的 selector，并在同步调用期间保持存活。
    unsafe fn msg_void_bool(receiver: Id, selector: &str, value: Bool) {
        // SAFETY: FnType 精确描述单 BOOL 参数、无返回值的 Objective-C 消息 ABI。
        type FnType = unsafe extern "C" fn(Id, Sel, Bool);
        // SAFETY: objc_msgSend 只转换为本函数已经约束的 BOOL 参数签名。
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), value);
    }

    // SAFETY: receiver 必须响应接收两个对象且无返回值的 selector，两个对象在同步调用期间保持存活。
    unsafe fn msg_void_id_id(receiver: Id, selector: &str, first: Id, second: Id) {
        // SAFETY: FnType 精确描述双对象参数、无返回值的 Objective-C 消息 ABI。
        type FnType = unsafe extern "C" fn(Id, Sel, Id, Id);
        // SAFETY: objc_msgSend 只转换为本函数已经约束的双对象参数签名。
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector), first, second);
    }

    // SAFETY: receiver 必须响应无额外参数且返回 double 的 selector，并在同步调用期间保持存活。
    unsafe fn msg_f64(receiver: Id, selector: &str) -> f64 {
        // SAFETY: FnType 精确描述无参数、f64 返回值的 Objective-C 消息 ABI。
        type FnType = unsafe extern "C" fn(Id, Sel) -> f64;
        // SAFETY: objc_msgSend 只转换为本函数已经约束的 f64 返回签名。
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
