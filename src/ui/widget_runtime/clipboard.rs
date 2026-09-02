//! 剪贴板服务——让 widget 在事件分发期间访问当前平台剪贴板。
//!
//! 平台剪贴板由事件循环按词法作用域托管；作用域结束后立即恢复先前服务，
//! 不在公开安全 API 中暴露或长期保存可伪造、可悬垂的 trait-object 指针。

use crate::platform::windowing::IClipboard;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;

thread_local! {
    static CURRENT: RefCell<Option<NonNull<dyn IClipboard>>> = const { RefCell::new(None) };
}

/// 在一次受框架托管的调用期间安装剪贴板服务。
///
/// 该函数仅供运行时和 crate 内测试使用。`ClipboardScope` 会在正常返回或
/// panic 展开时恢复先前服务，因此指针不会越过 `clipboard` 的借用期。
pub(crate) fn with_clipboard<R>(
    clipboard: &mut dyn IClipboard,
    operation: impl FnOnce() -> R,
) -> R {
    let pointer = NonNull::from(clipboard);
    // SAFETY: `thread_local!` 的存储类型要求 trait object 为 `'static`。这里只擦除
    // 指针上的借用期；下方作用域守卫保证它在原借用结束前被移除，且指针不会跨线程。
    let pointer: NonNull<dyn IClipboard> = unsafe { std::mem::transmute(pointer) };
    let previous = CURRENT.with(|slot| slot.replace(Some(pointer)));
    let _scope = ClipboardScope {
        previous,
        _not_send: PhantomData,
    };
    operation()
}

struct ClipboardScope {
    previous: Option<NonNull<dyn IClipboard>>,
    // 作用域必须在安装它的线程上销毁，才能恢复正确的 thread-local 状态。
    _not_send: PhantomData<Rc<()>>,
}

impl Drop for ClipboardScope {
    fn drop(&mut self) {
        CURRENT.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

/// widget 调用此方法将文本写入当前平台剪贴板。
///
/// 没有活动窗口事件分发时为 no-op；剪贴板的生命周期由框架维护。
pub fn copy_to_clipboard(text: &str) {
    CURRENT.with(|slot| {
        // 在平台调用期间持有 RefCell 独占借用。若某个自定义 IClipboard
        // 实现重入本服务，则在创建第二个 `&mut` 前拒绝本次调用。
        let Ok(mut current) = slot.try_borrow_mut() else {
            return;
        };
        let Some(pointer) = current.as_mut() else {
            return;
        };
        // SAFETY: `with_clipboard` 安装的指针在作用域结束前始终有效；当前
        // RefCell 独占借用同时阻止通过本服务重入并创建第二个可变引用。
        if let Err(error) = unsafe { pointer.as_mut().set_text(text) } {
            // widget_runtime 层不持有 Diagnostics 句柄（诊断属于 app 组装根，
            // UI 树不得反向依赖）：经显式边界观察入口记录。
            crate::diagnostics::observe_boundary_error("widget_runtime/clipboard", &error);
        }
    });
}

/// widget 调用此方法读取当前平台剪贴板文本。
pub fn read_text_from_clipboard() -> Option<String> {
    CURRENT.with(|slot| {
        let Ok(current) = slot.try_borrow_mut() else {
            return None;
        };
        let pointer = current.as_ref()?;
        // SAFETY: 见 `copy_to_clipboard`。读取期间仍持有 RefCell 独占借用，
        // 因而自定义实现无法通过本服务重入并与该引用发生别名。
        match unsafe { pointer.as_ref().text() } {
            Ok(text) => (!text.is_empty()).then_some(text),
            Err(error) => {
                // 同上：UI 层边界，经显式边界观察入口记录。
                crate::diagnostics::observe_boundary_error("widget_runtime/clipboard", &error);
                None
            }
        }
    })
}
