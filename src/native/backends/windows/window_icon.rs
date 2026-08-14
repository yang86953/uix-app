#![cfg(windows)]

use std::ptr::NonNull;

use crate::native::{Errc, Result};

use super::consts::{
    ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_LOADFROMFILE, SM_CXICON, SM_CXSMICON, SM_CYICON,
    SM_CYSMICON, WM_SETICON,
};
use super::ffi::{DestroyIcon, GetSystemMetrics, IsWindow, LoadImageW, SendMessageW};
use super::util::{to_wide, windows_diag};

pub(crate) struct WindowIconState {
    hwnd: *mut std::ffi::c_void,
    large: Option<NonNull<std::ffi::c_void>>,
    small: Option<NonNull<std::ffi::c_void>>,
}

impl WindowIconState {
    pub(crate) fn new(hwnd: *mut std::ffi::c_void) -> Self {
        Self {
            hwnd,
            large: None,
            small: None,
        }
    }

    pub(crate) fn set_from_file(&mut self, path: &str) -> Result<()> {
        let path = to_wide(path);
        let large = load_icon(
            &path,
            metric_or_default(SM_CXICON, 32),
            metric_or_default(SM_CYICON, 32),
            "large",
        )?;
        let small = match load_icon(
            &path,
            metric_or_default(SM_CXSMICON, 16),
            metric_or_default(SM_CYSMICON, 16),
            "small",
        ) {
            Ok(icon) => icon,
            Err(error) => {
                destroy_icon(large);
                return Err(error);
            }
        };

        // SAFETY: HWND is checked by the caller and both HICON values stay owned by
        // this state until after the window has stopped referencing them.
        unsafe {
            SendMessageW(self.hwnd, WM_SETICON, ICON_BIG, large.as_ptr() as isize);
            SendMessageW(self.hwnd, WM_SETICON, ICON_SMALL, small.as_ptr() as isize);
        }
        let old_large = self.large.replace(large);
        let old_small = self.small.replace(small);
        if let Some(icon) = old_large {
            destroy_icon(icon);
        }
        if let Some(icon) = old_small {
            destroy_icon(icon);
        }
        Ok(())
    }
}

impl Drop for WindowIconState {
    fn drop(&mut self) {
        // SAFETY: IsWindow guards the synchronous messages. Null lParam removes
        // the references before UIX-owned icon handles are destroyed.
        if !self.hwnd.is_null() && unsafe { IsWindow(self.hwnd) } != 0 {
            unsafe {
                SendMessageW(self.hwnd, WM_SETICON, ICON_BIG, 0);
                SendMessageW(self.hwnd, WM_SETICON, ICON_SMALL, 0);
            }
        }
        if let Some(icon) = self.large.take() {
            destroy_icon(icon);
        }
        if let Some(icon) = self.small.take() {
            destroy_icon(icon);
        }
    }
}

fn metric_or_default(metric: i32, fallback: i32) -> i32 {
    // SAFETY: GetSystemMetrics accepts the documented icon metric constants.
    let value = unsafe { GetSystemMetrics(metric) };
    if value > 0 { value } else { fallback }
}

fn load_icon(
    path: &[u16],
    width: i32,
    height: i32,
    role: &str,
) -> Result<NonNull<std::ffi::c_void>> {
    // SAFETY: path is NUL-terminated and valid for the duration of the call;
    // LR_LOADFROMFILE transfers ownership of the returned HICON to the caller.
    let handle = unsafe {
        LoadImageW(
            std::ptr::null_mut(),
            path.as_ptr(),
            IMAGE_ICON,
            width,
            height,
            LR_LOADFROMFILE,
        )
    };
    NonNull::new(handle).ok_or_else(|| {
        windows_diag(
            Errc::PlatformError,
            &format!("os_set_icon: LoadImageW failed for {role} icon"),
        )
    })
}

fn destroy_icon(icon: NonNull<std::ffi::c_void>) {
    // SAFETY: handles stored here were returned by LoadImageW without LR_SHARED
    // and are destroyed exactly once.
    let _ = unsafe { DestroyIcon(icon.as_ptr()) };
}
