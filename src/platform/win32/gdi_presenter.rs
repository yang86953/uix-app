//! GdiPresenter — Win32 GDI DIB pixel presentation.
//!
//! Takes a `&[u32]` BGRA pixel buffer from a software renderer and presents it
//! to a Win32 window via `CreateDIBSection` + `BitBlt`.

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]
#![allow(nonstandard_style)]

use crate::diag::{Errc, Error};
use std::mem::MaybeUninit;

// ── Win32 FFI declarations ────────────────────────────────────────────────

#[link(name = "gdi32")]
extern "system" {
    fn CreateDIBSection(
        hdc: *mut std::ffi::c_void,
        pbmi: *const BITMAPINFO,
        usage: u32,
        ppvBits: *mut *mut std::ffi::c_void,
        hSection: *mut std::ffi::c_void,
        offset: u32,
    ) -> *mut std::ffi::c_void;
    fn SelectObject(hdc: *mut std::ffi::c_void, h: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn DeleteObject(h: *mut std::ffi::c_void) -> i32;
    fn DeleteDC(hdc: *mut std::ffi::c_void) -> i32;
    fn BitBlt(
        hdc_dst: *mut std::ffi::c_void,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        hdc_src: *mut std::ffi::c_void,
        sx: i32,
        sy: i32,
        rop: u32,
    ) -> i32;
    fn CreateCompatibleDC(hdc: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}

#[link(name = "user32")]
extern "system" {
    fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ReleaseDC(hwnd: *mut std::ffi::c_void, dc: *mut std::ffi::c_void) -> i32;
}

#[repr(C)]
struct BITMAPINFOHEADER {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_xpels_per_meter: i32,
    bi_ypels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[repr(C)]
struct BITMAPINFO {
    bmi_header: BITMAPINFOHEADER,
    bmi_colors: [u32; 0],
}

const BI_RGB: u32 = 0;
const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC0020;

// ════════════════════════════════════════════════════════════════════════════
// GdiPresenter
// ════════════════════════════════════════════════════════════════════════════

/// Presents a BGRA pixel buffer to a Win32 window using GDI DIB sections.
pub struct GdiPresenter {
    hwnd: *mut std::ffi::c_void,
    hdc_mem: *mut std::ffi::c_void,
    hbitmap: *mut std::ffi::c_void,
    dib_bits: *mut u32,
    width: i32,
    height: i32,
}

impl GdiPresenter {
    /// Create a new DIB-backed presenter for the given window and dimensions.
    ///
    /// # Safety
    ///
    /// `hwnd` must be a valid native window handle (non-null, owned by the caller).
    /// The caller must ensure the window is not destroyed during this object's lifetime.
    ///
    /// Returns `Err` if `w <= 0` or `h <= 0`, or if the DIB cannot be created.
    pub unsafe fn new(hwnd: *mut std::ffi::c_void, w: i32, h: i32) -> Result<Self, Error> {
        if w <= 0 || h <= 0 {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("GdiPresenter: dimensions must be positive, got {}x{}", w, h),
            ));
        }
        let mut dib_bits: *mut u32 = std::ptr::null_mut();
        let mut hdc_mem: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut hbitmap: *mut std::ffi::c_void = std::ptr::null_mut();
        unsafe {
            let hdc = GetDC(hwnd);
            if !hdc.is_null() {
                hdc_mem = CreateCompatibleDC(hdc);
                if !hdc_mem.is_null() {
                    let bmi = BITMAPINFO {
                        bmi_header: BITMAPINFOHEADER {
                            bi_size: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                            bi_width: w,
                            bi_height: -h,
                            bi_planes: 1,
                            bi_bit_count: 32,
                            bi_compression: BI_RGB,
                            bi_size_image: 0,
                            bi_xpels_per_meter: 0,
                            bi_ypels_per_meter: 0,
                            bi_clr_used: 0,
                            bi_clr_important: 0,
                        },
                        bmi_colors: [],
                    };
                    let mut ppv: MaybeUninit<*mut std::ffi::c_void> = MaybeUninit::uninit();
                    hbitmap = CreateDIBSection(
                        hdc_mem,
                        &bmi,
                        DIB_RGB_COLORS,
                        ppv.as_mut_ptr(),
                        std::ptr::null_mut(),
                        0,
                    );
                    dib_bits = ppv.assume_init() as *mut u32;
                    if !hbitmap.is_null() {
                        SelectObject(hdc_mem, hbitmap);
                    } else {
                        DeleteDC(hdc_mem);
                        hdc_mem = std::ptr::null_mut();
                    }
                }
                ReleaseDC(hwnd, hdc);
            }
        }
        if hdc_mem.is_null() || hbitmap.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "GdiPresenter: failed to create DIB section".to_string(),
            ));
        }
        Ok(Self {
            hwnd,
            hdc_mem,
            hbitmap,
            dib_bits,
            width: w,
            height: h,
        })
    }

    /// Copy pixel data to the DIB and blit to the window.
    pub fn present(&self, pixels: &[u32]) {
        unsafe {
            if self.dib_bits.is_null() {
                return;
            }
            let len = (self.width as usize)
                .checked_mul(self.height as usize)
                .map_or(0, |n| n.min(pixels.len()));
            if len == 0 {
                return;
            }
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), self.dib_bits, len);
            let hdc = GetDC(self.hwnd);
            if hdc.is_null() {
                return;
            }
            BitBlt(
                hdc,
                0,
                0,
                self.width,
                self.height,
                self.hdc_mem,
                0,
                0,
                SRCCOPY,
            );
            ReleaseDC(self.hwnd, hdc);
        }
    }
}

impl Drop for GdiPresenter {
    fn drop(&mut self) {
        unsafe {
            if !self.hbitmap.is_null() {
                DeleteObject(self.hbitmap);
            }
            if !self.hdc_mem.is_null() {
                DeleteDC(self.hdc_mem);
            }
        }
    }
}
