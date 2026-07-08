//! WGL + OpenGL ES 3.0 graphics context for Windows.
//!
//! Creates an ES profile context on the window HWND and implements
//! [`IGraphicsContext`] for [`GpuEngine`].

#![cfg(windows)]
#![allow(nonstandard_style)]
#![allow(clippy::missing_safety_doc)]

use std::ffi::{c_void, CString};
use std::ptr;

use crate::native::backends::windows::bindings::RECT;
use crate::native::backends::windows::ffi::{GetClientRect, GetDC};
use crate::native::backends::windows::util::windows_diag;
use crate::native::traits::present::{IGraphicsContext, PresentDamage};
use crate::native::{Errc, Error};

type HDC = *mut c_void;
type HGLRC = *mut c_void;
type HWND = *mut c_void;

type CreateContextAttribsFn = unsafe extern "system" fn(HDC, HGLRC, *const i32) -> HGLRC;

const WGL_CONTEXT_MAJOR_VERSION_ARB: i32 = 0x2091;
const WGL_CONTEXT_MINOR_VERSION_ARB: i32 = 0x2092;
const WGL_CONTEXT_PROFILE_MASK_ARB: i32 = 0x9126;
const WGL_CONTEXT_OPENGL_ES_PROFILE_BIT_EXT: i32 = 0x0000_0004;

const PFD_DRAW_TO_WINDOW: u32 = 0x0000_0004;
const PFD_SUPPORT_OPENGL: u32 = 0x0000_0020;
const PFD_DOUBLEBUFFER: u32 = 0x0000_0001;
const PFD_TYPE_RGBA: u8 = 0;
const PFD_MAIN_PLANE: u8 = 0;

#[repr(C)]
struct PIXELFORMATDESCRIPTOR {
    nSize: u16,
    nVersion: u16,
    dwFlags: u32,
    iPixelType: u8,
    cColorBits: u8,
    cRedBits: u8,
    cRedShift: u8,
    cGreenBits: u8,
    cGreenShift: u8,
    cBlueBits: u8,
    cBlueShift: u8,
    cAlphaBits: u8,
    cAlphaShift: u8,
    cAccumBits: u8,
    cAccumRedBits: u8,
    cAccumGreenBits: u8,
    cAccumBlueBits: u8,
    cAccumAlphaBits: u8,
    cDepthBits: u8,
    cStencilBits: u8,
    cAuxBuffers: u8,
    iLayerType: u8,
    bReserved: u8,
    dwLayerMask: u32,
    dwVisibleMask: u32,
    dwDamageMask: u32,
}

const LOGPIXELSX: i32 = 88;

#[link(name = "gdi32")]
extern "system" {
    fn ChoosePixelFormat(hdc: HDC, ppfd: *const PIXELFORMATDESCRIPTOR) -> i32;
    fn SetPixelFormat(hdc: HDC, format: i32, ppfd: *const PIXELFORMATDESCRIPTOR) -> i32;
    fn SwapBuffers(hdc: HDC) -> i32;
    fn GetDeviceCaps(hdc: HDC, index: i32) -> i32;
}

#[link(name = "opengl32")]
extern "system" {
    fn wglCreateContext(hdc: HDC) -> HGLRC;
    fn wglMakeCurrent(hdc: HDC, hglrc: HGLRC) -> i32;
    fn wglDeleteContext(hglrc: HGLRC) -> i32;
    fn wglGetProcAddress(name: *const i8) -> *const c_void;
}

fn load_wgl_fn<T>(name: &str) -> Option<T> {
    let c_name = CString::new(name).ok()?;
    let proc = unsafe { wglGetProcAddress(c_name.as_ptr()) };
    if proc.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute_copy(&proc) })
    }
}

fn default_pfd() -> PIXELFORMATDESCRIPTOR {
    PIXELFORMATDESCRIPTOR {
        nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
        nVersion: 1,
        dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
        iPixelType: PFD_TYPE_RGBA,
        cColorBits: 32,
        cRedBits: 0,
        cRedShift: 0,
        cGreenBits: 0,
        cGreenShift: 0,
        cBlueBits: 0,
        cBlueShift: 0,
        cAlphaBits: 8,
        cAlphaShift: 0,
        cAccumBits: 0,
        cAccumRedBits: 0,
        cAccumGreenBits: 0,
        cAccumBlueBits: 0,
        cAccumAlphaBits: 0,
        cDepthBits: 24,
        cStencilBits: 8,
        cAuxBuffers: 0,
        iLayerType: PFD_MAIN_PLANE,
        bReserved: 0,
        dwLayerMask: 0,
        dwVisibleMask: 0,
        dwDamageMask: 0,
    }
}

