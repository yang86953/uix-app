//! WGL + OpenGL ES 3.0 graphics context for Windows.
//!
//! Creates an ES profile context on the window HWND and implements
//! OpenGL ES GPU recipe context for the unique draw `Renderer`.

#![allow(nonstandard_style)]
#![allow(clippy::missing_safety_doc)]
#![allow(
    clippy::upper_case_acronyms,
    reason = "these private declarations intentionally mirror the Windows ABI spellings"
)]

use std::ffi::{CStr, CString, c_void};
use std::ptr;

use crate::native::backends::windows::util::windows_diag;
// 引入共享的 OpenGL RHI host 生命周期实现。
use crate::native::presentation::graphics::opengl::raster::OpenGlRasterPipeline;
use crate::native::presentation::graphics::platform::windows::{
    DrawableSize, device_context, drawable_size_from_hdc, release_device_context,
    release_device_context_checked,
};
use crate::native::{Errc, Error};

// 将 WGL 的 RHI 生命周期实现拆到独立文件，避免平台适配文件继续膨胀。
#[path = "wgl_rhi.rs"]
mod wgl_rhi;
// 将 WGL 的共享生命周期与类型化 recipe 实现拆到独立文件。
#[path = "wgl_graphics.rs"]
mod wgl_graphics;

type HDC = *mut c_void;
type HGLRC = *mut c_void;
type HWND = *mut c_void;

// SAFETY: 该函数指针只由同名 WGL 扩展符号解析，调用点负责提供存活 HDC/HGLRC 与零结尾属性表。
type CreateContextAttribsFn = unsafe extern "system" fn(HDC, HGLRC, *const i32) -> HGLRC;
// SAFETY: 该函数指针只由同名 WGL 扩展符号解析，调用点负责提供有效属性数组与输出缓冲区。
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
// SAFETY: GDI32 声明与 Windows system ABI 一致；每个调用点继续验证 HDC、结构体指针和缓冲区生命周期。
unsafe extern "system" {
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
// SAFETY: OpenGL32 声明与 Windows system ABI 一致；每个调用点继续维护 HDC/HGLRC 存活和 current 上下文。
unsafe extern "system" {
    fn wglCreateContext(hdc: HDC) -> HGLRC;
    fn wglMakeCurrent(hdc: HDC, hglrc: HGLRC) -> i32;
    fn wglDeleteContext(hglrc: HGLRC) -> i32;
    fn wglGetProcAddress(name: *const i8) -> *const c_void;
}

#[link(name = "kernel32")]
// SAFETY: Kernel32 声明与 Windows system ABI 一致；每个调用点继续提供零结尾名称并验证模块句柄。
unsafe extern "system" {
    fn GetModuleHandleA(module_name: *const i8) -> *mut c_void;
    fn GetModuleHandleW(module_name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, proc_name: *const i8) -> *const c_void;
}

#[link(name = "user32")]
// SAFETY: User32 声明与 Windows system ABI 一致；每个调用点继续维护窗口、类名和实例句柄的生命周期。
unsafe extern "system" {
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
    // SAFETY: hdc 由调用方持有且存活；actual 指向的 PIXELFORMATDESCRIPTOR 与传入的字节数匹配，DescribePixelFormat 只会写入该结构体。
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
    // SAFETY: hdc 存活；actual 为上方 describe_pixel_format 返回的有效像素格式描述，SetPixelFormat 同步读取它且调用期间未修改。
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
    // SAFETY: hdc 存活；pfd 为栈上完整初始化的像素格式描述，ChoosePixelFormat 只读它。
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
    // SAFETY: hdc 存活；attributes 为栈上存活且 NUL 结尾的属性数组（以 0 结束）；format/count 指向有效输出；float 列表传 null。
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
        // SAFETY: GetModuleHandleW 传 null 表示查询当前进程模块句柄，无指针输入。
        let instance = unsafe { GetModuleHandleW(ptr::null()) };
        if instance.is_null() {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: bootstrap GetModuleHandleW failed",
            ));
        }
        // SAFETY: STATIC_CLASS/EMPTY_TITLE 均为存活且 NUL 结尾的 UTF-16 数组；instance 为刚取得的进程模块句柄；其余参数为常量或 null。
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
        // SAFETY: hwnd 为刚创建的非空窗口句柄，device_context 只读取它并返回有效 HDC。
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
        // SAFETY: hdc 存活且已设置像素格式；创建失败以空指针返回而非 UB。
        context.hglrc = unsafe { wglCreateContext(hdc) };
        if context.hglrc.is_null() {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: bootstrap wglCreateContext failed",
            ));
        }
        // SAFETY: hdc 存活、hglrc 为刚创建的非空上下文；失败以返回码 0 表示。
        if unsafe { wglMakeCurrent(hdc, context.hglrc) } == 0 {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: bootstrap wglMakeCurrent failed",
            ));
        }
        Ok(context)
    }

    // 按 WGL 依赖逆序关闭 bootstrap 资源，并只在成功后释放对应 owner 槽位。
    fn shutdown_result(&mut self) -> Result<(), Error> {
        // OpenGL context 必须先解绑再删除，后续 DC 与窗口才能安全释放。
        if !self.hglrc.is_null() {
            // SAFETY: 当前线程只持有本 bootstrap context；空 HDC/HGLRC 表示解除 current 绑定。
            if unsafe { wglMakeCurrent(ptr::null_mut(), ptr::null_mut()) } == 0 {
                // 解绑失败时保留全部句柄，允许 Drop 再次尝试完整 teardown。
                return Err(windows_diag(
                    // 将 WGL 关闭失败归入平台错误。
                    Errc::PlatformError,
                    // 保留失败的精确 native 操作名。
                    "WglContext: bootstrap wglMakeCurrent(NULL) failed",
                ));
            }
            // SAFETY: hglrc 仍由本对象唯一持有，且已从当前线程解除绑定。
            if unsafe { wglDeleteContext(self.hglrc) } == 0 {
                // 删除失败时保留 hglrc，供 Drop 重试并留下最终诊断。
                return Err(windows_diag(
                    // 将 WGL 关闭失败归入平台错误。
                    Errc::PlatformError,
                    // 保留失败的精确 native 操作名。
                    "WglContext: bootstrap wglDeleteContext failed",
                ));
            }
            // 只有删除成功后才提交 context owner 为空。
            self.hglrc = ptr::null_mut();
        }
        // context 已删除后释放与隐藏窗口配对的 device context。
        if !self.hdc.is_null() {
            // SAFETY: hdc 来自同一 hwnd 的 GetDC，且仍由本对象唯一持有。
            if !unsafe { release_device_context_checked(self.hwnd, self.hdc) } {
                // 释放失败时保留 hdc 与 hwnd，允许 Drop 重试。
                return Err(windows_diag(
                    // 将 Win32 关闭失败归入平台错误。
                    Errc::PlatformError,
                    // 保留失败的精确 native 操作名。
                    "WglContext: bootstrap ReleaseDC failed",
                ));
            }
            // 只有 ReleaseDC 成功后才清空 device context owner。
            self.hdc = ptr::null_mut();
        }
        // 最后销毁仅为加载 WGL 扩展而创建的隐藏窗口。
        if !self.hwnd.is_null() {
            // SAFETY: hwnd 由本对象创建且 context/DC 均已完成释放。
            if unsafe { DestroyWindow(self.hwnd) } == 0 {
                // 销毁失败时保留 hwnd，允许 Drop 重试。
                return Err(windows_diag(
                    // 将 Win32 关闭失败归入平台错误。
                    Errc::PlatformError,
                    // 保留失败的精确 native 操作名。
                    "WglContext: bootstrap DestroyWindow failed",
                ));
            }
            // 只有隐藏窗口销毁成功后才清空最终 owner 槽位。
            self.hwnd = ptr::null_mut();
        }
        // 所有 bootstrap native 资源均已完成检查式关闭。
        Ok(())
    }
}

