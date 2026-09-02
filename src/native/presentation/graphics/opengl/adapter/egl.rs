// ============================================================================
// native/graphics/opengl/egl.rs — EGL + GLES 3.0 图形上下文
//
// 通过 EGL 创建 OpenGL ES 3.0 上下文，对接 Wayland surface（wl_egl_window）。
// 实现类型化 GPU recipe trait，供唯一 Renderer 的 GPU 后端使用。
//
// 依赖 khronos-egl v6 (static 链接) + wayland-egl (系统库 FFI)。
// ============================================================================

// 在 Rust 2024 下禁止 unsafe 函数体隐式扩大底层操作范围。
#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;
use std::ptr;
// EGL context 与 Wayland windowing 共享逐窗 surface metrics owner。
use std::sync::Arc;

// 引入共享的 OpenGL RHI host 生命周期实现。
use crate::native::presentation::graphics::opengl::raster::OpenGlRasterPipeline;
// 引入 Drop 中调用的 checked shutdown 生命周期契约。
use crate::native::{Errc, Error};
use crate::platform::presentation::GraphicsContextLifecycle;
// 引入唯一共享 Surface 生命周期与初始化原因。
use crate::platform::presentation::rhi::{
    RhiExtent, RhiSurfaceLifecycle, RhiSurfaceRecreateReason,
};

use crate::native::presentation::graphics::platform::linux::{
    WaylandSurfaceHandle, WaylandSurfaceMetrics,
};

// 将 EGL 的 RHI 与 recipe 生命周期实现拆到独立平台组件。
#[path = "egl_rhi.rs"]
mod egl_rhi;

// 将 EGL 交换错误映射为恢复 FSM 可消费的 surface/device typed failure。
fn map_egl_surface_error(operation: &str, error: khronos_egl::Error) -> Error {
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
    Error::new(code, format!("EglContext: {operation} failed: {error:?}"))
}

// 将交换错误投影到共享恢复 FSM 使用的稳定分类。
fn map_egl_swap_error(error: khronos_egl::Error) -> Error {
    map_egl_surface_error("eglSwapBuffers", error)
}

// EGL window surface 的生产交换节拍由 adapter 唯一固定，上层不感知 EGL 策略。
const EGL_PRODUCTION_SWAP_INTERVAL: i32 = 1;

// 对每个新建或替换后的 current window surface 应用同一生产节拍策略。
fn apply_production_swap_interval(
    egl: &khronos_egl::Instance<khronos_egl::Static>,
    display: khronos_egl::Display,
) -> Result<(), Error> {
    egl.swap_interval(display, EGL_PRODUCTION_SWAP_INTERVAL)
        .map_err(|error| {
            Error::new(
                Errc::PlatformError,
                format!(
                    "EglContext: production eglSwapInterval({EGL_PRODUCTION_SWAP_INTERVAL}) failed: {error:?}"
                ),
            )
        })
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
// SAFETY：外部 FFI 声明，调用方必须保证传入的 surface 指针与尺寸参数有效。
unsafe extern "C" {
    fn wl_egl_window_create(surface: *mut c_void, width: i32, height: i32) -> *mut WlEglWindow;

    fn wl_egl_window_destroy(window: *mut WlEglWindow);

    fn wl_egl_window_resize(window: *mut WlEglWindow, width: i32, height: i32, dx: i32, dy: i32);
}

// khronos-egl v6 的 static 绑定不携带 #[link]；项目刻意使用 no-pkg-config，
// 因此这里显式声明系统 libEGL 链接，保证 egl* 符号进入最终链接。
#[link(name = "EGL")]
// SAFETY: 该空声明只请求链接系统 libEGL，不声明可被 Rust 直接调用的符号。
unsafe extern "C" {}

// 在 EglContext 成功交付前唯一持有构造期 native 资源，并负责失败回滚。
struct PendingEglContext<'a> {
    // 借用同一构造事务的 EGL API 实例，不取得第二份资源所有权。
    egl: &'a khronos_egl::Instance<khronos_egl::Static>,
    // 保存已经成功初始化且尚未终止的 display。
    display: khronos_egl::Display,
    // 只在创建成功后登记尚未交付的 context。
    context: Option<khronos_egl::Context>,
    // 只在创建成功后登记尚未交付的 surface。
    surface: Option<khronos_egl::Surface>,
    // 保存尚未交付的 Wayland EGL window。
    egl_window: *mut WlEglWindow,
    // 记录构造期 context 是否已经成为当前线程的 current context。
    current: bool,
    // 区分仍需回滚与已经成功移交的事务状态。
    armed: bool,
}

