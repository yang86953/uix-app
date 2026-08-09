// ============================================================================
// native/graphics/opengl/egl.rs — EGL + GLES 3.0 图形上下文
//
// 通过 EGL 创建 OpenGL ES 3.0 上下文，对接 Wayland surface（wl_egl_window）。
// 实现 IGraphicsContext trait，供唯一 Renderer 的 GPU 后端使用。
//
// 依赖 khronos-egl v6 (static 链接) + wayland-egl (系统库 FFI)。
// ============================================================================

use std::ffi::c_void;
use std::ptr;

use crate::native::present::{
    IGraphicsContext, NativeRasterCaps, PresentCoherency, PresentDamage, PresentFrame,
};
// 引入共享的 OpenGL RHI host 生命周期实现。
use crate::native::presentation::graphics::opengl::raster::OpenGlRasterPipeline;
use crate::native::presentation::graphics::opengl::rhi_host::OpenGlRhiHost;
use crate::native::{Errc, Error};
// 引入 surface resize 使用的物理 extent 类型。
use crate::native::present::rhi::RhiExtent;

use crate::native::presentation::graphics::platform::linux::WaylandSurfaceHandle;

// 将 EGL 交换错误映射为恢复 FSM 可消费的 surface/device typed failure。
fn map_egl_swap_error(error: khronos_egl::Error) -> Error {
    // EGL_BAD_SURFACE 与 EGL_BAD_NATIVE_WINDOW 表示 native surface 已失效。
    let code = match &error {
        khronos_egl::Error::BadSurface | khronos_egl::Error::BadNativeWindow => {
            Errc::GraphicsSurfaceLost
        }
        // EGL_CONTEXT_LOST 要求销毁 context 并重新初始化所有 GLES 对象。
        khronos_egl::Error::ContextLost => Errc::GraphicsDeviceLost,
        // 其它 EGL 交换错误保留平台错误，不伪造更窄的恢复分类。
        _ => Errc::PlatformError,
    };
    // 保留原始 EGL 枚举，便于日志和故障诊断定位。
    Error::new(
        code,
        format!("EglContext: eglSwapBuffers failed: {error:?}"),
    )
}
// ════════════════════════════════════════════════════════════════════════════
// wl_egl_window FFI（wayland-egl 客户端库，Linux 系统自带）
// ════════════════════════════════════════════════════════════════════════════

/// wl_egl_window 不透明结构体
#[repr(C)]
struct WlEglWindow {
    _private: [u8; 0],
}

// 注意：wayland-egl 不是 khronos-egl 的一部分。
// 它在编译期通过 #[link] 与系统 libwayland-egl.so 链接。
#[link(name = "wayland-egl")]
extern "C" {
    fn wl_egl_window_create(surface: *mut c_void, width: i32, height: i32) -> *mut WlEglWindow;

    fn wl_egl_window_destroy(window: *mut WlEglWindow);

    fn wl_egl_window_resize(window: *mut WlEglWindow, width: i32, height: i32, dx: i32, dy: i32);
}

// ════════════════════════════════════════════════════════════════════════════
// EglContext
// ════════════════════════════════════════════════════════════════════════════

/// EGL + GLES 3.0 图形上下文。
///
/// 使用 khronos-egl v6 的静态链接 API。
/// 持有 EGLDisplay / Config / Context / Surface，
/// 通过 wl_egl_window 对接 Wayland surface。
pub struct EglContext {
    /// khronos-egl v6 的静态 API 实例（static 链接到系统 libEGL）
    egl: khronos_egl::Instance<khronos_egl::Static>,
    display: khronos_egl::Display,
    _config: khronos_egl::Config,
    context: khronos_egl::Context,
    surface: khronos_egl::Surface,
    egl_window: *mut WlEglWindow,
    width: i32,
    height: i32,
    // surface 重建代际，用于拒绝迟到 FramePlan。
    surface_generation: u64,
    pipeline: OpenGlRasterPipeline,
    shutdown: bool,
    context_destroyed: bool,
    surface_destroyed: bool,
    display_terminated: bool,
}

