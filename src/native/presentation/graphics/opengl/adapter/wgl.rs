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
    DrawableSize, device_context, drawable_size_from_hdc, release_device_context_checked,
};
use crate::native::{Errc, Error};
// 引入唯一共享 Surface 生命周期与初始化原因。
use crate::platform::presentation::rhi::{
    RhiExtent, RhiSurfaceLifecycle, RhiSurfaceRecreateReason,
};

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
            // 启动期关闭失败经边界观察入口记录，便于定位泄漏。
            crate::diagnostics::observe_boundary_error("wgl/bootstrap", &error);
        }
    }
}

// 在正式 WglContext 接管前唯一持有新建 HGLRC，并负责失败回滚。
struct PendingWglContext {
    // 保存尚未交付的 WGL context 句柄。
    hglrc: HGLRC,
    // 记录该 context 当前是否绑定到构造线程。
    current: bool,
}

impl PendingWglContext {
    // 从刚创建的非空 HGLRC 建立构造期唯一 owner。
    fn new(hglrc: HGLRC) -> Self {
        // 构造完成前 context 尚未成功设为 current。
        Self {
            // 接管调用方刚创建的 HGLRC。
            hglrc,
            // 初始绑定状态为 false。
            current: false,
        }
    }

    // 标记 wglMakeCurrent 已成功提交，回滚时必须先解绑。
    fn mark_current(&mut self) {
        // 只在 native 调用成功后更新 owner 状态。
        self.current = true;
    }

    // 创建失败时执行一次检查式清理，并保留主错误与清理错误链。
    fn finish_failure(&mut self, primary_error: Error) -> Error {
        // 清理成功时仍向调用方传播原始创建失败。
        match self.shutdown_result() {
            // 临时 HGLRC 已完整释放。
            Ok(()) => primary_error,
            // 清理失败成为外层错误，原始创建失败保留为原因。
            Err(cleanup_error) => cleanup_error.with_source(primary_error),
        }
    }

    // 成功创建后把 HGLRC 所有权移交给正式 WglContext。
    fn into_handle(mut self) -> HGLRC {
        // 保存即将交付的唯一句柄。
        let hglrc = self.hglrc;
        // 清空临时 owner，防止其 Drop 删除已交付句柄。
        self.hglrc = ptr::null_mut();
        // current 状态随句柄一并转移给正式 owner。
        self.current = false;
        // 返回由 WglContext 字段接管的 HGLRC。
        hglrc
    }

    // 按 current 绑定依赖逆序解绑并删除尚未交付的 HGLRC。
    fn shutdown_result(&mut self) -> Result<(), Error> {
        // 空句柄表示已清理或已移交，保持幂等成功。
        if self.hglrc.is_null() {
            // 不重复触碰底层 WGL API。
            return Ok(());
        }
        // 只有成功设为 current 的 context 才需要先解绑。
        if self.current {
            // SAFETY: 当前线程创建并绑定了该 HGLRC；空参数表示解除 current 绑定。
            if unsafe { wglMakeCurrent(ptr::null_mut(), ptr::null_mut()) } == 0 {
                // 解绑失败时保留句柄与 current 状态供 Drop 重试。
                return Err(windows_diag(
                    // 将 WGL teardown 失败归入平台错误。
                    Errc::PlatformError,
                    // 保留失败的精确 native 操作名。
                    "WglContext: pending context wglMakeCurrent(NULL) failed",
                ));
            }
            // 只有解绑成功后才提交 current 状态清除。
            self.current = false;
        }
        // SAFETY: hglrc 仍由本临时 owner 唯一持有且当前未绑定。
        if unsafe { wglDeleteContext(self.hglrc) } == 0 {
            // 删除失败时保留句柄供 Drop 重试。
            return Err(windows_diag(
                // 将 WGL teardown 失败归入平台错误。
                Errc::PlatformError,
                // 保留失败的精确 native 操作名。
                "WglContext: pending context wglDeleteContext failed",
            ));
        }
        // 只有删除成功后才清空唯一 owner 槽位。
        self.hglrc = ptr::null_mut();
        // 未交付 context 已完成检查式关闭。
        Ok(())
    }
}