impl Drop for BootstrapContext {
    fn drop(&mut self) {
        // Drop 只重试尚未完成的检查式关闭，失败必须留下最终诊断。
        if let Err(error) = self.shutdown_result() {
            // 保留 bootstrap owner 身份与 typed error 摘要，便于定位启动期泄漏。
            tracing::error!(
                "WglContext: bootstrap checked shutdown failed during Drop: {}",
                error.short_what()
            );
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
    // SAFETY: hdc 存活；attribs 为栈上存活且 NUL 结尾的属性数组；共享列表参数传 null。
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
    // surface 重建代际，用于拒绝迟到 FramePlan。
    surface_generation: u64,
    pipeline: OpenGlRasterPipeline,
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
        // SAFETY: native_window 非空（上方已校验），device_context 返回的 HDC 与窗口匹配且由本对象持有。
        let hdc = unsafe { device_context(hwnd) };
        if hdc.is_null() {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: GetDC failed",
            ));
        }

        let result = (|| -> Result<Self, Error> {
            // bootstrap 是加载 WGL 扩展期间唯一持有临时 native 资源的 owner。
            let mut bootstrap = BootstrapContext::new()?;
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
            // 在创建正式 context 前显式观察临时 bootstrap teardown 结果。
            bootstrap.shutdown_result()?;
            // bootstrap 扩展函数指针在临时 context 关闭后仍可用于创建正式 context。
            let hglrc = if let Some(create_ctx) = create_ctx {
                create_es_context(hdc, create_ctx, 3, 0)?
            } else {
                return Err(Error::new(
                    Errc::PlatformError,
                    "WglContext: wglCreateContextAttribsARB unavailable",
                ));
            };
            // SAFETY: hdc 存活、hglrc 为刚创建的非空上下文；失败时 hglrc 仍有效可删除。
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
                crate::native::presentation::graphics::opengl::NativeOpenGlRuntime::from_loader(
                    |name| {
                        let Some(name) = CString::new(name).ok() else {
                            return ptr::null();
                        };
                        let proc = load_gl_proc(&name);
                        if invalid_wgl_proc(proc) {
                            ptr::null()
                        } else {
                            proc
                        }
                    },
                );
            let pipeline = match OpenGlRasterPipeline::new(
                runtime,
                drawable.logical_width,
                drawable.logical_height,
                drawable.width,
                drawable.height,
            ) {
                Ok(pipeline) => pipeline,
                Err(error) => {
                    // SAFETY: hglrc 仍存活且此时 context 已 current（bootstrap 阶段设置），先解除再删除以避免在 current 状态下销毁。
                    unsafe {
                        wglMakeCurrent(ptr::null_mut(), ptr::null_mut());
                        wglDeleteContext(hglrc);
                    }
                    return Err(error);
                }
            };
            tracing::info!(
                "WglContext: OpenGL ES context created ({}x{} drawable, logical {}x{}, pixel_format={pixel_format}, flags={pixel_format_flags:#010X})",
                drawable.width,
                drawable.height,
                drawable.logical_width,
                drawable.logical_height,
            );
            Ok(Self {
                hwnd,
                hdc,
                hglrc,
                logical_width: drawable.logical_width,
                logical_height: drawable.logical_height,
                width: drawable.width,
                height: drawable.height,
                // 初始 WGL swapchain 属于第一代 surface。
                surface_generation: 0,
                pipeline,
            })
        })();

