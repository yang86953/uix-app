#![cfg(windows)]
use super::bindings::{CANDIDATEFORM, COMPOSITIONFORM, POINT, RECT};
use super::consts::{GCS_COMPSTR, GCS_RESULTSTR};
use super::ffi::*;

// ============================================================================
// native/backends/windows/text_input.rs — Windows IME text input (ITextInput)
//
// IMM32：关联上下文 + caret/candidate 窗；composition 字符串由 wnd_proc 读取后
// 经 `ime_dispatch` 合成 UiEvent。TSF：`tsf_session` 在 start/stop 做 AssociateFocus，
// 并挂最小 `ITextStoreACP` + `ITfContextOwnerCompositionSink`（composition → 共享队列）。
// ============================================================================

use crate::core::{Errc, Error, Rect, Result, WindowId};
use crate::diagnostics::PendingFailureSource;
use crate::native::backends::windows::tsf_session::{TsfActivateParams, TsfSession};
use crate::native::backends::windows::util::windows_diag;
use crate::platform::windowing::ITextInput;
use crate::platform::windowing::event::UiEvent;
use std::collections::VecDeque;
use std::ptr;
use std::sync::{Arc, Mutex};

const IACE_DEFAULT: u32 = 0x0010;
const CFS_POINT: u32 = 0x0002;
const CFS_EXCLUDE: u32 = 0x0080;
const IMM_ERROR_NODATA: i32 = -1;
const IMM_ERROR_GENERAL: i32 = -2;

#[derive(Debug, Default)]
pub(crate) struct WindowsImeState {
    composition_active: bool,
    pending_high_surrogate: Option<u16>,
}

impl WindowsImeState {
    pub(crate) fn begin_composition(&mut self) -> bool {
        if self.composition_active {
            false
        } else {
            self.composition_active = true;
            true
        }
    }

    pub(crate) fn composition_active(&self) -> bool {
        self.composition_active
    }

    pub(crate) fn end_composition(&mut self) -> bool {
        std::mem::take(&mut self.composition_active)
    }

    pub(crate) fn reset_text_decoder(&mut self) {
        self.pending_high_surrogate = None;
    }

    pub(crate) fn decode_utf16_unit(&mut self, unit: u16) -> Option<String> {
        match unit {
            0xD800..=0xDBFF => {
                let replaced_invalid = self.pending_high_surrogate.replace(unit).is_some();
                replaced_invalid.then(|| String::from(char::REPLACEMENT_CHARACTER))
            }
            0xDC00..=0xDFFF => {
                if let Some(high) = self.pending_high_surrogate.take() {
                    Some(String::from_utf16_lossy(&[high, unit]))
                } else {
                    Some(String::from_utf16_lossy(&[unit]))
                }
            }
            _ => {
                if let Some(high) = self.pending_high_surrogate.take() {
                    Some(String::from_utf16_lossy(&[high, unit]))
                } else {
                    char::from_u32(u32::from(unit)).map(String::from)
                }
            }
        }
    }
}

pub(crate) fn read_ime_string(hwnd: *mut std::ffi::c_void, index: u32) -> Result<Option<String>> {
    if hwnd.is_null() {
        return Err(Error::new(Errc::InvalidArgument, "Windows IME: null HWND"));
    }

    // SAFETY: HWND belongs to the current platform window; HIMC is released
    // before returning and the buffer remains valid for the synchronous call.
    unsafe {
        let himc = ImmGetContext(hwnd);
        if himc.is_null() {
            return Err(windows_diag(
                Errc::PlatformError,
                "Windows IME: ImmGetContext failed",
            ));
        }

        let result = (|| {
            let byte_len = ImmGetCompositionStringW(himc, index, ptr::null_mut(), 0);
            if matches!(byte_len, IMM_ERROR_NODATA) {
                return Ok(None);
            }
            if byte_len == IMM_ERROR_GENERAL || byte_len < 0 {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "Windows IME: composition length query failed",
                ));
            }
            if byte_len == 0 {
                return Ok(Some(String::new()));
            }
            if byte_len % 2 != 0 {
                return Err(Error::new(
                    Errc::FormatError,
                    "Windows IME: UTF-16 byte length is not even",
                ));
            }

            let mut utf16 = vec![0_u16; byte_len as usize / 2];
            let copied =
                ImmGetCompositionStringW(himc, index, utf16.as_mut_ptr().cast(), byte_len as u32);
            if copied < 0 || copied > byte_len || copied % 2 != 0 {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "Windows IME: composition read failed",
                ));
            }
            utf16.truncate(copied as usize / 2);
            Ok(Some(String::from_utf16_lossy(&utf16)))
        })();
        let _ = ImmReleaseContext(hwnd, himc);
        result
    }
}

pub(crate) fn composition_string(hwnd: *mut std::ffi::c_void) -> Result<Option<String>> {
    read_ime_string(hwnd, GCS_COMPSTR)
}

pub(crate) fn result_string(hwnd: *mut std::ffi::c_void) -> Result<Option<String>> {
    read_ime_string(hwnd, GCS_RESULTSTR)
}

// Windows 文本输入后端只在 crate 内部平台注册表中构造。
pub(crate) struct WindowsTextInput {
    hwnd: *mut std::ffi::c_void,
    window_id: Option<WindowId>,
    events: Arc<Mutex<VecDeque<UiEvent>>>,
    pending_failures: PendingFailureSource,
    tsf: Option<TsfSession>,
}

