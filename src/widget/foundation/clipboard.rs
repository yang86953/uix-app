//! 剪贴板服务 — 通过 thread-local 使 widget 可访问平台剪贴板。
//!
//! 将 IClipboard fat pointer 拆分为 (data, vtable) 两个 usize 存储，
//! 断开与 platform borrow 的 provenance 关联。
//!
//! 用法：
//!   1. 应用层调用 `set_clipboard_parts(data, vtable)` 注入剪贴板指针
//!   2. widget 在 Ctrl+C 时调用 `copy_to_clipboard(text)`

use std::cell::Cell;
use crate::platform::IClipboard;

thread_local! {
    static DATA: Cell<usize> = const { Cell::new(0) };
    static VTABLE: Cell<usize> = const { Cell::new(0) };
}

/// 注入剪贴板指针的拆分部分（断开 borrow provenance）。
pub fn set_clipboard_parts(data: usize, vtable: usize) {
    DATA.with(|d| d.set(data));
    VTABLE.with(|v| v.set(vtable));
}

/// widget 调用此方法将文本写入剪贴板。
pub fn copy_to_clipboard(text: &str) {
    DATA.with(|data| {
        let d = data.get();
        if d == 0 {
            return;
        }
        VTABLE.with(|vtable| {
            let v = vtable.get();
            let ptr = fat_ptr_from_parts::<dyn IClipboard>(d, v);
            unsafe {
                (*ptr).set_text(text);
            }
        });
    });
}

fn fat_ptr_from_parts<T: ?Sized>(data: usize, vtable: usize) -> *mut T {
    union FatPtr<T: ?Sized> {
        wide: *mut T,
        parts: (usize, usize),
    }
    unsafe {
        FatPtr::<T> {
            parts: (data, vtable),
        }
        .wide
    }
}
