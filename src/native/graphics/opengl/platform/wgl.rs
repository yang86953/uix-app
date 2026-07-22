//! WGL + OpenGL ES 3.0 graphics context for Windows.
//!
//! Creates an ES profile context on the window HWND and implements
//! [`IGraphicsContext`] for [`GpuEngine`].

#![allow(nonstandard_style)]
#![allow(clippy::missing_safety_doc)]
#![allow(
    clippy::upper_case_acronyms,
    reason = "these private declarations intentionally mirror the Windows ABI spellings"
)]

use std::ffi::{c_void, CStr, CString};
use std::ptr;

use crate::native::backends::windows::util::windows_diag;
use crate::native::graphics::opengl::raster::OpenGlRasterPipeline;
use crate::native::graphics::platform::windows::{
    device_context, drawable_size_from_hdc, release_device_context, release_device_context_checked,
};
use crate::native::traits::present::{
    GpuGlyphBlit, GpuSolidRect, IGraphicsContext, NativeRasterCaps, OffscreenTargetId,
    PresentCoherency, PresentDamage, PresentFrame, SoftFallbackTile,
};
use crate::native::{Errc, Error};

type HDC = *mut c_void;
type HGLRC = *mut c_void;
type HWND = *mut c_void;

type CreateContextAttribsFn = unsafe extern "system" fn(HDC, HGLRC, *const i32) -> HGLRC;
type ChoosePixelFormatArbFn =
    unsafe extern "system" fn(HDC, *const i32, *const f32, u32, *mut i32, *mut u32) -> i32;

const WGL_CONTEXT_MAJOR_VERSION_ARB: i32 = 0x2091;
const WGL_CONTEXT_MINOR_VERSION_ARB: i32 = 0x2092;
const WGL_CONTEXT_PROFILE_MASK_ARB: i32 = 0x9126;
const WGL_CONTEXT_OPENGL_ES_PROFILE_BIT_EXT: i32 = 0x0000_0004;
const WGL_DRAW_TO_WINDOW_ARB: i32 = 0x2001;
const WGL_SUPPORT_OPENGL_ARB: i32 = 0x2010;
const WGL_DOUBLE_BUFFER_ARB: i32 = 0x2011;
const WGL_PIXEL_TYPE_ARB: i32 = 0x2013;
const WGL_COLOR_BITS_ARB: i32 = 0x2014;
const WGL_TYPE_RGBA_ARB: i32 = 0x202B;

const PFD_DRAW_TO_WINDOW: u32 = 0x0000_0004;
const PFD_SUPPORT_OPENGL: u32 = 0x0000_0020;
const PFD_DOUBLEBUFFER: u32 = 0x0000_0001;
const PFD_TYPE_RGBA: u8 = 0;
const PFD_MAIN_PLANE: u8 = 0;
const WS_POPUP: u32 = 0x8000_0000;

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

#[link(name = "gdi32")]
extern "system" {
    fn ChoosePixelFormat(hdc: HDC, ppfd: *const PIXELFORMATDESCRIPTOR) -> i32;
    fn DescribePixelFormat(
        hdc: HDC,
        format: i32,
        bytes: u32,
        ppfd: *mut PIXELFORMATDESCRIPTOR,
    ) -> i32;
    fn SetPixelFormat(hdc: HDC, format: i32, ppfd: *const PIXELFORMATDESCRIPTOR) -> i32;
    fn SwapBuffers(hdc: HDC) -> i32;
}

#[link(name = "opengl32")]
extern "system" {
    fn wglCreateContext(hdc: HDC) -> HGLRC;
    fn wglMakeCurrent(hdc: HDC, hglrc: HGLRC) -> i32;
    fn wglDeleteContext(hglrc: HGLRC) -> i32;
    fn wglGetProcAddress(name: *const i8) -> *const c_void;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleA(module_name: *const i8) -> *mut c_void;
    fn GetModuleHandleW(module_name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, proc_name: *const i8) -> *const c_void;
}