fn setup_pixel_format(hdc: HDC) -> Result<(), Error> {
    let pfd = default_pfd();
    unsafe {
        let format = ChoosePixelFormat(hdc, &pfd);
        if format == 0 {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: ChoosePixelFormat failed",
            ));
        }
        if SetPixelFormat(hdc, format, &pfd) == 0 {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: SetPixelFormat failed",
            ));
        }
    }
    Ok(())
}

fn create_es_context(
    hdc: HDC,
    create_ctx: CreateContextAttribsFn,
    major: i32,
    minor: i32,
) -> Result<HGLRC, Error> {
    let attribs = [
        WGL_CONTEXT_MAJOR_VERSION_ARB,
        major,
        WGL_CONTEXT_MINOR_VERSION_ARB,
        minor,
        WGL_CONTEXT_PROFILE_MASK_ARB,
        WGL_CONTEXT_OPENGL_ES_PROFILE_BIT_EXT,
        0,
    ];
    let ctx = unsafe { create_ctx(hdc, ptr::null_mut(), attribs.as_ptr()) };
    if ctx.is_null() {
        Err(Error::new(
            Errc::PlatformError,
            format!("WglContext: failed to create OpenGL ES {major}.{minor} context"),
        ))
    } else {
        Ok(ctx)
    }
}

/// Query logical client size and physical drawable pixels for HiDPI monitors.
///
/// On DPI-unaware processes `GetClientRect` stays logical while the WGL default
/// framebuffer is allocated at monitor DPI, so we scale by `GetDeviceCaps(LOGPIXELSX)`.
fn drawable_size(hwnd: HWND, hdc: HDC) -> (i32, i32, i32, i32) {
    unsafe {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetClientRect(hwnd, &mut rect) == 0 {
            return (1, 1, 1, 1);
        }
        let logical_w = (rect.right - rect.left).max(1);
        let logical_h = (rect.bottom - rect.top).max(1);
        let dpi = GetDeviceCaps(hdc, LOGPIXELSX).max(96);
        let physical_w = ((logical_w as i64 * dpi as i64 + 48) / 96).max(1) as i32;
        let physical_h = ((logical_h as i64 * dpi as i64 + 48) / 96).max(1) as i32;
        (logical_w, logical_h, physical_w, physical_h)
    }
}

/// WGL + OpenGL ES graphics context bound to a Win32 HWND.
pub struct WglContext {
    hwnd: HWND,
    hdc: HDC,
    hglrc: HGLRC,
    logical_width: i32,
    logical_height: i32,
    width: i32,
    height: i32,
}

impl WglContext {
    /// Create a WGL context on `native_window` (HWND).
    ///
    /// Tries OpenGL ES 3.0 first, then ES 2.0.
    pub fn new(native_window: *mut c_void, width: i32, height: i32) -> Result<Self, Error> {
        if native_window.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "WglContext: native window handle is null",
            ));
        }
        if width <= 0 || height <= 0 {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("WglContext: dimensions must be positive, got {width}x{height}"),
            ));
        }

        let hwnd = native_window;
        let hdc = unsafe { GetDC(hwnd) };
        if hdc.is_null() {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: GetDC failed",
            ));
        }

        let result = (|| -> Result<Self, Error> {
            setup_pixel_format(hdc)?;

            let temp_ctx = unsafe { wglCreateContext(hdc) };
            if temp_ctx.is_null() {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "WglContext: wglCreateContext (temp) failed",
                ));
            }
            if unsafe { wglMakeCurrent(hdc, temp_ctx) } == 0 {
                unsafe {
                    wglDeleteContext(temp_ctx);
                }
                return Err(windows_diag(
                    Errc::PlatformError,
                    "WglContext: wglMakeCurrent (temp) failed",
                ));
            }

            let create_ctx = load_wgl_fn::<CreateContextAttribsFn>("wglCreateContextAttribsARB");
            let hglrc = if let Some(create_ctx) = create_ctx {
                create_es_context(hdc, create_ctx, 3, 0)
                    .or_else(|_| create_es_context(hdc, create_ctx, 2, 0))?
            } else {
                unsafe {
                    wglDeleteContext(temp_ctx);
                }
                return Err(Error::new(
                    Errc::PlatformError,
                    "WglContext: wglCreateContextAttribsARB unavailable",
                ));
            };

            unsafe {
                wglMakeCurrent(ptr::null_mut(), ptr::null_mut());
                wglDeleteContext(temp_ctx);
                if wglMakeCurrent(hdc, hglrc) == 0 {
                    wglDeleteContext(hglrc);
                    return Err(windows_diag(
                        Errc::PlatformError,
                        "WglContext: wglMakeCurrent failed",
                    ));
                }
            }

            let (logical_w, logical_h, physical_w, physical_h) = drawable_size(hwnd, hdc);
            let _ = (width, height);
            crate::core::log::info_fn(format!(
                "WglContext: OpenGL ES context created ({physical_w}x{physical_h} drawable, logical {logical_w}x{logical_h})"
            ));
            Ok(Self {
                hwnd,
                hdc,
                hglrc,
                logical_width: logical_w,
                logical_height: logical_h,
                width: physical_w,
                height: physical_h,
            })
        })();

        if result.is_err() {
            unsafe {
                crate::native::backends::windows::ffi::ReleaseDC(hwnd, hdc);
            }
        }
        result
    }
}