impl<'a> PendingEglContext<'a> {
    // 从已经成功初始化的 display 建立构造期唯一 owner。
    fn new(
        // 借用创建 display 的同一 EGL API 实例。
        egl: &'a khronos_egl::Instance<khronos_egl::Static>,
        // 接管已初始化 display 的构造期 teardown 责任。
        display: khronos_egl::Display,
    ) -> Self {
        // 其余资源尚未创建，按空 owner 状态初始化。
        Self {
            // 保存 EGL API 借用。
            egl,
            // 保存 display 句柄。
            display,
            // context 尚未创建。
            context: None,
            // surface 尚未创建。
            surface: None,
            // Wayland EGL window 尚未创建。
            egl_window: ptr::null_mut(),
            // 尚无 current context。
            current: false,
            // display 已初始化，因此 guard 立即进入 armed 状态。
            armed: true,
        }
    }

    // 创建失败时执行一次检查式回滚，并保留主错误与清理错误链。
    fn finish_failure(&mut self, primary_error: Error) -> Error {
        // 清理成功时仍传播触发回滚的原始初始化错误。
        match self.rollback_result() {
            // 所有构造期资源已释放。
            Ok(()) => primary_error,
            // 清理失败成为外层错误，原初始化错误保留为 source。
            Err(cleanup_error) => cleanup_error.with_source(primary_error),
        }
    }

    // 成功创建后原子移交 context、surface 与 Wayland EGL window。
    fn into_handles(
        // 消费构造期唯一 owner，防止移交后继续使用。
        mut self,
    ) -> (
        // 返回正式 context 句柄。
        khronos_egl::Context,
        // 返回正式 surface 句柄。
        khronos_egl::Surface,
        // 返回正式 Wayland EGL window。
        *mut WlEglWindow,
    ) {
        // 成功路径必须已经登记 context。
        let context = self
            // 从临时 owner 取出 context。
            .context
            // context 缺失表示构造事务内部不变量被破坏。
            .take()
            // 该错误只可能是开发期接线错误，因此使用明确断言信息。
            .expect("EGL construction guard must own a context before handoff");
        // 成功路径必须已经登记 surface。
        let surface = self
            // 从临时 owner 取出 surface。
            .surface
            // surface 缺失表示构造事务内部不变量被破坏。
            .take()
            // 该错误只可能是开发期接线错误，因此使用明确断言信息。
            .expect("EGL construction guard must own a surface before handoff");
        // 把 Wayland EGL window 从临时 owner 移出。
        let egl_window = std::mem::replace(&mut self.egl_window, ptr::null_mut());
        // 显式解除 guard，Drop 不得终止已交付 display。
        self.armed = false;
        // current 绑定状态随句柄一起转移给正式 EglContext。
        self.current = false;
        // 返回由 EglContext 字段接管的三个句柄。
        (context, surface, egl_window)
    }