#[link(name = "user32")]
extern "system" {
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: HWND,
        menu: *mut c_void,
        instance: *mut c_void,
        param: *mut c_void,
    ) -> HWND;
    fn DestroyWindow(hwnd: HWND) -> i32;
}

fn invalid_wgl_proc(proc: *const c_void) -> bool {
    matches!(proc as isize, -1..=3)
}

pub(crate) fn load_gl_proc(name: &CStr) -> *const c_void {
    // SAFETY: `name` is NUL-terminated and remains alive for both loader calls.
    let proc = unsafe { wglGetProcAddress(name.as_ptr()) };
    if !invalid_wgl_proc(proc) {
        return proc;
    }

    // Core OpenGL 1.1 exports (for example `glGetString`) come from opengl32.dll
    // and are not required to be returned by wglGetProcAddress.
    // SAFETY: opengl32 is a linked dependency of this module, so its module handle
    // is valid while the process is running. Both pointers are NUL-terminated.
    unsafe {
        let module = GetModuleHandleA(c"opengl32.dll".as_ptr());
        if module.is_null() {
            ptr::null()
        } else {
            GetProcAddress(module, name.as_ptr())
        }
    }
}

fn load_wgl_fn<T>(name: &str) -> Option<T> {
    let c_name = CString::new(name).ok()?;
    let proc = load_gl_proc(&c_name);
    if invalid_wgl_proc(proc) {
        None
    } else {
        // SAFETY: callers request a function type matching the named WGL symbol;
        // Win32 function pointers have the same pointer-sized representation.
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

fn describe_pixel_format(hdc: HDC, format: i32) -> Result<PIXELFORMATDESCRIPTOR, Error> {
    let mut actual = default_pfd();
    unsafe {
        if DescribePixelFormat(
            hdc,
            format,
            std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u32,
            &mut actual,
        ) == 0
        {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: DescribePixelFormat failed",
            ));
        }
    }
    Ok(actual)
}

fn set_selected_pixel_format(hdc: HDC, format: i32) -> Result<(i32, u32), Error> {
    let actual = describe_pixel_format(hdc, format)?;
    let required = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
    if actual.dwFlags & required != required {
        return Err(Error::new(
            Errc::PlatformError,
            format!(
                "WglContext: selected pixel format {format} lacks required flags; actual={:#010X}, required={required:#010X}",
                actual.dwFlags
            ),
        ));
    }
    if unsafe { SetPixelFormat(hdc, format, &actual) } == 0 {
        return Err(windows_diag(
            Errc::PlatformError,
            "WglContext: SetPixelFormat failed",
        ));
    }
    Ok((format, actual.dwFlags))
}

fn setup_legacy_pixel_format(hdc: HDC) -> Result<(i32, u32), Error> {
    let pfd = default_pfd();
    let format = unsafe { ChoosePixelFormat(hdc, &pfd) };
    if format == 0 {
        return Err(windows_diag(
            Errc::PlatformError,
            "WglContext: ChoosePixelFormat failed",
        ));
    }
    set_selected_pixel_format(hdc, format)
}

fn setup_arb_pixel_format(
    hdc: HDC,
    choose_pixel_format: ChoosePixelFormatArbFn,
) -> Result<(i32, u32), Error> {
    let attributes = [
        WGL_DRAW_TO_WINDOW_ARB,
        1,
        WGL_SUPPORT_OPENGL_ARB,
        1,
        WGL_DOUBLE_BUFFER_ARB,
        1,
        WGL_PIXEL_TYPE_ARB,
        WGL_TYPE_RGBA_ARB,
        WGL_COLOR_BITS_ARB,
        24,
        0,
    ];
    let mut format = 0;
    let mut count = 0;
    let ok = unsafe {
        choose_pixel_format(
            hdc,
            attributes.as_ptr(),
            ptr::null(),
            1,
            &mut format,
            &mut count,
        )
    };
    if ok == 0 || count == 0 || format == 0 {
        return Err(windows_diag(
            Errc::PlatformError,
            "WglContext: wglChoosePixelFormatARB found no displayable format",
        ));
    }
    set_selected_pixel_format(hdc, format)
}

