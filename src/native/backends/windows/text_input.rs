#![cfg(windows)]
use super::bindings::{CANDIDATEFORM, COMPOSITIONFORM, POINT, RECT};
use super::consts::{GCS_COMPSTR, GCS_RESULTSTR};
use super::ffi::*;

// ============================================================================
// native/backends/windows/text_input.rs — Windows IME text input (ITextInput)
//
// IMM32：关联上下文 + caret/candidate 窗；composition 字符串由 wnd_proc 读取后
// 经 `ime_dispatch` 合成 UiEvent。TSF：`tsf_session` 在 start/stop 做 AssociateFocus；
// `ITfTextEditSink` / TextStore 仍待（`tsf_composition_events` 已预留事件形）。
// ============================================================================

use crate::core::{Errc, Error, Rect, Result};
use crate::native::backends::windows::tsf_session::TsfSession;
use crate::native::backends::windows::util::windows_diag;
use crate::native::traits::input::ITextInput;
use std::ptr;

const IACE_DEFAULT: u32 = 0x0010;
const CFS_POINT: u32 = 0x0002;
const CFS_EXCLUDE: u32 = 0x0080;
const LOGPIXELSX: i32 = 88;
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

pub struct WindowsTextInput {
    hwnd: *mut std::ffi::c_void,
    tsf: Option<TsfSession>,
}

impl WindowsTextInput {
    pub fn new() -> Self {
        Self {
            hwnd: ptr::null_mut(),
            tsf: None,
        }
    }
    pub fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }

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
        // SAFETY: HWND is validated by require_hwnd. GetDC/ReleaseDC are paired.
        unsafe {
            let hdc = GetDC(hwnd);
            if hdc.is_null() {
                return 1.0;
            }
            let dpi = GetDeviceCaps(hdc, LOGPIXELSX);
            let _ = ReleaseDC(hwnd, hdc);
            if dpi > 0 {
                dpi as f32 / 96.0
            } else {
                1.0
            }
        }
    }

    fn activate_tsf(&mut self, hwnd: *mut std::ffi::c_void) {
        if self.tsf.is_some() {
            return;
        }
        match TsfSession::activate(hwnd) {
            Ok(session) => self.tsf = Some(session),
            Err(err) => {
                // IMM32 仍可用；TSF 失败不阻断输入启动。
                crate::core::log::warn_fn(err.short_what());
            }
        }
    }

    fn deactivate_tsf(&mut self) {
        if let Some(session) = self.tsf.take() {
            session.deactivate();
        }
    }
}

impl Default for WindowsTextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl ITextInput for WindowsTextInput {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_surrogate_pair_is_emitted_as_one_scalar() {
        let mut state = WindowsImeState::default();
        assert_eq!(state.decode_utf16_unit(0xD83D), None);
        assert_eq!(state.decode_utf16_unit(0xDE00).as_deref(), Some("😀"));
    }

    #[test]
    fn invalid_surrogate_does_not_poison_following_text() {
        let mut state = WindowsImeState::default();
        assert_eq!(state.decode_utf16_unit(0xD83D), None);
        assert_eq!(
            state.decode_utf16_unit(u16::from(b'A')).as_deref(),
            Some("�A")
        );
        assert_eq!(
            state.decode_utf16_unit(u16::from(b'B')).as_deref(),
            Some("B")
        );
    }

    #[test]
    fn composition_state_transitions_are_idempotent() {
        let mut state = WindowsImeState::default();
        assert!(state.begin_composition());
        assert!(!state.begin_composition());
        assert!(state.end_composition());
        assert!(!state.end_composition());
    }

    #[test]
    fn start_without_selected_window_reports_error() {
        let mut input = WindowsTextInput::new();
        let err = input.start().expect_err("start without HWND must fail");
        assert_eq!(err.code(), Errc::InvalidOperation);
    }

    #[test]
    fn start_on_real_window_activates_tsf_session() {
        if std::env::consts::OS != "windows" {
            return;
        }
        let mut platform = crate::native::create_platform().expect("platform");
        let mut window = platform
            .window_manager()
            .create_window("TSF text input start", 320, 240)
            .expect("window");
        let hwnd = window.native_surface_ptr();
        let mut input = WindowsTextInput::new();
        input.set_hwnd(hwnd);
        input.start().expect("start");
        assert!(
            input.tsf_client_id().unwrap_or(0) != 0,
            "start must activate TSF AssociateFocus session"
        );
        input.stop().expect("stop");
        assert!(input.tsf_client_id().is_none());
        window.close().expect("close");
    }
}
