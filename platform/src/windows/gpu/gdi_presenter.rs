//! GdiPresenter — Windows GDI DIB pixel presentation.
//!
//! Takes a `&[u32]` ARGB pixel buffer from a software renderer and presents it
//! to a Windows window via `CreateDIBSection` + `BitBlt`.
//!
//! Uses memory DC (`CreateCompatibleDC` + `SelectObject`) + `BitBlt`, the
//! classic and reliable GDI pixel-pushing approach.

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]
#![allow(nonstandard_style)]

use crate::windows::ffi::{GetDC, ReleaseDC};
use crate::IPresenter;
use crate::{Errc, Error};

// ── Windows FFI declarations ────────────────────────────────────────────────

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
// DibHandle — own s a GDI DIB section + memory DC (RAII)
// ════════════════════════════════════════════════════════════════════════════

struct DibHandle {
    hbitmap: *mut std::ffi::c_void,
    hdc_mem: *mut std::ffi::c_void,
    bits: *mut u32,
}

impl DibHandle {
    unsafe fn new(hwnd: *mut std::ffi::c_void, w: i32, h: i32) -> Result<Self, Error> {
        unsafe {
            let hdc = GetDC(hwnd);
            if hdc.is_null() {
                return Err(Error::new(
                    Errc::PlatformError,
                    "GdiPresenter: GetDC failed".to_string(),
                ));
            }
            let hdc_mem = CreateCompatibleDC(hdc);
            if hdc_mem.is_null() {
                ReleaseDC(hwnd, hdc);
                return Err(Error::new(
                    Errc::PlatformError,
                    "GdiPresenter: CreateCompatibleDC failed".to_string(),
                ));
            }
            let bmi = BITMAPINFO {
                bmi_header: BITMAPINFOHEADER {
                    bi_size: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    bi_width: w,
                    bi_height: -h, // top-down
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
            let mut pbits: *mut std::ffi::c_void = std::ptr::null_mut();
            let hbitmap = CreateDIBSection(
                hdc_mem,
                &bmi,
                DIB_RGB_COLORS,
                &mut pbits,
                std::ptr::null_mut(),
                0,
            );
            ReleaseDC(hwnd, hdc);
            if hbitmap.is_null() || pbits.is_null() {
                DeleteDC(hdc_mem);
                if !hbitmap.is_null() {
                    DeleteObject(hbitmap);
                }
                return Err(Error::new(
                    Errc::PlatformError,
                    "GdiPresenter: CreateDIBSection failed".to_string(),
                ));
            }
            SelectObject(hdc_mem, hbitmap);
            Ok(Self {
                hbitmap,
                hdc_mem,
                bits: pbits as *mut u32,
            })
        }
    }
}

impl Drop for DibHandle {
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

// ════════════════════════════════════════════════════════════════════════════
// GdiPresenter
// ════════════════════════════════════════════════════════════════════════════

pub struct GdiPresenter {
    hwnd: *mut std::ffi::c_void,
    dib: DibHandle,
    width: i32,
    height: i32,
}

impl GdiPresenter {
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn new(hwnd: *mut std::ffi::c_void, w: i32, h: i32) -> Result<Self, Error> {
        if w <= 0 || h <= 0 {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("GdiPresenter: dimensions must be positive, got {}x{}", w, h),
            ));
        }
        let dib = unsafe { DibHandle::new(hwnd, w, h)? };
        Ok(Self {
            hwnd,
            dib,
            width: w,
            height: h,
        })
    }

    fn blit(&self) {
        unsafe {
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
                self.dib.hdc_mem,
                0,
                0,
                SRCCOPY,
            );
            ReleaseDC(self.hwnd, hdc);
        }
    }

    fn blit_rect(&self, x: i32, y: i32, w: i32, h: i32) {
        unsafe {
            let hdc = GetDC(self.hwnd);
            if hdc.is_null() {
                return;
            }
            BitBlt(hdc, x, y, w, h, self.dib.hdc_mem, x, y, SRCCOPY);
            ReleaseDC(self.hwnd, hdc);
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IPresenter 实现
// ════════════════════════════════════════════════════════════════════════════

impl IPresenter for GdiPresenter {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), Error> {
        // Auto-resize if dimensions changed
        if (width != self.width || height != self.height) && self.resize(width, height).is_err() {
            crate::log::debug_fn(format!(
                "GdiPresenter: resize to {}x{} failed, fallback to {}x{}",
                width, height, self.width, self.height
            ));
        }
        unsafe {
            if self.dib.bits.is_null() {
                return Err(Error::new(
                    Errc::PlatformError,
                    "GdiPresenter: DIB not initialized",
                ));
            }
            if let Some((dx, dy, dw, dh)) = dirty_rect {
                if dx >= 0
                    && dy >= 0
                    && dw > 0
                    && dh > 0
                    && dx + dw <= self.width
                    && dy + dh <= self.height
                {
                    let src_row_start = (dy * self.width + dx) as usize;
                    let dst_row_start = src_row_start;
                    let row_bytes = dw as usize * 4;
                    for row in 0..dh as usize {
                        let src_offset = src_row_start + row * self.width as usize;
                        let dst_offset = dst_row_start + row * self.width as usize;
                        let src = &pixels[src_offset..src_offset.saturating_add(row_bytes / 4)];
                        let dst = self.dib.bits.add(dst_offset);
                        let copy_len = src.len().min(row_bytes / 4);
                        if copy_len > 0 {
                            std::ptr::copy_nonoverlapping(src.as_ptr(), dst, copy_len);
                        }
                    }
                    self.blit_rect(dx, dy, dw, dh);
                    return Ok(());
                }
            }
            let len = (self.width as usize)
                .checked_mul(self.height as usize)
                .map_or(0, |n| n.min(pixels.len()));
            if len == 0 {
                return Ok(());
            }
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), self.dib.bits, len);
        }
        self.blit();
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        if width <= 0 || height <= 0 {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("GdiPresenter::resize: got {}x{}", width, height),
            ));
        }
        if width == self.width && height == self.height {
            return Ok(());
        }

        // Create new DIB first, then swap
        let new_dib = unsafe { DibHandle::new(self.hwnd, width, height)? };
        self.dib = new_dib;
        self.width = width;
        self.height = height;
        Ok(())
    }
}