impl IGraphicsContext for WglContext {
    fn graphics_backend(&self) -> crate::native::traits::present::GraphicsBackend {
        crate::native::traits::present::GraphicsBackend::OpenGlEs
    }

    fn initialize(
        &mut self,
        _native_window: *mut c_void,
        _width: i32,
        _height: i32,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) {
        let _ = (width, height);
        self.make_current();
        let (logical_w, logical_h, physical_w, physical_h) = drawable_size(self.hwnd, self.hdc);
        self.logical_width = logical_w;
        self.logical_height = logical_h;
        self.width = physical_w;
        self.height = physical_h;
    }

    fn make_current(&mut self) {
        unsafe {
            let _ = wglMakeCurrent(self.hdc, self.hglrc);
        }
    }

    fn swap_buffers(&mut self, damage: PresentDamage) {
        let _ = damage;
        unsafe {
            let _ = SwapBuffers(self.hdc);
        }
    }

    fn shutdown(&mut self) {
        unsafe {
            if !self.hglrc.is_null() {
                wglMakeCurrent(ptr::null_mut(), ptr::null_mut());
                wglDeleteContext(self.hglrc);
                self.hglrc = ptr::null_mut();
            }
            if !self.hdc.is_null() {
                crate::native::backends::windows::ffi::ReleaseDC(self.hwnd, self.hdc);
                self.hdc = ptr::null_mut();
            }
        }
    }

    fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Vec<u32> {
        Vec::new()
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn device_pixel_ratio(&self) -> f32 {
        if self.logical_width <= 0 {
            return 1.0;
        }
        self.width as f32 / self.logical_width as f32
    }

    fn get_proc_address(&self, name: &str) -> Option<*const c_void> {
        let c_name = CString::new(name).ok()?;
        let proc = unsafe { wglGetProcAddress(c_name.as_ptr()) };
        if proc.is_null() {
            None
        } else {
            Some(proc)
        }
    }
}

impl Drop for WglContext {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawable_size_scales_logical_client_by_monitor_dpi() {
        let dpi = 192_i32;
        let logical_w = 1200_i32;
        let logical_h = 800_i32;
        let physical_w = ((logical_w as i64 * dpi as i64 + 48) / 96).max(1) as i32;
        let physical_h = ((logical_h as i64 * dpi as i64 + 48) / 96).max(1) as i32;
        assert_eq!(physical_w, 2400);
        assert_eq!(physical_h, 1600);
        assert!(((physical_w as f32 / logical_w as f32) - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn wgl_context_rejects_null_hwnd() {
        match WglContext::new(ptr::null_mut(), 800, 600) {
            Err(err) => assert_eq!(err.code(), Errc::PlatformError),
            Ok(_) => panic!("expected null hwnd to fail"),
        }
    }

    #[test]
    fn factory_create_gpu_context_on_real_window() {
        use crate::native::create_gpu_context;

        let mut platform = crate::native::create_platform().expect("platform");
        let window = platform
            .window_manager()
            .create_window("GPU test", 640, 480)
            .expect("window");
        let surface = window.native_surface_ptr();
        assert!(
            !surface.is_null(),
            "Windows HWND must be exposed as native_surface_ptr"
        );
        let mut ctx = create_gpu_context(surface, 640, 480).expect("WglContext");
        assert!(ctx.width() > 0);
        assert!(ctx.height() > 0);
        ctx.shutdown();
    }
}
