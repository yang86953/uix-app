use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::{CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::{Arc, Mutex, Once};

use crate::core::{Errc, Error, WindowId};
use crate::diagnostics::PendingFailureSource;
use crate::native::windowing::shared::{WindowState, push_window_close, push_window_resize};
use crate::platform::windowing::event::{UiEvent, UiEventPayload, UiEventType};

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
///
/// # Safety
/// 必须在 macOS 主线程传入存活的 NSWindow；`context` 的内部 Rc 只能由同一窗口线程访问直至 delegate dealloc。
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
///
/// # Safety
/// `window` 必须是仍存活且由 `install` 配置过的 UIX NSWindow，并在本同步查询期间保持有效。
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
    // SAFETY: 这些声明与 Objective-C runtime C ABI 一致；类、selector、IMP、关联对象键与结构体返回由窄封装校验。
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

    ///
    /// # Safety
    /// 必须在 macOS 主线程调用；Once 保证动态类只注册一次，返回类由 Objective-C runtime 永久拥有。
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

    ///
    /// # Safety
    /// `delegate` 必须是当前动态类的新实例，其 context ivar 必须仍为空且只允许安装一次。
    pub unsafe fn install_delegate_context(
        delegate: Id,
        context: Box<WindowDelegateContext>,
    ) -> Result<(), Box<WindowDelegateContext>> {
        objc_runtime::install_box(delegate, DELEGATE_CONTEXT_OFFSET, context)
    }

    ///
    /// # Safety
    /// `delegate` 必须是仍存活的当前动态类实例；返回指针不得越过 delegate dealloc 或跨线程使用。
    pub unsafe fn delegate_context(delegate: Id) -> *mut WindowDelegateContext {
        objc_runtime::box_ptr(delegate, DELEGATE_CONTEXT_OFFSET)
    }

    ///
    /// # Safety
    /// `window` 与 `delegate` 必须是存活 Objective-C 对象；关联键唯一且调用必须发生在窗口主线程。
    pub unsafe fn retain_delegate_for_window(window: Id, delegate: Id) {
        objc_setAssociatedObject(
            window,
            delegate_owner_key(),
            delegate,
            OBJC_ASSOCIATION_RETAIN_NONATOMIC,
        );
    }

    ///
    /// # Safety
    /// `window` 必须是存活 Objective-C 对象；返回 delegate 只在 window 继续存活时借用。
    pub unsafe fn owned_delegate(window: Id) -> Id {
        if window.is_null() {
            return std::ptr::null_mut();
        }
        objc_getAssociatedObject(window, delegate_owner_key())
    }

    // SAFETY: 返回地址指向进程期静态字节，仅用作不解引用的 Objective-C 关联对象唯一键。
    unsafe fn delegate_owner_key() -> *const c_void {
        std::ptr::addr_of!(WINDOW_DELEGATE_OWNER_KEY).cast::<c_void>()
    }

    // SAFETY: 仅作为已登记的 dealloc IMP 调用；runtime 保证 delegate 是当前类实例且释放链只执行一次。
    unsafe extern "C" fn delegate_dealloc(delegate: Id, _cmd: Sel) {
        // SAFETY: this class installs the Box exactly once before publication;
        // replacing the ivar with null makes dealloc the unique reclaim point.
        let context =
            objc_runtime::take_box::<WindowDelegateContext>(delegate, DELEGATE_CONTEXT_OFFSET);
        let pending_failures = context
            .as_ref()
            .map(|context| context.pending_failures.clone());
        drop(context);
        // SAFETY: delegate 正处于当前类 dealloc 链，Rust Box 已清空；super 调用被 catch_unwind 隔离在 FFI 边界内。
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

    // SAFETY: 仅由 NSWindowDelegate runtime 按 v@:@ ABI 调用，delegate 在回调期间保持存活。
    unsafe extern "C" fn window_will_close(delegate: Id, _cmd: Sel, _notification: Id) {
        with_context(delegate, "windowWillClose", |context| {
            // 先执行检查式 close 投递，保留可能的队列失败。
            let delivery = push_window_close(&context.events, context.window_id);
            // native window 已进入关闭回调，即使事件队列失败也必须提交 open=false。
            context.open.set(false);
            // 把投递结果交给共享 callback failure 边界。
            delivery
        });
    }

    // SAFETY: 仅由 NSWindowDelegate runtime 按 v@:@ ABI 调用，notification 是当前 resize 通知对象。
    unsafe extern "C" fn window_did_resize(delegate: Id, _cmd: Sel, notification: Id) {
        with_context(delegate, "windowDidResize", |context| {
            // 从本次通知解析唯一窗口对象。
            let window = window_from_notification(notification);
            // 读取同一窗口当前客户区尺寸。
            let (width, height) = content_view_size(window);
            // 检查式提交状态与 resize event，失败进入已有 failure queue。
            push_window_resize(
                &context.events,
                context.window_id,
                &context.state,
                width,
                height,
            )
        });
    }

    // SAFETY: 仅由 NSWindowDelegate runtime 按 v@:@ ABI 调用，delegate 在回调期间保持存活。
    unsafe extern "C" fn window_did_become_key(delegate: Id, _cmd: Sel, _notification: Id) {
        push_focus_event(delegate, UiEventType::WindowFocus);
    }

    // SAFETY: 仅由 NSWindowDelegate runtime 按 v@:@ ABI 调用，delegate 在回调期间保持存活。
    unsafe extern "C" fn window_did_resign_key(delegate: Id, _cmd: Sel, _notification: Id) {
        push_focus_event(delegate, UiEventType::WindowBlur);
    }

    // SAFETY: 仅由 NSWindowDelegate runtime 按 v@:@ ABI 调用，delegate 在回调期间保持存活。
    unsafe extern "C" fn window_did_change_occlusion_state(
        delegate: Id,
        _cmd: Sel,
        _notification: Id,
    ) {
        push_window_event(delegate, UiEvent::window_occlusion_changed());
    }

    // SAFETY: delegate 必须是存活的当前动态类实例，其 Rust context 尚未被 dealloc 接管。
    unsafe fn push_focus_event(delegate: Id, event_type: UiEventType) {
        push_window_event(delegate, UiEvent::new(event_type, UiEventPayload::None));
    }

    // SAFETY: delegate 必须是存活的当前动态类实例，其事件队列在同步入队期间保持有效。
    unsafe fn push_window_event(delegate: Id, event: UiEvent) {
        with_context(delegate, "window event", |context| {
            // 给通用事件附加稳定窗口身份。
            let event = event.for_window(context.window_id);
            // 锁中毒必须成为 typed failure，不能静默丢失 focus/occlusion。
            let mut events = context.events.lock().map_err(|_| {
                // 构造可进入平台 failure queue 的稳定状态错误。
                Error::new(
                    // callback 事件队列已经不可安全使用。
                    Errc::InvalidState,
                    // 保留普通窗口事件的投递阶段。
                    "macOS window callback event queue mutex poisoned",
                )
            })?;
            // 只有成功取得队列 owner 后才提交事件。
            events.push_back(event);
            // 向共享 callback 边界确认投递成功。
            Ok(())
        });
    }

    // SAFETY: delegate 必须是回调期间存活的当前动态类实例；callback 不得保存借用或越过 catch_unwind 展开。
    unsafe fn with_context<F>(delegate: Id, callback_name: &str, callback: F)
    where
        // callback 返回同步投递结果，由本 FFI adapter 统一写入 failure queue。
        F: FnOnce(&WindowDelegateContext) -> crate::core::Result<()>,
    {
        // 读取仅在 delegate 生命周期内存活的 Rust context。
        let context = delegate_context(delegate);
        // 缺少 context 表示对象尚未发布或正在释放，不得解引用。
        if context.is_null() {
            // 无安全失败源可用，只能结束 callback。
            return;
        }
        // 同时隔离 Rust panic 与 callback 返回的 typed delivery failure。
        let result = catch_unwind(AssertUnwindSafe(|| callback(&*context)));
        // 将两种失败形态收敛到同一 PendingFailureSource。
        match result {
            // callback 已成功把事实投递给 owner thread。
            Ok(Ok(())) => {}
            // callback 返回的 typed failure 原样进入 owner-thread failure queue。
            Ok(Err(error)) => {
                // queue 已关闭时没有更高层可继续接收错误，保留既有 fail-closed 语义。
                let _ = (*context).pending_failures.enqueue(error);
            }
            // panic 不得越过 Objective-C FFI 边界。
            Err(_) => {
                // 把 panic 转换为稳定平台错误并投递给 owner thread。
                let _ = (*context).pending_failures.enqueue(Error::new(
                    // panic 属于平台 callback 边界错误。
                    Errc::PlatformError,
                    // 保留发生 panic 的 selector 语义名。
                    format!("macOS {callback_name} callback panicked"),
                ));
            }
        }
    }

    // SAFETY: notification 必须是存活的 NSNotification，并响应无参数、对象返回的 object selector。
    unsafe fn window_from_notification(notification: Id) -> Id {
        if notification.is_null() {
            return std::ptr::null_mut();
        }
        msg_id(notification, "object")
    }

    // SAFETY: window 必须是存活 NSWindow；contentView 与 frame selector 的返回 ABI 必须符合 AppKit 契约。
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

    // SAFETY: class 必须尚未注册，imp 与 encoding 必须精确描述待登记回调的 extern C ABI。
    unsafe fn add_method(class: Id, name: &str, imp: *const c_void, encoding: &str) -> bool {
        let Ok(encoding) = CString::new(encoding) else {
            return false;
        };
        let selector = sel(name);
        !selector.is_null() && class_addMethod(class, selector, imp, encoding.as_ptr()) != 0
    }

    // SAFETY: objc_getClass 同步读取零结尾名称且不保留 Rust 指针；返回类由 runtime 拥有。
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

    ///
    /// # Safety
    /// `receiver` 必须响应无额外参数且返回对象的 `selector`，并在同步消息发送期间保持存活。
    pub unsafe fn msg_id(receiver: Id, selector: &str) -> Id {
        // SAFETY: FnType 精确描述无参数、对象返回值的 Objective-C 消息 ABI。
        type FnType = unsafe extern "C" fn(Id, Sel) -> Id;
        // SAFETY: objc_msgSend 只转换为本函数已经约束的对象返回签名。
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector))
    }

    ///
    /// # Safety
    /// `receiver` 必须响应无额外参数且无返回值的 `selector`，并在同步消息发送期间保持存活。
    pub unsafe fn msg_void(receiver: Id, selector: &str) {
        // SAFETY: FnType 精确描述无参数、无返回值的 Objective-C 消息 ABI。
        type FnType = unsafe extern "C" fn(Id, Sel);
        // SAFETY: objc_msgSend 只转换为本函数已经约束的 void 签名。
        let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, sel(selector));
    }

    ///
    /// # Safety
    /// `receiver` 必须响应接收一个对象且无返回值的 `selector`，两个对象均须在同步调用期间保持存活。
    pub unsafe fn msg_void_id(receiver: Id, selector: &str, arg: Id) {
        // SAFETY: FnType 精确描述单对象参数、无返回值的 Objective-C 消息 ABI。
        type FnType = unsafe extern "C" fn(Id, Sel, Id);
        // SAFETY: objc_msgSend 只转换为本函数已经约束的单对象参数签名。
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

    // SAFETY: receiver 必须响应无参数且返回 CGRect 的 selector；架构分支必须使用各自规定的结构体返回 ABI。
    unsafe fn msg_rect(receiver: Id, selector: &str) -> CGRect {
        #[cfg(target_arch = "x86_64")]
        {
            // SAFETY: x86_64 的 objc_msgSend_stret 使用首参数写入 CGRect，与 AppKit frame 返回 ABI 一致。
            type FnType = unsafe extern "C" fn(*mut CGRect, Id, Sel);
            // SAFETY: 原始 stret 符号只转换为上方已核对的 CGRect 写入签名。
            let function: FnType =
                std::mem::transmute(objc_msgSend_stret as unsafe extern "C" fn());
            let mut result = std::mem::MaybeUninit::<CGRect>::uninit();
            function(result.as_mut_ptr(), receiver, sel(selector));
            result.assume_init()
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            // SAFETY: 非 x86_64 架构按普通寄存器 ABI 返回 CGRect。
            type FnType = unsafe extern "C" fn(Id, Sel) -> CGRect;
            // SAFETY: objc_msgSend 只转换为上方已核对的 CGRect 返回签名。
            let function: FnType = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            function(receiver, sel(selector))
        }
    }
}