struct BootstrapContext {
    hwnd: HWND,
    hdc: HDC,
    hglrc: HGLRC,
}

impl BootstrapContext {
    fn new() -> Result<Self, Error> {
        const STATIC_CLASS: [u16; 7] = [83, 84, 65, 84, 73, 67, 0];
        const EMPTY_TITLE: [u16; 1] = [0];
        let instance = unsafe { GetModuleHandleW(ptr::null()) };
        if instance.is_null() {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: bootstrap GetModuleHandleW failed",
            ));
        }
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                STATIC_CLASS.as_ptr(),
                EMPTY_TITLE.as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null_mut(),
            )
        };
        if hwnd.is_null() {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: bootstrap CreateWindowExW failed",
            ));
        }
        let hdc = unsafe { device_context(hwnd) };
        let mut context = Self {
            hwnd,
            hdc,
            hglrc: ptr::null_mut(),
        };
        if hdc.is_null() {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: bootstrap GetDC failed",
            ));
        }
        setup_legacy_pixel_format(hdc)?;
        context.hglrc = unsafe { wglCreateContext(hdc) };
        if context.hglrc.is_null() {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: bootstrap wglCreateContext failed",
            ));
        }
        if unsafe { wglMakeCurrent(hdc, context.hglrc) } == 0 {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: bootstrap wglMakeCurrent failed",
            ));
        }
        Ok(context)
    }
}

impl Drop for BootstrapContext {
    fn drop(&mut self) {
        unsafe {
            if !self.hglrc.is_null() {
                wglMakeCurrent(ptr::null_mut(), ptr::null_mut());
                wglDeleteContext(self.hglrc);
                self.hglrc = ptr::null_mut();
            }
            if !self.hdc.is_null() {
                release_device_context(self.hwnd, self.hdc);
                self.hdc = ptr::null_mut();
            }
            if !self.hwnd.is_null() {
                DestroyWindow(self.hwnd);
                self.hwnd = ptr::null_mut();
            }
        }
    }
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

/// WGL + OpenGL ES graphics context bound to a Win32 HWND.
pub struct WglContext {
    hwnd: HWND,
    hdc: HDC,
    hglrc: HGLRC,
    logical_width: i32,
    logical_height: i32,
    width: i32,
    height: i32,
    pipeline: OpenGlRasterPipeline,
}

#[cfg(test)]
impl WglContext {
    pub(crate) fn glyph_atlas_upload_count(&self) -> usize {
        self.pipeline.glyph_atlas_upload_count()
    }

    pub(crate) fn has_soft_texture(&self) -> bool {
        self.pipeline.has_soft_texture()
    }
}

impl WglContext {
    /// Create a WGL context on `native_window` (HWND).
    ///
    /// Creates the OpenGL ES 3.0 context required by the renderer shaders and VAOs.
    pub(crate) fn new(native_window: *mut c_void, width: i32, height: i32) -> Result<Self, Error> {
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
        let hdc = unsafe { device_context(hwnd) };
        if hdc.is_null() {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: GetDC failed",
            ));
        }