        if result.is_err() {
            // SAFETY: hwnd 与 hdc 均为本函数取得且未转移，只在此失败路径释放一次。
            unsafe {
                release_device_context(hwnd, hdc);
            }
        }
        result
    }

    fn make_current_result(&self) -> Result<(), Error> {
        // SAFETY: self.hdc/self.hglrc 由本对象持有且未销毁，本对象从未在线程间迁移；失败以返回码 0 表示。
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
        // SAFETY: self.hdc 为当前已绑定 context 的设备上下文且存活，SwapBuffers 只在该 HDC 上执行交换。
        if unsafe { SwapBuffers(self.hdc) } == 0 {
            Err(windows_diag(
                // SwapBuffers 失败时当前 HWND/HDC surface 已不可交换。
                Errc::GraphicsSurfaceLost,
                "WglContext: SwapBuffers failed",
            ))
        } else {
            Ok(())
        }
    }

    fn shutdown_result(&mut self) -> Result<(), Error> {
        if !self.hglrc.is_null() {
            // SAFETY: hglrc 非空且存活；先 current 该上下文以便 release GL 资源，再解除 current，最后删除 context；全程同一 owner 线程。
            if unsafe { wglMakeCurrent(self.hdc, self.hglrc) } == 0 {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "WglContext: wglMakeCurrent during shutdown failed",
                ));
            }
            self.pipeline.release();
            // SAFETY: 解除 current 传 null/null，不引用任何句柄。
            if unsafe { wglMakeCurrent(ptr::null_mut(), ptr::null_mut()) } == 0 {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "WglContext: wglMakeCurrent(NULL) during shutdown failed",
                ));
            }
            // SAFETY: hglrc 非空且已解除 current，删除不会与活动上下文冲突；删除后置空防止双重释放。
            if unsafe { wglDeleteContext(self.hglrc) } == 0 {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "WglContext: wglDeleteContext failed",
                ));
            }
            self.hglrc = ptr::null_mut();
        }
        if !self.hdc.is_null() {
            // SAFETY: hwnd/hdc 由本对象持有且未转移，release_device_context_checked 只在此释放一次。
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

    // 直接更新 WGL surface 的 drawable 元数据和 RHI swapchain 状态。
    fn resize_surface_drawable(&mut self, drawable: DrawableSize) -> Result<(), Error> {
        // 确保 pipeline 的 viewport 和资源操作仍在 owner-thread context 上。
        self.make_current_result()?;
        // 记录本次 resize 是否真的改变了 drawable surface。
        let changed = drawable.logical_width != self.logical_width
            || drawable.logical_height != self.logical_height
            || drawable.width != self.width
            || drawable.height != self.height;
        // 先更新逻辑 drawable 元数据。
        self.logical_width = drawable.logical_width;
        self.logical_height = drawable.logical_height;
        // 再更新实际物理 drawable 尺寸。
        self.width = drawable.width;
        self.height = drawable.height;
        // 把同一尺寸事实交给 OpenGL RHI pipeline。
        self.pipeline.resize_swapchain(
            drawable.logical_width,
            drawable.logical_height,
            drawable.width,
            drawable.height,
        );
        // 只有 surface 事实改变时才推进 generation，避免无意义地丢帧。
        if changed {
            self.surface_generation = self.surface_generation.saturating_add(1);
        }
        // 原生 OpenGL surface resize 已经完成。
        Ok(())
    }
}

// 释放 WGL owner-thread 的所有 GPU 资源。
impl Drop for WglContext {
    fn drop(&mut self) {
        // Drop 直接调用 inherent shutdown，避免依赖已拆出的 trait impl。
        if let Err(error) = self.shutdown_result() {
            // 保留 adapter 身份和完整 typed error 摘要，供最终责任边界定位泄漏。
            tracing::error!(
                "WglContext: checked shutdown failed during Drop: {}",
                error.short_what()
            );
        }
    }
}