impl EglContext {
    /// 创建 EGL 上下文，绑定到指定的 Wayland surface 指针。
    ///
    /// `native_surface` 必须是 `*mut wl_surface`（Wayland surface 的 C 指针）。
    /// Requires GLES 3.0 because the native pipeline owns GLSL ES 3 shaders.
    pub(crate) fn new(native_surface: *mut c_void, width: i32, height: i32) -> Result<Self, Error> {
        use khronos_egl as egl;

        let wayland = unsafe { WaylandSurfaceHandle::from_native(native_surface)? };
        let egl = egl::Instance::new(egl::Static);

        // 1. 获取 display —— Wayland 下传入 display 连接指针
        let display = unsafe { egl.get_display(wayland.display as egl::NativeDisplayType) }
            .ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "EglContext: eglGetDisplay 返回 NO_DISPLAY",
                )
            })?;

        // 2. 初始化 EGL
        let (major, minor) = egl.initialize(display).map_err(|e| {
            Error::new(
                Errc::PlatformError,
                format!("EglContext: eglInitialize 失败: {e:?}"),
            )
        })?;
        tracing::info!("EglContext: EGL {major}.{minor}");

        // 3. 绑定 API 到 OpenGL ES
        if let Err(e) = egl.bind_api(egl::OPENGL_ES_API) {
            let _ = egl.terminate(display);
            return Err(Error::new(
                Errc::PlatformError,
                format!("EglContext: eglBindAPI 失败: {e:?}"),
            ));
        }

        // 4. 选择配置：RGBA 8888, depth 24, stencil 8, GLES 3。
        // The native pipeline uses GLSL ES 3 sources; accepting an ES2 config
        // would only defer a guaranteed shader failure until after setup.
        let choose_config = |renderable_type| {
            let config_attribs = [
                egl::SURFACE_TYPE,
                egl::WINDOW_BIT,
                egl::RENDERABLE_TYPE,
                renderable_type,
                egl::RED_SIZE,
                8,
                egl::GREEN_SIZE,
                8,
                egl::BLUE_SIZE,
                8,
                egl::ALPHA_SIZE,
                8,
                egl::DEPTH_SIZE,
                24,
                egl::STENCIL_SIZE,
                8,
                egl::NONE,
            ];
            egl.choose_first_config(display, &config_attribs)
        };

        let config = match choose_config(egl::OPENGL_ES3_BIT) {
            Ok(Some(config)) => config,
            Ok(None) => {
                let _ = egl.terminate(display);
                return Err(Error::new(
                    Errc::NotImplemented,
                    "EglContext: native raster requires a GLES 3 EGL config",
                ));
            }
            Err(e) => {
                let _ = egl.terminate(display);
                return Err(Error::new(
                    Errc::PlatformError,
                    format!("EglContext: GLES 3 choose_config 失败: {e:?}"),
                ));
            }
        };

        // 5. 创建 wl_egl_window（Wayland 原生窗口封装）
        let egl_window = unsafe { wl_egl_window_create(wayland.surface, width, height) };
        if egl_window.is_null() {
            let _ = egl.terminate(display);
            return Err(Error::new(
                Errc::PlatformError,
                "EglContext: wl_egl_window_create 返回 null",
            ));
        }

        // 6. 创建 EGL surface
        let surface = unsafe {
            egl.create_window_surface(display, config, egl_window as egl::NativeWindowType, None)
        }
        .map_err(|e| {
            unsafe {
                wl_egl_window_destroy(egl_window);
            }
            let _ = egl.terminate(display);
            Error::new(
                Errc::PlatformError,
                format!("EglContext: eglCreateWindowSurface 失败: {e:?}"),
            )
        })?;

        // 7. 创建 GLES 3.0 上下文；没有等价 ES2 pipeline 时不得降级。
        let ctx3_attribs = [
            egl::CONTEXT_MAJOR_VERSION,
            3,
            egl::CONTEXT_MINOR_VERSION,
            0,
            egl::NONE,
        ];
        let context = egl
            .create_context(display, config, None, &ctx3_attribs)
            .map_err(|e| {
                unsafe {
                    let _ = egl.destroy_surface(display, surface);
                    wl_egl_window_destroy(egl_window);
                }
                let _ = egl.terminate(display);
                Error::new(
                    Errc::NotImplemented,
                    format!("EglContext: native raster requires GLES 3.0: {e:?}"),
                )
            })?;
        tracing::info!("EglContext: GLES 3.0 上下文创建成功");

        // 8. make current
        egl.make_current(display, Some(surface), Some(surface), Some(context))
            .map_err(|e| {
                unsafe {
                    let _ = egl.destroy_context(display, context);
                    let _ = egl.destroy_surface(display, surface);
                    wl_egl_window_destroy(egl_window);
                }
                let _ = egl.terminate(display);
                Error::new(
                    Errc::PlatformError,
                    format!("EglContext: eglMakeCurrent 失败: {e:?}"),
                )
            })?;

        let runtime =
            crate::native::presentation::graphics::opengl::NativeOpenGlRuntime::from_loader(
                |name| {
                    egl.get_proc_address(name)
                        .map(|function| function as *const std::ffi::c_void)
                        .unwrap_or(std::ptr::null())
                },
            );
        let pipeline =
            OpenGlRasterPipeline::new(runtime, width, height, width, height).map_err(|error| {
                unsafe {
                    let _ = egl.destroy_context(display, context);
                    let _ = egl.destroy_surface(display, surface);
                    wl_egl_window_destroy(egl_window);
                }
                let _ = egl.terminate(display);
                error
            })?;

        Ok(Self {
            egl,
            display,
            _config: config,
            context,
            surface,
            egl_window,
            width,
            height,
            // 初始 EGL swapchain 属于第一代 surface。
            surface_generation: 0,
            pipeline,
            shutdown: false,
            context_destroyed: false,
            surface_destroyed: false,
            display_terminated: false,
        })
    }

    pub(crate) fn shutdown_result(&mut self) -> Result<(), Error> {
        if self.shutdown {
            return Ok(());
        }
        if !self.context_destroyed {
            self.egl
                .make_current(
                    self.display,
                    Some(self.surface),
                    Some(self.surface),
                    Some(self.context),
                )
                .map_err(|err| {
                    Error::new(
                        Errc::PlatformError,
                        format!("EglContext: eglMakeCurrent during shutdown failed: {err:?}"),
                    )
                })?;
            self.pipeline.release();
            self.egl
                .make_current(self.display, None, None, None)
                .map_err(|err| {
                    Error::new(
                        Errc::PlatformError,
                        format!("EglContext: eglMakeCurrent(NULL) during shutdown failed: {err:?}"),
                    )
                })?;
            self.egl
                .destroy_context(self.display, self.context)
                .map_err(|err| {
                    Error::new(
                        Errc::PlatformError,
                        format!("EglContext: eglDestroyContext failed: {err:?}"),
                    )
                })?;
            self.context_destroyed = true;
        }
        if !self.surface_destroyed {
            self.egl
                .destroy_surface(self.display, self.surface)
                .map_err(|err| {
                    Error::new(
                        Errc::PlatformError,
                        format!("EglContext: eglDestroySurface failed: {err:?}"),
                    )
                })?;
            self.surface_destroyed = true;
        }
        if !self.display_terminated {
            self.egl.terminate(self.display).map_err(|err| {
                Error::new(
                    Errc::PlatformError,
                    format!("EglContext: eglTerminate failed: {err:?}"),
                )
            })?;
            self.display_terminated = true;
        }
        if !self.egl_window.is_null() {
            unsafe {
                wl_egl_window_destroy(self.egl_window);
            }
            self.egl_window = ptr::null_mut();
        }
        self.shutdown = true;
        Ok(())
    }

    // 直接更新 EGL surface、Wayland window 和 OpenGL RHI 的 drawable 状态。
    fn resize_surface_extent(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // 相同物理尺寸无需重复触碰 Wayland 或推进 surface generation。
        if width == self.width && height == self.height {
            return Ok(());
        }
        // 保存新的物理 surface 尺寸。
        self.width = width;
        self.height = height;
        // 通知 Wayland EGL window 更新其 native buffer 尺寸。
        if !self.egl_window.is_null() {
            unsafe {
                wl_egl_window_resize(self.egl_window, width, height, 0, 0);
            }
        }
        // 把尺寸事实同步到共享 OpenGL RHI pipeline。
        self.pipeline.resize_swapchain(width, height, width, height);
        // resize 成功后推进 surface generation，隔离旧 FramePlan。
        self.surface_generation = self.surface_generation.saturating_add(1);
        // 原生 EGL surface resize 已经完成。
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IGraphicsContext impl
// ════════════════════════════════════════════════════════════════════════════

impl IGraphicsContext for EglContext {
    fn caps(&self) -> crate::native::present::GraphicsContextCaps {
        crate::native::present::GraphicsContextCaps::gpu_native_swapchain(
            crate::native::present::GraphicsApi::OpenGlEs,
            PresentCoherency::FullOnly,
            1.0,
        )
    }

    // 暴露同一 owner-thread context 上的 OpenGL ES 薄 RHI 组合视图。
    fn rhi_context(&mut self) -> Option<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
        // EGL adapter 已经实现共享 GraphicsDevice/GraphicsSurface。
        Some(self)
    }

    fn graphics_backend(&self) -> crate::native::present::GraphicsApi {
        crate::native::present::GraphicsApi::OpenGlEs
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        // EGL 只公布固定 probe 已验证的 retained 与 Additive 事实。
        NativeRasterCaps::retained_rhi_with_additive()
    }

    fn initialize(
        &mut self,
        _native_window: *mut c_void,
        _width: i32,
        _height: i32,
    ) -> Result<(), Error> {
        // EglContext 在 new() 中初始化完毕；initialize 表达 trait 生命周期入口。
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.make_current()?;
        self.resize_surface_extent(width, height)
    }

    fn make_current(&mut self) -> Result<(), Error> {
        self.egl
            .make_current(
                self.display,
                Some(self.surface),
                Some(self.surface),
                Some(self.context),
            )
            .map_err(|err| {
                Error::new(
                    Errc::PlatformError,
                    format!("EglContext: eglMakeCurrent failed: {err:?}"),
                )
            })
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.shutdown_result()
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>, Error> {
        self.make_current()?;
        self.pipeline.read_pixels(x, y, width, height)
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    // 统一 present 入口显式区分 EGL swapchain 与不支持的像素上传载荷。
    fn present(&mut self, frame: &PresentFrame<'_>) -> Result<(), Error> {
        // 按统一 payload 选择 EGL 能执行的提交协议。
        match frame {
            // swapchain 提交先恢复 owner-thread current context。
            PresentFrame::Swapchain { damage } => {
                // 保持与旧默认入口相同的 current 前置条件。
                self.make_current()?;
                // 统一 presenter 也必须消费共享 OpenGL lower surface-lost 注入。
                #[cfg(feature = "test-harness")]
                if self.pipeline.rhi_take_surface_lost_for_test() {
                    // 保留与共享 RHI surface host 相同的故障 marker。
                    tracing::warn!("OpenGL RHI test surface lost");
                    // 返回可由恢复 FSM 分类的 surface-lost 错误。
                    return Err(Error::new(
                        // 标记 surface 需要重建而不是重试同一帧。
                        Errc::GraphicsSurfaceLost,
                        // 保留稳定的测试与诊断文本。
                        "OpenGL RHI test surface lost before present",
                    ));
                }
                // 完整与局部 damage 当前都只作为语义输入，不启用未验证扩展。
                match damage {
                    // 完整 damage 直接交换当前 EGL window surface。
                    PresentDamage::Full => self
                        // 调用 EGL owner 的交换入口。
                        .egl
                        // 提交当前 display/surface 组合。
                        .swap_buffers(self.display, self.surface)
                        // 把原生错误映射为统一图形错误。
                        .map_err(map_egl_swap_error),
                    // 局部 damage 不能宣称未验证的 buffer preservation。
                    PresentDamage::Partial(_) => self
                        // 继续使用同一个 EGL owner。
                        .egl
                        // 以完整交换保持像素语义正确。
                        .swap_buffers(self.display, self.surface)
                        // 保留与完整交换相同的错误分类。
                        .map_err(map_egl_swap_error),
                }
            }
            // EGL GPU-native recipe 不接受 CPU pixel buffer。
            PresentFrame::PixelBuffer { .. } => Err(Error::new(
                // payload 与已选 recipe 不匹配，使用稳定参数错误。
                Errc::InvalidArgument,
                // 明确阻止调用方退回隐式上传分叉。
                "EglContext: CPU pixel present is unsupported",
            )),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Drop — 确保 GPU 资源释放
// ════════════════════════════════════════════════════════════════════════════

// 将 EGL 原生生命周期接入共享 OpenGL RHI host。
impl OpenGlRhiHost for EglContext {
    // 借用可变 raster/RHI owner。
    fn rhi_pipeline_mut(&mut self) -> &mut OpenGlRasterPipeline {
        &mut self.pipeline
    }

    // 借用只读 raster/RHI owner。
    fn rhi_pipeline(&self) -> &OpenGlRasterPipeline {
        &self.pipeline
    }

    // 切换到 EGL owner-thread context。
    fn rhi_make_current(&mut self) -> Result<(), Error> {
        <Self as IGraphicsContext>::make_current(self)
    }

    // 返回 EGL surface generation。
    fn rhi_generation(&self) -> u64 {
        self.surface_generation
    }

    // 按物理 extent 进入 EGL 原生 surface resize helper。
    fn rhi_resize_surface(&mut self, extent: RhiExtent) -> Result<(), Error> {
        if self.pipeline.rhi_surface_extent() == extent {
            return Ok(());
        }
        self.rhi_make_current()?;
        self.resize_surface_extent(extent.width as i32, extent.height as i32)
    }

    // 交换 EGL window surface。
    fn rhi_swap_buffers(&mut self, _damage: PresentDamage) -> Result<(), Error> {
        self.egl
            .swap_buffers(self.display, self.surface)
            .map_err(map_egl_swap_error)
    }
}

// 释放 EGL owner-thread 的所有 GPU 资源。
impl Drop for EglContext {
    fn drop(&mut self) {
        let _ = self.try_shutdown();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Thread affinity
//
// EGL contexts remain on their creating event-loop thread; no Send/Sync
// marker may make them transferable.
// ════════════════════════════════════════════════════════════════════════════
