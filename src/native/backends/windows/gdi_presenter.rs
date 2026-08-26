//! GdiPresenter — Windows GDI DIB pixel presentation.
//!
//! 接收软件渲染器的 `&[u32]` ARGB 像素，并通过
//! `CreateDIBSection` + `StretchBlt` 提交到 Windows 窗口。
//!
//! 保留型 DIB 使用 logical pixels；每次提交再映射到当前 physical client extent。

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]
#![allow(nonstandard_style)]

use std::cell::Cell;

use crate::core::{PresentCoherency, PresentDamage, PresentSurface};
use crate::native::backends::windows::ffi::{GetDC, ReleaseDC};
use crate::native::backends::windows::util::windows_diag;
use crate::native::presentation::graphics::platform::windows::query_client_rect;
use crate::native::{Errc, Error};
use crate::platform::presentation::IPresenter;
// ── Windows FFI declarations ────────────────────────────────────────────────

#[link(name = "gdi32")]
// SAFETY: 声明对应 gdi32 ABI，调用方负责 HDC/HBITMAP 所有权、选择恢复顺序及像素输出槽的有效性。
unsafe extern "system" {
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
    fn StretchBlt(
        hdc_dst: *mut std::ffi::c_void,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        hdc_src: *mut std::ffi::c_void,
        sx: i32,
        sy: i32,
        sw: i32,
        sh: i32,
        rop: u32,
    ) -> i32;
    fn SaveDC(hdc: *mut std::ffi::c_void) -> i32;
    fn RestoreDC(hdc: *mut std::ffi::c_void, saved_dc: i32) -> i32;
    fn IntersectClipRect(
        hdc: *mut std::ffi::c_void,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    ) -> i32;
    fn SetStretchBltMode(hdc: *mut std::ffi::c_void, mode: i32) -> i32;
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
const COLORONCOLOR: i32 = 3;

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

/// 将 logical source damage 外扩映射到 physical target pixels。
#[allow(
    clippy::too_many_arguments,
    reason = "source and target extents are explicit at the GDI scaling boundary"
)]
pub(crate) fn scale_damage_rect_to_target(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    source_width: i32,
    source_height: i32,
    target_width: i32,
    target_height: i32,
) -> Option<(i32, i32, i32, i32)> {
    if target_width <= 0 || target_height <= 0 {
        return None;
    }
    let (x, y, width, height) = clip_damage_rect(x, y, width, height, source_width, source_height)?;
    let x0 = scale_floor(x, target_width, source_width);
    let y0 = scale_floor(y, target_height, source_height);
    let x1 = scale_ceil(x.saturating_add(width), target_width, source_width);
    let y1 = scale_ceil(y.saturating_add(height), target_height, source_height);
    Some((x0, y0, x1 - x0, y1 - y0))
}

fn scale_floor(value: i32, target: i32, source: i32) -> i32 {
    ((i64::from(value) * i64::from(target)) / i64::from(source.max(1))) as i32
}

fn scale_ceil(value: i32, target: i32, source: i32) -> i32 {
    let numerator = i64::from(value) * i64::from(target);
    let denominator = i64::from(source.max(1));
    ((numerator + denominator - 1) / denominator) as i32
}