        let result = (|| -> Result<Self, Error> {
            let bootstrap = BootstrapContext::new()?;
            let choose_pixel_format = load_wgl_fn::<ChoosePixelFormatArbFn>(
                "wglChoosePixelFormatARB",
            )
            .ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "WglContext: wglChoosePixelFormatARB unavailable",
                )
            })?;
            let create_ctx = load_wgl_fn::<CreateContextAttribsFn>("wglCreateContextAttribsARB");
            let (pixel_format, pixel_format_flags) =
                setup_arb_pixel_format(hdc, choose_pixel_format)?;
            let hglrc = if let Some(create_ctx) = create_ctx {
                create_es_context(hdc, create_ctx, 3, 0)?
            } else {
                return Err(Error::new(
                    Errc::PlatformError,
                    "WglContext: wglCreateContextAttribsARB unavailable",
                ));
            };
            drop(bootstrap);
            unsafe {
                if wglMakeCurrent(hdc, hglrc) == 0 {
                    wglDeleteContext(hglrc);
                    return Err(windows_diag(
                        Errc::PlatformError,
                        "WglContext: wglMakeCurrent failed",
                    ));
                }
            }

            let drawable = drawable_size_from_hdc(hwnd, hdc, width, height);
            let runtime =
                crate::native::graphics::opengl::NativeOpenGlRuntime::from_loader(|name| {
                    let Some(name) = CString::new(name).ok() else {
                        return ptr::null();
                    };
                    let proc = load_gl_proc(&name);
                    if invalid_wgl_proc(proc) {
                        ptr::null()
                    } else {
                        proc
                    }
                });
            let pipeline = match OpenGlRasterPipeline::new(
                runtime,
                drawable.logical_width,
                drawable.logical_height,
                drawable.width,
                drawable.height,
            ) {
                Ok(pipeline) => pipeline,
                Err(error) => {
                    unsafe {
                        wglMakeCurrent(ptr::null_mut(), ptr::null_mut());
                        wglDeleteContext(hglrc);
                    }
                    return Err(error);
                }
            };
            crate::core::log::info_fn(format_args!(
                "WglContext: OpenGL ES context created ({}x{} drawable, logical {}x{}, pixel_format={pixel_format}, flags={pixel_format_flags:#010X})",
                drawable.width, drawable.height, drawable.logical_width, drawable.logical_height,
            ));
            Ok(Self {
                hwnd,
                hdc,
                hglrc,
                logical_width: drawable.logical_width,
                logical_height: drawable.logical_height,
                width: drawable.width,
                height: drawable.height,
                pipeline,
            })
        })();

        if result.is_err() {
            unsafe {
                release_device_context(hwnd, hdc);
            }
        }
        result
    }

    fn make_current_result(&self) -> Result<(), Error> {
        if unsafe { wglMakeCurrent(self.hdc, self.hglrc) } == 0 {
            Err(windows_diag(
                Errc::PlatformError,
                "WglContext: wglMakeCurrent failed",
            ))
        } else {
            Ok(())
        }
    }

    fn swap_buffers_result(&self) -> Result<(), Error> {
        if unsafe { SwapBuffers(self.hdc) } == 0 {
            Err(windows_diag(
                Errc::PlatformError,
                "WglContext: SwapBuffers failed",
            ))
        } else {
            Ok(())
        }
    }

    fn shutdown_result(&mut self) -> Result<(), Error> {
        if !self.hglrc.is_null() {
            if unsafe { wglMakeCurrent(self.hdc, self.hglrc) } == 0 {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "WglContext: wglMakeCurrent during shutdown failed",
                ));
            }
            self.pipeline.release();
            if unsafe { wglMakeCurrent(ptr::null_mut(), ptr::null_mut()) } == 0 {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "WglContext: wglMakeCurrent(NULL) during shutdown failed",
                ));
            }
            if unsafe { wglDeleteContext(self.hglrc) } == 0 {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "WglContext: wglDeleteContext failed",
                ));
            }
            self.hglrc = ptr::null_mut();
        }
        if !self.hdc.is_null() {
            if !unsafe { release_device_context_checked(self.hwnd, self.hdc) } {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "WglContext: ReleaseDC failed",
                ));
            }
            self.hdc = ptr::null_mut();
        }
        Ok(())
    }
}