impl Drop for PendingWglContext {
    fn drop(&mut self) {
        // Drop 只重试尚未完成的检查式关闭，失败必须留下最终诊断。
        if let Err(error) = self.shutdown_result() {
            // 创建期关闭失败经边界观察入口记录，便于定位泄漏。
            crate::diagnostics::observe_boundary_error("wgl/pending", &error);
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
    // 唯一拥有 Surface generation、extent 与重建顺序的共享状态机。
    surface_lifecycle: RhiSurfaceLifecycle,
    pipeline: OpenGlRasterPipeline,
    // 关闭事务一旦开始便禁止新的业务 RHI 借用，但允许 cleanup 重试。
    shutdown_started: bool,
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
            // 在创建正式 WGL context 前冻结初始 drawable 与初始化事务。
            let drawable = drawable_size_from_hdc(hwnd, hdc, width, height);
            let initial_extent = RhiExtent::new(drawable.width as u32, drawable.height as u32);
            let mut surface_lifecycle = RhiSurfaceLifecycle::uninitialized(initial_extent);
            let surface_initialize = surface_lifecycle
                .begin_recreate(initial_extent, RhiSurfaceRecreateReason::Initialize)?;
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
            // 先创建尚未交付给正式 WglContext 的 HGLRC。
            let hglrc = if let Some(create_ctx) = create_ctx {
                create_es_context(hdc, create_ctx, 3, 0)?
            } else {
                return Err(Error::new(
                    Errc::PlatformError,
                    "WglContext: wglCreateContextAttribsARB unavailable",
                ));
            };
            // 建立构造期唯一 owner，后续失败由它检查式回滚。
            let mut pending_context = PendingWglContext::new(hglrc);
            // SAFETY: hdc 存活、hglrc 为刚创建的非空上下文；失败时 hglrc 仍有效可删除。
            if unsafe { wglMakeCurrent(hdc, hglrc) } == 0 {
                // 在任何清理调用前捕获原始 WGL 创建失败。
                let primary_error = windows_diag(
                    // 将 WGL 创建失败归入平台错误。
                    Errc::PlatformError,
                    // 保留失败的精确 native 操作名。
                    "WglContext: wglMakeCurrent failed",
                );
                // 传播主错误；若删除失败则同时保留清理错误。
                return Err(pending_context.finish_failure(primary_error));
            }
            // 只在 native 绑定成功后更新临时 owner 状态。
            pending_context.mark_current();

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
            let mut pipeline = match OpenGlRasterPipeline::new(
                runtime,
                drawable.logical_width,
                drawable.logical_height,
                drawable.width,
                drawable.height,
            ) {
                Ok(pipeline) => pipeline,
                Err(error) => {
                    // 由临时 owner 检查式解绑并删除，保留初始化与清理双错误链。
                    return Err(pending_context.finish_failure(error));
                }
            };
            // 原生 drawable 与 pipeline 成功后一次发布初始 generation 和 extent。
            if let Err(error) =
                surface_lifecycle.commit_recreate(surface_initialize, initial_extent)
            {
                // 生命周期提交失败时仍在 current context 上检查式释放 GL 资源。
                pipeline.release();
                // 临时 HGLRC owner 继续完成解绑和删除。
                return Err(pending_context.finish_failure(error));
            }
            tracing::info!(
                "WglContext: OpenGL ES context created ({}x{} drawable, logical {}x{}, pixel_format={pixel_format}, flags={pixel_format_flags:#010X})",
                drawable.width,
                drawable.height,
                drawable.logical_width,
                drawable.logical_height,
            );
            // 所有创建步骤成功后才把唯一 HGLRC owner 移交给正式 context。
            let hglrc = pending_context.into_handle();
            // 正式 WglContext 从此负责 HGLRC 的 checked teardown。
            Ok(Self {
                hwnd,
                hdc,
                hglrc,
                logical_width: drawable.logical_width,
                logical_height: drawable.logical_height,
                width: drawable.width,
                height: drawable.height,
                // 接管已经完成 Initialize 事务的共享生命周期 owner。
                surface_lifecycle,
                pipeline,
                // 初始 owner 尚未进入关闭事务。
                shutdown_started: false,
            })
        })();

