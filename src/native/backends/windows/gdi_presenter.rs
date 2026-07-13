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

use crate::native::backends::windows::ffi::{GetDC, ReleaseDC};
use crate::native::backends::windows::util::windows_diag;
use crate::native::traits::present::IPresenter;
use crate::native::traits::present::PresentDamage;
use crate::native::{Errc, Error};
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

pub(crate) fn clip_damage_rect(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    surface_width: i32,
    surface_height: i32,
) -> Option<(i32, i32, i32, i32)> {
    if width <= 0 || height <= 0 || surface_width <= 0 || surface_height <= 0 {
        return None;
    }
    let surface_width = i64::from(surface_width);
    let surface_height = i64::from(surface_height);
    let x0 = i64::from(x).clamp(0, surface_width);
    let y0 = i64::from(y).clamp(0, surface_height);
    let x1 = (i64::from(x) + i64::from(width)).clamp(0, surface_width);
    let y1 = (i64::from(y) + i64::from(height)).clamp(0, surface_height);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some((x0 as i32, y0 as i32, (x1 - x0) as i32, (y1 - y0) as i32))
}

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
                return Err(windows_diag(
                    Errc::PlatformError,
                    "GdiPresenter: GetDC failed",
                ));
            }
            let hdc_mem = CreateCompatibleDC(hdc);
            if hdc_mem.is_null() {
                let err = windows_diag(
                    Errc::PlatformError,
                    "GdiPresenter: CreateCompatibleDC failed",
                );
                ReleaseDC(hwnd, hdc);
                return Err(err);
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
            let dib_err = (hbitmap.is_null() || pbits.is_null()).then(|| {
                windows_diag(Errc::PlatformError, "GdiPresenter: CreateDIBSection failed")
            });
            ReleaseDC(hwnd, hdc);
            if let Some(err) = dib_err {
                DeleteDC(hdc_mem);
                if !hbitmap.is_null() {
                    DeleteObject(hbitmap);
                }
                return Err(err);
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

    /// 将像素缓冲中的局部区域复制到 DIB。
    fn copy_partial_rect(
        &self,
        pixels: &[u32],
        dx: i32,
        dy: i32,
        dw: i32,
        dh: i32,
    ) -> Option<(i32, i32, i32, i32)> {
        let (dx, dy, dw, dh) = clip_damage_rect(dx, dy, dw, dh, self.width, self.height)?;
        unsafe {
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
        }
        Some((dx, dy, dw, dh))
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
        damage: PresentDamage,
    ) -> Result<(), Error> {
        // A mismatched DIB cannot safely represent this frame. Propagate the
        // resize failure so the window session retains dirty state instead of
        // copying the new payload through the old extent.
        if width != self.width || height != self.height {
            self.resize(width, height)?;
        }
        unsafe {
            if self.dib.bits.is_null() {
                return Err(Error::new(
                    Errc::PlatformError,
                    "GdiPresenter: DIB not initialized",
                ));
            }
            match damage {
                PresentDamage::Partial(ref rects) if !rects.is_empty() => {
                    for &(dx, dy, dw, dh) in rects {
                        if let Some((dx, dy, dw, dh)) =
                            self.copy_partial_rect(pixels, dx, dy, dw, dh)
                        {
                            self.blit_rect(dx, dy, dw, dh);
                        }
                    }
                    return Ok(());
                }
                PresentDamage::Full | PresentDamage::Partial(_) => {}
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