impl WindowsTextInput {
    // 创建连接事件队列与失败源的文本输入后端。
    pub(crate) fn new(
        events: Arc<Mutex<VecDeque<UiEvent>>>,
        pending_failures: PendingFailureSource,
    ) -> Self {
        Self {
            hwnd: ptr::null_mut(),
            window_id: None,
            events,
            pending_failures,
            tsf: None,
        }
    }
    pub(crate) fn clear_target_window(&mut self, window_id: WindowId) {
        if self.window_id == Some(window_id) {
            self.deactivate_tsf();
            self.window_id = None;
            self.hwnd = ptr::null_mut();
        }
    }

    pub(crate) fn tsf_session_active_for(&self, window_id: WindowId) -> bool {
        self.tsf.is_some() && self.window_id == Some(window_id)
    }

    // 测试目标保留 TSF client id 观测入口，供输入会话契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tsf_client_id(&self) -> Option<u32> {
        self.tsf.as_ref().map(TsfSession::client_id)
    }

    fn require_hwnd(&self, operation: &str) -> Result<*mut std::ffi::c_void> {
        if self.hwnd.is_null() {
            Err(Error::new(
                Errc::InvalidOperation,
                format!("Windows text input: {operation} requires an active window"),
            ))
        } else {
            Ok(self.hwnd)
        }
    }

    fn dpi_scale(hwnd: *mut std::ffi::c_void) -> f32 {
        super::dpi::dpi_for_window(hwnd) as f32 / super::dpi::BASE_DPI as f32
    }

    fn activate_tsf(&mut self, hwnd: *mut std::ffi::c_void) {
        if self.tsf.is_some() {
            return;
        }
        let Some(window_id) = self.window_id else {
            let _ = self.pending_failures.enqueue(Error::new(
                Errc::InvalidState,
                "Windows text input: TSF skipped because the target has no window_id",
            ));
            return;
        };
        match TsfSession::activate(TsfActivateParams {
            hwnd,
            window_id,
            events: Arc::clone(&self.events),
            pending_failures: self.pending_failures.clone(),
        }) {
            Ok(session) => self.tsf = Some(session),
            Err(err) => {
                let _ = self.pending_failures.enqueue(err);
            }
        }
    }

    fn deactivate_tsf(&mut self) {
        if let Some(session) = self.tsf.take() {
            session.deactivate();
        }
    }
}

impl ITextInput for WindowsTextInput {
    fn set_target_window(
        &mut self,
        window_id: WindowId,
        native_window: *mut std::ffi::c_void,
    ) -> Result<()> {
        if native_window.is_null() {
            return Err(Error::new(
                Errc::InvalidOperation,
                "Windows text input: target window is null",
            ));
        }
        if self.window_id != Some(window_id) || self.hwnd != native_window {
            self.deactivate_tsf();
            self.window_id = Some(window_id);
            self.hwnd = native_window;
        }
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        let hwnd = self.require_hwnd("start")?;
        // SAFETY: HWND is the selected live platform window.
        if unsafe { ImmAssociateContextEx(hwnd, ptr::null_mut(), IACE_DEFAULT) } == 0 {
            return Err(windows_diag(
                Errc::PlatformError,
                "Windows text input: ImmAssociateContextEx(start) failed",
            ));
        }
        self.activate_tsf(hwnd);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.deactivate_tsf();
        if self.hwnd.is_null() {
            return Ok(());
        }
        // SAFETY: HWND is the selected live platform window.
        if unsafe { ImmAssociateContextEx(self.hwnd, ptr::null_mut(), 0) } == 0 {
            Err(windows_diag(
                Errc::PlatformError,
                "Windows text input: ImmAssociateContextEx(stop) failed",
            ))
        } else {
            Ok(())
        }
    }

    fn set_cursor_rect(&mut self, rect: Rect) -> Result<()> {
        let hwnd = self.require_hwnd("set_cursor_rect")?;
        if ![rect.x, rect.y, rect.w, rect.h]
            .iter()
            .all(|value| value.is_finite())
            || rect.w < 0.0
            || rect.h < 0.0
        {
            return Err(Error::new(
                Errc::InvalidArgument,
                "Windows text input: cursor rect must be finite and non-negative",
            ));
        }

        let scale = Self::dpi_scale(hwnd);
        let left = (rect.x * scale).round() as i32;
        let top = (rect.y * scale).round() as i32;
        let right = ((rect.x + rect.w) * scale).round() as i32;
        let bottom = ((rect.y + rect.h) * scale).round() as i32;
        let point = POINT { x: left, y: bottom };
        let area = RECT {
            left,
            top,
            right,
            bottom,
        };
        let composition = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: point,
            rcArea: area,
        };
        let candidate = CANDIDATEFORM {
            dwIndex: 0,
            dwStyle: CFS_EXCLUDE,
            ptCurrentPos: point,
            rcArea: area,
        };

        // SAFETY: structures have the Win32 ABI layout and remain alive for
        // both synchronous IMM calls. HIMC is released before returning.
        unsafe {
            let himc = ImmGetContext(hwnd);
            if himc.is_null() {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "Windows text input: ImmGetContext failed",
                ));
            }
            let composition_ok = ImmSetCompositionWindow(himc, &composition) != 0;
            let candidate_ok = ImmSetCandidateWindow(himc, &candidate) != 0;
            let _ = ImmReleaseContext(hwnd, himc);
            if composition_ok && candidate_ok {
                if let Some(tsf) = self.tsf.as_ref() {
                    tsf.set_cursor_rect(windows::Win32::Foundation::RECT {
                        left,
                        top,
                        right,
                        bottom,
                    })?;
                }
                Ok(())
            } else {
                Err(windows_diag(
                    Errc::PlatformError,
                    "Windows text input: cursor rect update failed",
                ))
            }
        }
    }
}