        // 构造事务成功时把正式 context 与 HDC owner 一并交付调用方。
        match result {
            // 成功值已经接管 HDC，外层不得再释放。
            Ok(context) => Ok(context),
            // 失败时当前构造事务仍是目标 HDC 的唯一 owner。
            Err(primary_error) => {
                // SAFETY: hwnd/hdc 由本函数取得且尚未转移，checked helper 只在此释放一次。
                if unsafe { release_device_context_checked(hwnd, hdc) } {
                    // 清理成功后保留原始构造失败作为调用方应观察的主错误。
                    Err(primary_error)
                } else {
                    // HDC 未完成释放时以清理失败为外层错误，并保留原构造失败原因。
                    Err(windows_diag(
                        // 将 Win32 关闭失败归入平台错误。
                        Errc::PlatformError,
                        // 保留失败的精确 native 操作名。
                        "WglContext: construction rollback ReleaseDC failed",
                    )
                    // 错误链同时保留触发回滚的原始构造失败。
                    .with_source(primary_error))
                }
            }
        }
    }

    fn make_current_result(&self) -> Result<(), Error> {
        // 在任何 WGL 调用前拒绝已开始关闭或句柄不完整的 owner。
        self.ensure_rhi_active()?;
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
        // 在任何原生 cleanup 或 pipeline release 前发布关闭事实。
        self.shutdown_started = true;
        if !self.hglrc.is_null() {
            // SAFETY: hglrc 非空且存活；先 current 该上下文以便 release GL 资源，再解除 current，最后删除 context；全程同一 owner 线程。
            if !self.hdc.is_null() {
                if unsafe { wglMakeCurrent(self.hdc, self.hglrc) } == 0 {
                    return Err(windows_diag(
                        Errc::PlatformError,
                        "WglContext: wglMakeCurrent during shutdown failed",
                    ));
                }
                self.pipeline.release();
            }
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

    // 检查 WGL owner 是否仍可被业务 RHI 使用。
    pub(crate) fn ensure_rhi_active(&self) -> Result<(), Error> {
        // 关闭事务或任一关键句柄失效都统一为 InvalidState。
        if self.shutdown_started || self.hdc.is_null() || self.hglrc.is_null() {
            // 在触碰任何 WGL API 前返回稳定生命周期错误。
            return Err(Error::new(
                Errc::InvalidState,
                "WglContext: operation requested after shutdown",
            ));
        }
        // owner 的 HDC 与 HGLRC 仍完整存活。
        Ok(())
    }

    // 直接更新 WGL surface 的 drawable 元数据和 RHI swapchain 状态。
    fn resize_surface_drawable(&mut self, drawable: DrawableSize) -> Result<(), Error> {
        // 确保 pipeline 的 viewport 和资源操作仍在 owner-thread context 上。
        self.make_current_result()?;
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
        // generation 只允许在共享生命周期 commit 后发布。
        Ok(())
    }

    // 在 SurfaceLost 后按 WGL 原生顺序重新取得唯一窗口 HDC。
    fn recreate_device_context(&mut self) -> Result<(), Error> {
        // SAFETY: null/null 只解除当前线程绑定，不引用旧 HDC/HGLRC。
        if unsafe { wglMakeCurrent(ptr::null_mut(), ptr::null_mut()) } == 0 {
            return Err(windows_diag(
                Errc::PlatformError,
                "WglContext: wglMakeCurrent(NULL) before surface recreate failed",
            ));
        }
        if !self.hdc.is_null() {
            // SAFETY: hwnd/hdc 由当前 owner 配对持有，成功后立即清空旧句柄。
            if !unsafe { release_device_context_checked(self.hwnd, self.hdc) } {
                return Err(windows_diag(
                    Errc::GraphicsSurfaceLost,
                    "WglContext: surface recreate ReleaseDC failed",
                ));
            }
            self.hdc = ptr::null_mut();
        }
        // SAFETY: hwnd 在正式 context 生命周期内仍由窗口 owner 保持存活。
        let replacement = unsafe { device_context(self.hwnd) };
        if replacement.is_null() {
            return Err(windows_diag(
                Errc::GraphicsSurfaceLost,
                "WglContext: surface recreate GetDC failed",
            ));
        }
        // 新 HDC 一经取得便立即交回唯一正式 owner。
        self.hdc = replacement;
        // SAFETY: replacement 属于同一 HWND，pixel format 固定在窗口上；hglrc 仍存活。
        if unsafe { wglMakeCurrent(self.hdc, self.hglrc) } == 0 {
            return Err(windows_diag(
                Errc::GraphicsSurfaceLost,
                "WglContext: wglMakeCurrent on recreated surface failed",
            ));
        }
        Ok(())
    }
}

// 释放 WGL owner-thread 的所有 GPU 资源。
impl Drop for WglContext {
    fn drop(&mut self) {
        // Drop 直接调用 inherent shutdown，避免依赖已拆出的 trait impl。
        if let Err(error) = self.shutdown_result() {
            // Drop 关闭失败经边界观察入口记录，供最终责任边界定位泄漏。
            crate::diagnostics::observe_boundary_error("wgl/adapter", &error);
        }
    }
}
