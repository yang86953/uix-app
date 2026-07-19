// ============================================================================
// native/graphics/opengl/egl.rs — EGL + GLES 3.0 图形上下文
//
// 通过 EGL 创建 OpenGL ES 3.0 上下文，对接 Wayland surface（wl_egl_window）。
// 实现 IGraphicsContext trait，供 GpuEngine 使用。
//
// 依赖 khronos-egl v6 (static 链接) + wayland-egl (系统库 FFI)。
// ============================================================================

use std::ffi::c_void;
use std::ptr;

use crate::native::graphics::opengl::raster::OpenGlRasterPipeline;
use crate::native::traits::present::{
    GpuGlyphBlit, GpuSolidRect, IGraphicsContext, NativeRasterCaps, OffscreenTargetId,
    PresentCoherency, PresentDamage, SoftFallbackTile,
};
use crate::native::{Errc, Error};

use crate::native::graphics::platform::linux::WaylandSurfaceHandle;
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
        crate::core::log::info_fn(format!("EglContext: EGL {major}.{minor}"));

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
        crate::core::log::info_fn("EglContext: GLES 3.0 上下文创建成功");

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

        let runtime = crate::native::graphics::opengl::NativeOpenGlRuntime::from_loader(|name| {
            egl.get_proc_address(name)
                .map(|function| function as *const std::ffi::c_void)
                .unwrap_or(std::ptr::null())
        });
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
}

// ════════════════════════════════════════════════════════════════════════════
// IGraphicsContext impl
// ════════════════════════════════════════════════════════════════════════════

impl IGraphicsContext for EglContext {
    fn caps(&self) -> crate::native::traits::present::GraphicsContextCaps {
        crate::native::traits::present::GraphicsContextCaps::gpu_native_swapchain(
            crate::native::traits::present::GraphicsBackend::OpenGlEs,
            PresentCoherency::FullOnly,
            1.0,
        )
    }

    fn graphics_backend(&self) -> crate::native::traits::present::GraphicsBackend {
        crate::native::traits::present::GraphicsBackend::OpenGlEs
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
        if width == self.width && height == self.height {
            return Ok(());
        }
        self.width = width;
        self.height = height;
        if !self.egl_window.is_null() {
            unsafe {
                wl_egl_window_resize(self.egl_window, width, height, 0, 0);
            }
        }
        self.pipeline.resize_swapchain(width, height, width, height);
        Ok(())
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

    fn swap_buffers(&mut self, damage: PresentDamage) -> Result<(), Error> {
        match damage {
            PresentDamage::Full => {
                self.egl
                    .swap_buffers(self.display, self.surface)
                    .map_err(|err| {
                        Error::new(
                            Errc::PlatformError,
                            format!("EglContext: eglSwapBuffers failed: {err:?}"),
                        )
                    })
            }
            PresentDamage::Partial(_) => {
                // `EGL_KHR_swap_buffers_with_damage` is only a compositor hint.  Until
                // buffer preservation and age are verified, a partial input must not
                // choose an untyped extension ABI or claim partial-redraw semantics.
                self.egl
                    .swap_buffers(self.display, self.surface)
                    .map_err(|err| {
                        Error::new(
                            Errc::PlatformError,
                            format!("EglContext: eglSwapBuffers failed: {err:?}"),
                        )
                    })
            }
        }
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

    fn clear_render_target(&mut self, r: f32, g: f32, b: f32, a: f32) -> Result<(), Error> {
        self.make_current()?;
        self.pipeline.clear_render_target([r, g, b, a])
    }

    fn draw_solid_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<(), Error> {
        self.make_current()?;
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
        self.make_current()?;
        self.pipeline
            .draw_glyphs(viewport_w, viewport_h, scissor, glyphs)
    }

    fn blit_soft_fallback_tile(
        &mut self,
        pixels: &[u32],
        tile: SoftFallbackTile,
    ) -> Result<(), Error> {
        self.make_current()?;
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
        self.make_current()?;
        self.pipeline.upload_surface_pixels(pixels, width, height)
    }

    fn clear_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        rects: &[GpuSolidRect],
    ) -> Result<(), Error> {
        self.make_current()?;
        self.pipeline.clear_rects(rects)
    }

    fn create_offscreen_target(
        &mut self,
        width: i32,
        height: i32,
    ) -> Result<OffscreenTargetId, Error> {
        self.make_current()?;
        self.pipeline.create_offscreen_target(width, height)
    }

    fn try_destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        self.make_current()?;
        self.pipeline.destroy_offscreen_target(id)
    }

    fn bind_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        self.make_current()?;
        self.pipeline.bind_offscreen_target(id)
    }

    fn bind_swapchain_target(&mut self) -> Result<(), Error> {
        self.make_current()?;
        self.pipeline.bind_swapchain_target();
        Ok(())
    }

    fn blit_offscreen_target(
        &mut self,
        id: OffscreenTargetId,
        src: crate::core::Rect,
        dst: crate::core::Rect,
    ) -> Result<(), Error> {
        self.make_current()?;
        self.pipeline.blit_offscreen_target(id, src, dst)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Drop — 确保 GPU 资源释放
// ════════════════════════════════════════════════════════════════════════════

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