    // 按 current → context → surface → display → native window 逆序检查式回滚。
    fn rollback_result(&mut self) -> Result<(), Error> {
        // 已完成回滚或成功移交时保持幂等成功。
        if !self.armed {
            // 禁止重复触碰已释放或已交付资源。
            return Ok(());
        }
        // 只有成功绑定过的 context 才需要先解除 current 状态。
        if self.current {
            // 解除 draw/read/current 三个绑定，失败时保留全部 owner 状态供 Drop 重试。
            self.egl
                // 空 surface/context 表示解除当前线程的 EGL 绑定。
                .make_current(self.display, None, None, None)
                // 映射为框架稳定的 typed teardown error。
                .map_err(|error| {
                    // 保留失败操作与 EGL 枚举。
                    Error::new(
                        // 构造回滚失败属于平台资源错误。
                        Errc::PlatformError,
                        // 记录精确 native 操作。
                        format!(
                            "EglContext: construction rollback eglMakeCurrent(NULL) failed: {error:?}"
                        ),
                    )
                })?;
            // 只有解绑成功后才清除 current owner 状态。
            self.current = false;
        }
        // context 必须在 surface 与 display 之前删除。
        if let Some(context) = self.context {
            // 删除仍由 guard 唯一持有的 context。
            self.egl
                // context 与 display 来自同一 EGL 实例。
                .destroy_context(self.display, context)
                // 映射为框架稳定的 typed teardown error。
                .map_err(|error| {
                    // 保留失败操作与 EGL 枚举。
                    Error::new(
                        // 构造回滚失败属于平台资源错误。
                        Errc::PlatformError,
                        // 记录精确 native 操作。
                        format!(
                            "EglContext: construction rollback eglDestroyContext failed: {error:?}"
                        ),
                    )
                })?;
            // 只有删除成功后才清空 context owner 槽位。
            self.context = None;
        }
        // surface 必须在 display 终止前删除。
        if let Some(surface) = self.surface {
            // 删除仍由 guard 唯一持有的 surface。
            self.egl
                // surface 与 display 来自同一 EGL 实例。
                .destroy_surface(self.display, surface)
                // 映射为框架稳定的 typed teardown error。
                .map_err(|error| {
                    // 保留失败操作与 EGL 枚举。
                    Error::new(
                        // 构造回滚失败属于平台资源错误。
                        Errc::PlatformError,
                        // 记录精确 native 操作。
                        format!(
                            "EglContext: construction rollback eglDestroySurface failed: {error:?}"
                        ),
                    )
                })?;
            // 只有删除成功后才清空 surface owner 槽位。
            self.surface = None;
        }
        // display 初始化成功后必须由同一 guard 终止。
        self.egl
            // 终止当前构造事务初始化的 display。
            .terminate(self.display)
            // 映射为框架稳定的 typed teardown error。
            .map_err(|error| {
                // 保留失败操作与 EGL 枚举。
                Error::new(
                    // 构造回滚失败属于平台资源错误。
                    Errc::PlatformError,
                    // 记录精确 native 操作。
                    format!("EglContext: construction rollback eglTerminate failed: {error:?}"),
                )
            })?;
        // EGL display 终止后再销毁唯一持有的 Wayland native window。
        if !self.egl_window.is_null() {
            // SAFETY: 非空 window 由本 guard 唯一持有，surface 已删除且 display 已终止。
            unsafe {
                // wayland-egl 销毁 API 无失败返回值，只调用一次。
                wl_egl_window_destroy(self.egl_window);
            }
            // native window 销毁后立即清空 owner 槽位。
            self.egl_window = ptr::null_mut();
        }
        // 所有构造期资源均已完成回滚。
        self.armed = false;
        // 向调用方确认检查式清理成功。
        Ok(())
    }
}