fn query_target_extent(hwnd: *mut std::ffi::c_void) -> Option<(i32, i32)> {
    // SAFETY: query_client_rect 只在同步调用内写入栈上 RECT。
    let rect = unsafe { query_client_rect(hwnd) }?;
    let width = rect.right.saturating_sub(rect.left);
    let height = rect.bottom.saturating_sub(rect.top);
    (width > 0 && height > 0).then_some((width, height))
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
    /// 创建与窗口 DC 兼容的 DIB 段和内存 DC。
    ///
    /// # Safety
    /// 调用者必须保证 hwnd 为存活窗口句柄且 w/h 为正，调用期间窗口未被销毁。
    unsafe fn new(hwnd: *mut std::ffi::c_void, w: i32, h: i32) -> Result<Self, Error> {
        // SAFETY: hwnd 由调用方保证存活；hdc/hdc_mem/pbits 为输出句柄，失败路径逐一释放已创建对象避免泄漏。
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
        // SAFETY: 句柄经 null 检查且本对象独占所有权，只在此释放一次。
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

// GDI 呈现器只在 crate 内部呈现配方中构造。
pub(crate) struct GdiPresenter {
    hwnd: *mut std::ffi::c_void,
    dib: DibHandle,
    width: i32,
    height: i32,
    observed_target: Cell<Option<(i32, i32)>>,
    surface_generation: Cell<u64>,
}

impl GdiPresenter {
    /// 创建 GDI presenter；hwnd 由调用方（窗口会话）持有。
    ///
    /// # Safety
    /// 调用者必须保证 hwnd 存活且 w/h 为正，presenter 生命周期内窗口不被销毁。
    #[allow(clippy::missing_safety_doc)]
    // 由 crate 内部呈现配方在窗口句柄有效期内创建。
    pub(crate) unsafe fn new(hwnd: *mut std::ffi::c_void, w: i32, h: i32) -> Result<Self, Error> {
        if w <= 0 || h <= 0 {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("GdiPresenter: dimensions must be positive, got {}x{}", w, h),
            ));
        }
        // SAFETY: hwnd 已由本函数调用方保证存活；尺寸已验证为正。
        let dib = unsafe { DibHandle::new(hwnd, w, h)? };
        Ok(Self {
            hwnd,
            dib,
            width: w,
            height: h,
            observed_target: Cell::new(query_target_extent(hwnd)),
            surface_generation: Cell::new(1),
        })
    }

    fn target_extent(&self) -> Result<(i32, i32), Error> {
        query_target_extent(self.hwnd).ok_or_else(|| {
            windows_diag(
                Errc::PlatformError,
                "GdiPresenter: GetClientRect failed or returned an empty client",
            )
        })
    }

    fn blit(&self) -> Result<(), Error> {
        self.stretch_to_client(None)
    }

    fn blit_rect(&self, x: i32, y: i32, w: i32, h: i32) -> Result<(), Error> {
        self.stretch_to_client(Some((x, y, w, h)))
    }

    /// 在保存的 DC 状态内把完整 logical DIB 映射到 physical client；
    /// 局部提交只收窄 physical clip，因而与完整缩放使用同一采样原点。
    fn stretch_to_client(&self, logical_clip: Option<(i32, i32, i32, i32)>) -> Result<(), Error> {
        super::dpi::with_per_monitor_v2(|| self.stretch_to_client_in_dpi_scope(logical_clip))
    }

    fn stretch_to_client_in_dpi_scope(
        &self,
        logical_clip: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), Error> {
        let (target_width, target_height) = self.target_extent()?;
        let physical_clip = match logical_clip {
            Some((x, y, width, height)) => {
                let Some(rect) = scale_damage_rect_to_target(
                    x,
                    y,
                    width,
                    height,
                    self.width,
                    self.height,
                    target_width,
                    target_height,
                ) else {
                    return Ok(());
                };
                Some(rect)
            }
            None => None,
        };
        // SAFETY: hwnd 存活（本 presenter 持有）；hdc 为 GetDC 返回且在同一调用内 ReleaseDC 配对；dib.hdc_mem/hbitmap 为存活 GDI 对象；坐标已缩放且在目标范围内。
        unsafe {
            let hdc = GetDC(self.hwnd);
            if hdc.is_null() {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "GdiPresenter: GetDC failed during present",
                ));
            }

            let saved_dc = SaveDC(hdc);
            let mut failure = (saved_dc == 0)
                .then(|| windows_diag(Errc::PlatformError, "GdiPresenter: SaveDC failed"));
            if failure.is_none() && SetStretchBltMode(hdc, COLORONCOLOR) == 0 {
                failure = Some(windows_diag(
                    Errc::PlatformError,
                    "GdiPresenter: SetStretchBltMode failed",
                ));
            }
            if failure.is_none() {
                if let Some((x, y, width, height)) = physical_clip {
                    if IntersectClipRect(
                        hdc,
                        x,
                        y,
                        x.saturating_add(width),
                        y.saturating_add(height),
                    ) == 0
                    {
                        failure = Some(windows_diag(
                            Errc::PlatformError,
                            "GdiPresenter: IntersectClipRect failed",
                        ));
                    }
                }
            }
            if failure.is_none()
                && StretchBlt(
                    hdc,
                    0,
                    0,
                    target_width,
                    target_height,
                    self.dib.hdc_mem,
                    0,
                    0,
                    self.width,
                    self.height,
                    SRCCOPY,
                ) == 0
            {
                failure = Some(windows_diag(
                    Errc::PlatformError,
                    "GdiPresenter: StretchBlt failed",
                ));
            }
            if saved_dc != 0 && RestoreDC(hdc, saved_dc) == 0 && failure.is_none() {
                failure = Some(windows_diag(
                    Errc::PlatformError,
                    "GdiPresenter: RestoreDC failed",
                ));
            }
            if ReleaseDC(self.hwnd, hdc) == 0 && failure.is_none() {
                failure = Some(windows_diag(
                    Errc::PlatformError,
                    "GdiPresenter: ReleaseDC failed",
                ));
            }
            if let Some(error) = failure {
                return Err(error);
            }
        }
        Ok(())
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
        // SAFETY: 区域已按 DIB extent 裁剪；pixels 切片与 dib.bits 指向的有效内存按相同行宽布局，拷贝长度受切片边界约束。
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
    fn present_coherency(&self) -> PresentCoherency {
        // The DIB retains every successfully copied pixel across presents.
        PresentCoherency::RetainedBuffer
    }

    fn present_surface(
        &self,
        drawable_width: i32,
        drawable_height: i32,
        device_pixel_ratio: f32,
    ) -> PresentSurface {
        // Damage 仍以 logical DIB 为坐标面；physical client extent 只进入
        // generation，用于在跨 DPI 且 logical extent 不变时强制一次完整提交。
        let current_target = query_target_extent(self.hwnd);
        if self.observed_target.get() != current_target {
            self.observed_target.set(current_target);
            self.surface_generation
                .set(self.surface_generation.get().wrapping_add(1));
        }
        PresentSurface::identity(
            drawable_width,
            drawable_height,
            device_pixel_ratio,
            self.surface_generation.get(),
        )
    }

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
        // SAFETY: resize 后 width/height 与 DIB extent 匹配；pixels 长度已按 width×height 校验，拷贝不越界。
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
                            self.blit_rect(dx, dy, dw, dh)?;
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
        self.blit()
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
        // SAFETY: hwnd 由本 presenter 持有且存活；尺寸已验证为正。
        let new_dib = unsafe { DibHandle::new(self.hwnd, width, height)? };
        self.dib = new_dib;
        self.width = width;
        self.height = height;
        Ok(())
    }
}