impl IGraphicsContext for WglContext {
    fn caps(&self) -> crate::native::traits::present::GraphicsContextCaps {
        crate::native::traits::present::GraphicsContextCaps::gpu_native_swapchain(
            crate::native::traits::present::GraphicsBackend::OpenGlEs,
            PresentCoherency::FullOnly,
            self.device_pixel_ratio(),
        )
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps {
            clear_target: true,
            clear_rects: true,
            soft_blit: true,
            solid_rects: true,
            glyphs: true,
            offscreen_targets: true,
            ..NativeRasterCaps::default()
        }
    }

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

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.make_current()?;
        let drawable = drawable_size_from_hdc(self.hwnd, self.hdc, width, height);
        self.logical_width = drawable.logical_width;
        self.logical_height = drawable.logical_height;
        self.width = drawable.width;
        self.height = drawable.height;
        self.pipeline.resize_swapchain(
            drawable.logical_width,
            drawable.logical_height,
            drawable.width,
            drawable.height,
        );
        Ok(())
    }

    fn make_current(&mut self) -> Result<(), Error> {
        self.make_current_result()
    }

    fn swap_buffers(&mut self, damage: PresentDamage) -> Result<(), Error> {
        let _ = damage;
        self.swap_buffers_result()
    }

    fn present(&mut self, frame: &PresentFrame) -> Result<(), Error> {
        match frame {
            PresentFrame::Swapchain { .. } => {
                self.make_current_result()?;
                self.swap_buffers_result()
            }
            PresentFrame::PixelBuffer { .. } => Err(Error::new(
                Errc::NotImplemented,
                "WglContext: CPU pixel present is unsupported",
            )),
        }
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.shutdown_result()
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>, Error> {
        self.make_current_result()?;
        self.pipeline.read_pixels(x, y, width, height)
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn clear_render_target(&mut self, r: f32, g: f32, b: f32, a: f32) -> Result<(), Error> {
        self.make_current_result()?;
        self.pipeline.clear_render_target([r, g, b, a])
    }

    fn draw_solid_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<(), Error> {
        self.make_current_result()?;
        self.pipeline
            .draw_solid_rects(viewport_w, viewport_h, scissor, rects)
    }

    fn draw_glyphs(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<(), Error> {
        self.make_current_result()?;
        self.pipeline
            .draw_glyphs(viewport_w, viewport_h, scissor, glyphs)
    }

    fn blit_soft_fallback_tile(
        &mut self,
        pixels: &[u32],
        tile: SoftFallbackTile,
    ) -> Result<(), Error> {
        self.make_current_result()?;
        let (target_width, target_height) = self.pipeline.current_target_size();
        self.pipeline
            .blit_soft_fallback_tile(pixels, target_width, target_height, tile)
    }

    fn upload_surface_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
    ) -> Result<(), Error> {
        self.make_current_result()?;
        self.pipeline.upload_surface_pixels(pixels, width, height)
    }

    fn clear_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        rects: &[GpuSolidRect],
    ) -> Result<(), Error> {
        self.make_current_result()?;
        self.pipeline.clear_rects(rects)
    }

    fn create_offscreen_target(
        &mut self,
        width: i32,
        height: i32,
    ) -> Result<OffscreenTargetId, Error> {
        self.make_current_result()?;
        self.pipeline.create_offscreen_target(width, height)
    }

    fn try_destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        self.make_current_result()?;
        self.pipeline.destroy_offscreen_target(id)
    }

    fn bind_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        self.make_current_result()?;
        self.pipeline.bind_offscreen_target(id)
    }

    fn bind_swapchain_target(&mut self) -> Result<(), Error> {
        self.make_current_result()?;
        self.pipeline.bind_swapchain_target();
        Ok(())
    }

    fn blit_offscreen_target(
        &mut self,
        id: OffscreenTargetId,
        src: crate::core::Rect,
        dst: crate::core::Rect,
        opacity: f32,
        additive: bool,
    ) -> Result<(), Error> {
        if additive {
            return Err(Error::new(
                Errc::NotImplemented,
                "WglContext: Additive blit_offscreen_target requires wgpu",
            ));
        }
        self.make_current_result()?;
        self.pipeline.blit_offscreen_target(id, src, dst, opacity)
    }

    fn device_pixel_ratio(&self) -> f32 {
        if self.logical_width <= 0 {
            return 1.0;
        }
        self.width as f32 / self.logical_width as f32
    }
}

impl Drop for WglContext {
    fn drop(&mut self) {
        let _ = self.try_shutdown();
    }
}