impl Drop for PendingEglContext<'_> {
    fn drop(&mut self) {
        // Drop 只重试尚未完成的检查式回滚，失败必须留下最终诊断。
        if let Err(error) = self.rollback_result() {
            // 创建期回滚失败经边界观察入口记录，便于定位泄漏。
            crate::diagnostics::observe_boundary_error("egl/rollback", &error);
        }
    }
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
    config: khronos_egl::Config,
    context: khronos_egl::Context,
    surface: khronos_egl::Surface,
    egl_window: *mut WlEglWindow,
    // 当前 EGL drawable 的物理宽度。
    width: i32,
    // 当前 EGL drawable 的物理高度。
    height: i32,
    // 当前窗口协议使用的逻辑宽度。
    logical_width: i32,
    // 当前窗口协议使用的逻辑高度。
    logical_height: i32,
    // 已实际应用到 EGL drawable 的 Wayland 整数 scale。
    device_pixel_ratio: f32,
    // windowing 与 EGL 共享的逐窗 surface 元数据。
    metrics: Arc<WaylandSurfaceMetrics>,
    // 唯一拥有 Surface generation、extent 与重建事务顺序的共享状态机。
    surface_lifecycle: RhiSurfaceLifecycle,
    // 显式测试只记录成功替换真实 EGLSurface 的次数，不参与权威状态。
    #[cfg(feature = "test-harness")]
    window_surface_replacements_for_test: u64,
    pipeline: OpenGlRasterPipeline,
    // 关闭事务一旦开始便禁止新的业务 RHI 借用，但允许 cleanup 重试。
    shutdown_started: bool,
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

        // SAFETY: native_surface 来自平台 surface recipe，在本构造调用期间指向存活的 WaylandSurfaceHandle。
        let wayland = unsafe { WaylandSurfaceHandle::from_native(native_surface)? };
        // 读取当前 scale，再用构造参数提交初始 logical extent。
        let current_surface = wayland.metrics.snapshot()?;
        // 一次生成初始 logical/drawable/DPR 同代快照。
        let initial_surface = wayland.metrics.update(
            // graphics factory 传入窗口逻辑宽度。
            width.max(1),
            // graphics factory 传入窗口逻辑高度。
            height.max(1),
            // 保留 output registry 已发布的整数 scale。
            current_surface.scale,
        )?;
        // 在任何 EGL Surface 原生创建前冻结初始化事务。
        let initial_extent = RhiExtent::new(
            initial_surface.drawable_width as u32,
            initial_surface.drawable_height as u32,
        );
        let mut surface_lifecycle = RhiSurfaceLifecycle::uninitialized(initial_extent);
        let surface_initialize = surface_lifecycle
            .begin_recreate(initial_extent, RhiSurfaceRecreateReason::Initialize)?;
        let egl = egl::Instance::new(egl::Static);

        // 1. 获取 display —— Wayland 下传入 display 连接指针
        // SAFETY: wayland.display 已由 WaylandSurfaceHandle 校验非空，并在 EGL 上下文生命周期内保持连接存活。
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
        // display 初始化成功后立即建立构造期唯一资源 owner。
        let mut pending = PendingEglContext::new(&egl, display);

        // 3. 绑定 API 到 OpenGL ES
        if let Err(e) = egl.bind_api(egl::OPENGL_ES_API) {
            // 先构造触发回滚的原始 EGL 初始化错误。
            let primary_error = Error::new(
                // API 绑定失败属于平台错误。
                Errc::PlatformError,
                // 保留原始 EGL 枚举。
                format!("EglContext: eglBindAPI 失败: {e:?}"),
            );
            // 由唯一 guard 检查式终止 display，并保留双错误链。
            return Err(pending.finish_failure(primary_error));
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
                // 与共享 PipelineMultisampleState::SingleSample 保持一致。
                egl::SAMPLE_BUFFERS,
                0,
                // 禁止驱动为窗口 surface 私自选择多样本 config。
                egl::SAMPLES,
                0,
                egl::NONE,
            ];
            egl.choose_first_config(display, &config_attribs)
        };

        let config = match choose_config(egl::OPENGL_ES3_BIT) {
            Ok(Some(config)) => config,
            Ok(None) => {
                // 缺少 GLES 3 config 是触发回滚的原始能力错误。
                let primary_error = Error::new(
                    // 保持既有未实现分类。
                    Errc::NotImplemented,
                    // 保持既有产品错误说明。
                    "EglContext: native raster requires a GLES 3 EGL config",
                );
                // 由唯一 guard 检查式终止 display。
                return Err(pending.finish_failure(primary_error));
            }
            Err(e) => {
                // 配置查询失败是触发回滚的原始 EGL 错误。
                let primary_error = Error::new(
                    // 配置查询失败属于平台错误。
                    Errc::PlatformError,
                    // 保留原始 EGL 枚举。
                    format!("EglContext: GLES 3 choose_config 失败: {e:?}"),
                );
                // 由唯一 guard 检查式终止 display。
                return Err(pending.finish_failure(primary_error));
            }
        };

        // 5. 创建 wl_egl_window（Wayland 原生窗口封装）
        // SAFETY: wayland.surface 已校验非空且仍由平台窗口拥有，尺寸来自受检物理 drawable 快照。
        let egl_window = unsafe {
            wl_egl_window_create(
                // 传入当前窗口的 wl_surface 指针。
                wayland.surface,
                // wl_egl_window 使用物理 buffer 宽度。
                initial_surface.drawable_width,
                // wl_egl_window 使用物理 buffer 高度。
                initial_surface.drawable_height,
            )
        };
        if egl_window.is_null() {
            // native window 创建失败是触发回滚的原始平台错误。
            let primary_error = Error::new(
                // 空指针结果属于平台资源失败。
                Errc::PlatformError,
                // 保持既有产品错误说明。
                "EglContext: wl_egl_window_create 返回 null",
            );
            // 由唯一 guard 检查式终止 display。
            return Err(pending.finish_failure(primary_error));
        }
        // native window 创建成功后立即登记到唯一构造期 owner。
        pending.egl_window = egl_window;

        // 6. 创建 EGL surface
        // SAFETY: display/config 已由同一 EGL 实例创建，egl_window 非空且在调用期间保持存活。
        let surface_result = unsafe {
            egl.create_window_surface(display, config, egl_window as egl::NativeWindowType, None)
        };
        // surface 创建失败时由 guard 统一清理 native window 与 display。
        let surface = match surface_result {
            // 成功值尚未交付给正式 context。
            Ok(surface) => surface,
            // 失败值触发构造事务回滚。
            Err(e) => {
                // 构造原始 EGL surface 错误。
                let primary_error = Error::new(
                    // surface 创建失败属于平台错误。
                    Errc::PlatformError,
                    // 保留原始 EGL 枚举。
                    format!("EglContext: eglCreateWindowSurface 失败: {e:?}"),
                );
                // 由唯一 guard 检查式回滚已登记资源。
                return Err(pending.finish_failure(primary_error));
            }
        };
        // surface 创建成功后立即登记到唯一构造期 owner。
        pending.surface = Some(surface);

        // 7. 创建 GLES 3.0 上下文；没有等价 ES2 pipeline 时不得降级。
        let ctx3_attribs = [
            egl::CONTEXT_MAJOR_VERSION,
            3,
            egl::CONTEXT_MINOR_VERSION,
            0,
            egl::NONE,
        ];
        // 创建结果在登记到 guard 前不得离开本构造事务。
        let context_result = egl.create_context(display, config, None, &ctx3_attribs);
        // context 创建失败时由 guard 统一清理 surface、native window 与 display。
        let context = match context_result {
            // 成功值尚未交付给正式 context。
            Ok(context) => context,
            // 失败值触发构造事务回滚。
            Err(e) => {
                // 构造原始 GLES 3 能力错误。
                let primary_error = Error::new(
                    // 保持既有未实现分类。
                    Errc::NotImplemented,
                    // 保留原始 EGL 枚举。
                    format!("EglContext: native raster requires GLES 3.0: {e:?}"),
                );
                // 由唯一 guard 检查式回滚已登记资源。
                return Err(pending.finish_failure(primary_error));
            }
        };
        // context 创建成功后立即登记到唯一构造期 owner。
        pending.context = Some(context);
        tracing::info!("EglContext: GLES 3.0 上下文创建成功");

        // 8. make current
        // 把刚创建的 context 同时绑定为 draw/read current。
        if let Err(e) = egl.make_current(display, Some(surface), Some(surface), Some(context)) {
            // 构造原始 current 绑定错误。
            let primary_error = Error::new(
                // current 绑定失败属于平台错误。
                Errc::PlatformError,
                // 保留原始 EGL 枚举。
                format!("EglContext: eglMakeCurrent 失败: {e:?}"),
            );
            // 由唯一 guard 检查式删除 context、surface、display 与 native window。
            return Err(pending.finish_failure(primary_error));
        }
        // 只在 EGL 绑定成功后登记 current 状态。
        pending.current = true;

        // 生产窗口明确使用非零交换节拍；失败仍由构造期 owner 完整回滚。
        if let Err(error) = apply_production_swap_interval(&egl, display) {
            return Err(pending.finish_failure(error));
        }

        let runtime =
            crate::native::presentation::graphics::opengl::NativeOpenGlRuntime::from_loader(
                |name| {
                    egl.get_proc_address(name)
                        .map(|function| function as *const std::ffi::c_void)
                        .unwrap_or(std::ptr::null())
                },
            );
        // pipeline 初始化失败时同样由构造期唯一 owner 完整回滚 native 资源。
        let mut pipeline = match OpenGlRasterPipeline::new(
            // 传入已加载的 GLES runtime。
            runtime,
            // pipeline 场景保持逻辑宽度。
            initial_surface.logical_width,
            // pipeline 场景保持逻辑高度。
            initial_surface.logical_height,
            // swapchain 使用物理 drawable 宽度。
            initial_surface.drawable_width,
            // swapchain 使用物理 drawable 高度。
            initial_surface.drawable_height,
        ) {
            // 成功 pipeline 将与 native 句柄一起交付 EglContext。
            Ok(pipeline) => pipeline,
            // 失败时保留 pipeline 初始化错误与 native 清理错误链。
            Err(error) => {
                // 由唯一 guard 先解绑再逆序释放所有已登记资源。
                return Err(pending.finish_failure(error));
            }
        };

        // 原生 Surface 与 pipeline 都成功后一次发布初始 generation 和 extent。
        if let Err(error) = surface_lifecycle.commit_recreate(surface_initialize, initial_extent) {
            // 生命周期提交失败时仍在 current context 上检查式释放 pipeline 资源。
            pipeline.release();
            // 构造期 owner 继续逆序回滚全部 EGL 对象。
            return Err(pending.finish_failure(error));
        }

        // 所有创建步骤成功后才把 native 句柄从 guard 移交给正式 context。
        let (context, surface, egl_window) = pending.into_handles();

        Ok(Self {
            egl,
            display,
            config,
            context,
            surface,
            egl_window,
            // 保存初始物理 drawable 宽度。
            width: initial_surface.drawable_width,
            // 保存初始物理 drawable 高度。
            height: initial_surface.drawable_height,
            // 保存初始逻辑宽度。
            logical_width: initial_surface.logical_width,
            // 保存初始逻辑高度。
            logical_height: initial_surface.logical_height,
            // 保存已应用的初始整数 DPR。
            device_pixel_ratio: initial_surface.scale as f32,
            // 接管 descriptor 克隆的共享 metrics owner。
            metrics: wayland.metrics,
            // 接管已经完成 Initialize 事务的共享生命周期 owner。
            surface_lifecycle,
            // 初始 EGLSurface 属于构造，不计入失效后的 replacement。
            #[cfg(feature = "test-harness")]
            window_surface_replacements_for_test: 0,
            pipeline,
            // 初始 owner 尚未进入关闭事务。
            shutdown_started: false,
            shutdown: false,
            context_destroyed: false,
            surface_destroyed: false,
            display_terminated: false,
        })
    }

    // 返回当前真实 Wayland EGL window context 的 EGL 与 GPU 身份。
    #[cfg(feature = "opengl-parity-test")]
    pub(crate) fn parity_adapter_diagnostic(&self) -> Result<String, Error> {
        use khronos_egl as egl;

        // 诊断先服从正式 owner 门禁并恢复 current context。
        self.make_current_result()?;
        // 从当前 display 查询 EGL 实现身份，不使用 headless 或推断值。
        let egl_vendor = self
            .egl
            .query_string(Some(self.display), egl::VENDOR)
            .map_err(|error| map_egl_surface_error("eglQueryString(EGL_VENDOR)", error))?
            .to_string_lossy();
        let egl_version = self
            .egl
            .query_string(Some(self.display), egl::VERSION)
            .map_err(|error| map_egl_surface_error("eglQueryString(EGL_VERSION)", error))?
            .to_string_lossy();
        // GPU 字符串来自同一个 current window context 的 GLES runtime。
        Ok(format!(
            "EGL vendor={egl_vendor}; version={egl_version}; swap-interval={EGL_PRODUCTION_SWAP_INTERVAL} (explicit production); {}",
            self.pipeline.parity_gpu_diagnostic(),
        ))
    }

    // 返回成功替换真实 EGLSurface 的 feature-only 观察计数。
    #[cfg(feature = "test-harness")]
    pub(crate) const fn window_surface_replacements_for_test(&self) -> u64 {
        self.window_surface_replacements_for_test
    }

    pub(crate) fn shutdown_result(&mut self) -> Result<(), Error> {
        // 已完整关闭的 owner 保持幂等返回，不重复触碰 native 句柄。
        if self.shutdown {
            return Ok(());
        }
        // 在任何 native cleanup 或 pipeline release 前发布关闭事实。
        self.shutdown_started = true;
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
            // SAFETY: 非空 egl_window 由当前 EglContext 唯一拥有，shutdown 只在置空前调用一次销毁。
            unsafe {
                wl_egl_window_destroy(self.egl_window);
            }
            self.egl_window = ptr::null_mut();
        }
        self.shutdown = true;
        Ok(())
    }

    // 在 EGL adapter 内部恢复 owner-thread 的原生 current context。
    fn make_current_result(&self) -> Result<(), Error> {
        // 在任何 EGL 调用前拒绝已开始关闭或句柄不完整的 owner。
        self.ensure_rhi_active()?;
        // 委托给 EGL 实例并保留 typed platform error。
        self.egl
            // 同时绑定 draw 与 read surface，供 RHI device 和 present 共用。
            .make_current(
                // 使用构造阶段验证过的 display。
                self.display,
                // 绑定当前 window draw surface。
                Some(self.surface),
                // 绑定同一 window read surface。
                Some(self.surface),
                // 恢复当前 context。
                Some(self.context),
            )
            // 把原生错误收敛为统一平台错误。
            .map_err(|error| map_egl_surface_error("eglMakeCurrent", error))
    }

    // 检查 EGL owner 是否仍可被业务 RHI 使用。
    pub(crate) fn ensure_rhi_active(&self) -> Result<(), Error> {
        // 关闭事务、原生对象销毁或 display/window 失效都统一为 InvalidState。
        if self.shutdown_started
            || self.shutdown
            || self.context_destroyed
            || self.surface_destroyed
            || self.display_terminated
            || self.egl_window.is_null()
        {
            // 在触碰任何 EGL API 前返回稳定生命周期错误。
            return Err(Error::new(
                Errc::InvalidState,
                "EglContext: operation requested after shutdown",
            ));
        }
        // owner 的 EGL context、surface、display 与 window 仍完整存活。
        Ok(())
    }

    // 直接更新 EGL surface、Wayland window 和 OpenGL RHI 的 drawable 状态。
    fn resize_surface_extent(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // 读取 windowing 已原子发布的最新 logical/drawable/DPR 快照。
        let snapshot = self.metrics.snapshot()?;
        // 相同逻辑尺寸、物理尺寸与 DPR 无需重复触碰 EGL 或推进代次。
        if width == self.width
            && height == self.height
            && snapshot.logical_width == self.logical_width
            && snapshot.logical_height == self.logical_height
            && snapshot.scale as f32 == self.device_pixel_ratio
        {
            // 当前 EGL drawable 已满足请求。
            return Ok(());
        }
        // 记录物理尺寸是否需要调用 wayland-egl resize。
        let physical_changed = width != self.width || height != self.height;
        // 保存新的物理 surface 尺寸。
        self.width = width;
        // 保存新的物理 surface 高度。
        self.height = height;
        // 保存同代逻辑宽度。
        self.logical_width = snapshot.logical_width;
        // 保存同代逻辑高度。
        self.logical_height = snapshot.logical_height;
        // 保存已应用到 surface 的整数 DPR。
        self.device_pixel_ratio = snapshot.scale as f32;
        // 通知 Wayland EGL window 更新其 native buffer 尺寸。
        if physical_changed && !self.egl_window.is_null() {
            // SAFETY: 非空 egl_window 仍由当前 EglContext 拥有，尺寸参数来自已验证的当前 surface extent。
            unsafe {
                wl_egl_window_resize(self.egl_window, width, height, 0, 0);
            }
        }
        // 把尺寸事实同步到共享 OpenGL RHI pipeline。
        self.pipeline.resize_swapchain(
            // UI 与场景坐标保持逻辑宽度。
            snapshot.logical_width,
            // UI 与场景坐标保持逻辑高度。
            snapshot.logical_height,
            // native swapchain 使用物理宽度。
            width,
            // native swapchain 使用物理高度。
            height,
        );
        // generation 只允许在共享生命周期 commit 后发布。
        Ok(())
    }

    // 在 SurfaceLost 后按原生顺序替换唯一 EGL window surface。
    fn recreate_window_surface(&mut self) -> Result<(), Error> {
        use khronos_egl as egl;

        // 先解除旧 draw/read surface，禁止销毁仍为 current 的对象。
        self.egl
            .make_current(self.display, None, None, None)
            .map_err(|error| map_egl_surface_error("eglMakeCurrent(NULL)", error))?;
        // EGL_BAD_SURFACE 表示旧对象已失效，仍允许继续创建 replacement。
        match self.egl.destroy_surface(self.display, self.surface) {
            Ok(()) | Err(egl::Error::BadSurface) => {
                // 从此点开始 shutdown 不得再次销毁旧句柄。
                self.surface_destroyed = true;
            }
            Err(error) => return Err(map_egl_surface_error("eglDestroySurface", error)),
        }
        // 同一个 wl_egl_window 只能在旧 EGLSurface 释放后建立 replacement。
        let surface = unsafe {
            // SAFETY: display/config/egl_window 由当前 owner 持有，旧 surface 已解除并销毁。
            self.egl.create_window_surface(
                self.display,
                self.config,
                self.egl_window as egl::NativeWindowType,
                None,
            )
        }
        .map_err(|error| map_egl_surface_error("eglCreateWindowSurface", error))?;
        // 新句柄一经创建便立即交回唯一正式 owner。
        self.surface = surface;
        self.surface_destroyed = false;
        // 最后把既有 GLES context 绑定到 replacement surface。
        self.egl
            .make_current(
                self.display,
                Some(self.surface),
                Some(self.surface),
                Some(self.context),
            )
            .map_err(|error| map_egl_surface_error("eglMakeCurrent(recreated)", error))?;
        // replacement surface 不能依赖 EGL 默认值，重新提交同一生产交换节拍。
        apply_production_swap_interval(&self.egl, self.display)?;
        // 只在 replacement 已创建且重新 current 成功后记录一次原生事实。
        #[cfg(feature = "test-harness")]
        {
            self.window_surface_replacements_for_test =
                self.window_surface_replacements_for_test.saturating_add(1);
        }
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 释放 EGL owner-thread 的所有 GPU 资源。
impl Drop for EglContext {
    fn drop(&mut self) {
        // Drop 只重试既有检查式关闭；失败必须留下最终诊断而不能静默丢弃。
        if let Err(error) = self.try_shutdown() {
            // Drop 关闭失败经边界观察入口记录，供最终责任边界定位泄漏。
            crate::diagnostics::observe_boundary_error("egl/adapter", &error);
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Thread affinity
//
// EGL contexts remain on their creating event-loop thread; no Send/Sync
// marker may make them transferable.
// ════════════════════════════════════════════════════════════════════════════
