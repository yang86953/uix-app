#![cfg(windows)]
use super::ffi::*;

// ============================================================================
// native/backends/windows/clipboard.rs — Windows clipboard (IClipboard)
// ============================================================================

use crate::native::backends::windows::util::to_utf8;
use crate::native::backends::windows::util::to_wide;
use crate::native::{Errc, Error, Result};
use crate::platform::windowing::IClipboard;
use std::ptr;

struct ClipboardSession;

impl ClipboardSession {
    fn open(hwnd: *mut std::ffi::c_void) -> Option<Self> {
        // SAFETY: hwnd 为空时 Win32 允许以当前任务关联剪贴板；成功后由 Drop 成对关闭。
        (unsafe { OpenClipboard(hwnd) } != 0).then_some(Self)
    }
}

impl Drop for ClipboardSession {
    fn drop(&mut self) {
        // SAFETY: 仅在 OpenClipboard 成功后构造，且每个实例只关闭一次。
        let _ = unsafe { CloseClipboard() };
    }
}

struct OwnedGlobalMemory {
    handle: *mut std::ffi::c_void,
}

impl OwnedGlobalMemory {
    fn from_text(text: &str) -> Option<Self> {
        let wide = to_wide(text);
        let byte_len = wide.len().checked_mul(std::mem::size_of::<u16>())?;
        // SAFETY: 请求可移动且清零的 HGLOBAL；成功后由本类型负责释放或显式转移。
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, byte_len) };
        if handle.is_null() {
            return None;
        }
        let memory = Self { handle };
        // SAFETY: handle 由本函数刚分配，wide 的字节数不超过分配大小。
        let destination = unsafe { GlobalLock(handle) as *mut u16 };
        if destination.is_null() {
            return None;
        }
        unsafe {
            ptr::copy_nonoverlapping(wide.as_ptr(), destination, wide.len());
            let _ = GlobalUnlock(handle);
        }
        Some(memory)
    }

    fn handle(&self) -> *mut std::ffi::c_void {
        self.handle
    }

    fn transfer_to_clipboard(mut self) {
        self.handle = ptr::null_mut();
    }
}

impl Drop for OwnedGlobalMemory {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: 非空 handle 仍由本类型独占，尚未转移给系统剪贴板。
            let _ = unsafe { GlobalFree(self.handle) };
        }
    }
}

struct LockedGlobalMemory {
    handle: *mut std::ffi::c_void,
    data: *const u16,
    units: usize,
}

impl LockedGlobalMemory {
    fn lock(handle: *mut std::ffi::c_void) -> Option<Self> {
        if handle.is_null() {
            return None;
        }
        // SAFETY: handle 来自 GetClipboardData 或本模块分配；GlobalSize 给出可读上界。
        let byte_len = unsafe { GlobalSize(handle) };
        if byte_len < std::mem::size_of::<u16>() {
            return None;
        }
        let data = unsafe { GlobalLock(handle) as *const u16 };
        if data.is_null() {
            return None;
        }
        Some(Self {
            handle,
            data,
            units: byte_len / std::mem::size_of::<u16>(),
        })
    }

    fn text(&self) -> String {
        // SAFETY: data 在本对象存活期间保持锁定，units 由 GlobalSize 限定。
        let wide = unsafe { std::slice::from_raw_parts(self.data, self.units) };
        let len = wide
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(wide.len());
        to_utf8(&wide[..len])
    }
}

impl Drop for LockedGlobalMemory {
    fn drop(&mut self) {
        // SAFETY: handle 在构造时成功加锁，且每个实例只解锁一次。
        let _ = unsafe { GlobalUnlock(self.handle) };
    }
}

pub(crate) fn priority_result_has_text(result: i32) -> bool {
    result > 0
}

// 测试目标保留全局内存文本往返入口，供 Windows 剪贴板契约测试按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
pub(crate) fn global_memory_text_round_trip(text: &str) -> Option<String> {
    let memory = OwnedGlobalMemory::from_text(text)?;
    LockedGlobalMemory::lock(memory.handle()).map(|locked| locked.text())
}

// Windows 剪贴板所有者只在 crate 内部平台注册表中构造。
pub(crate) struct WindowsClipboard {
    hwnd: *mut std::ffi::c_void,
}

impl WindowsClipboard {
    // 创建未绑定窗口的剪贴板后端。
    pub(crate) fn new() -> Self {
        Self {
            hwnd: ptr::null_mut(),
        }
    }

    // 绑定当前平台窗口句柄。
    pub(crate) fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }
}

impl Default for WindowsClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl IClipboard for WindowsClipboard {
    fn text(&self) -> Result<String> {
        let Some(_session) = ClipboardSession::open(self.hwnd) else {
            return Err(clipboard_error("OpenClipboard"));
        };
        // SAFETY: 剪贴板会话保持打开，返回的 HGLOBAL 仅在会话内加锁读取。
        let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
        LockedGlobalMemory::lock(handle)
            .map(|memory| memory.text())
            .ok_or_else(|| clipboard_error("GetClipboardData"))
    }

    fn set_text(&mut self, text: &str) -> Result<()> {
        let Some(memory) = OwnedGlobalMemory::from_text(text) else {
            return Err(clipboard_error("GlobalAlloc"));
        };
        let Some(_session) = ClipboardSession::open(self.hwnd) else {
            return Err(clipboard_error("OpenClipboard"));
        };
        // SAFETY: 当前任务持有已打开的剪贴板；失败时不覆盖现有内容。
        if unsafe { EmptyClipboard() } == 0 {
            return Err(clipboard_error("EmptyClipboard"));
        }
        // SAFETY: HGLOBAL 已解锁且仍由 memory 独占；非空返回值将所有权转移给系统。
        if !unsafe { SetClipboardData(CF_UNICODETEXT, memory.handle()) }.is_null() {
            memory.transfer_to_clipboard();
            Ok(())
        } else {
            Err(clipboard_error("SetClipboardData"))
        }
    }

    fn has_text(&self) -> Result<bool> {
        let Some(_session) = ClipboardSession::open(self.hwnd) else {
            return Err(clipboard_error("OpenClipboard"));
        };
        let formats = [CF_UNICODETEXT, CF_TEXT];
        // SAFETY: formats 指向两个有效的剪贴板格式 ID，会话在调用期间保持打开。
        Ok(priority_result_has_text(unsafe {
            GetPriorityClipboardFormat(formats.as_ptr(), formats.len() as i32)
        }))
    }
}

fn clipboard_error(operation: &str) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("WindowsClipboard: {operation} failed"),
    )
}

// FFI declarations

const CF_UNICODETEXT: u32 = 13;
const CF_TEXT: u32 = 1;
const GMEM_MOVEABLE: u32 = 0x0002;
const GMEM_ZEROINIT: u32 = 0x0040;
