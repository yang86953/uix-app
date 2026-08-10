//! Factory-owned thread affinity for native graphics contexts (#183).
//!
//! Native APIs such as WGL, EGL, D3D and Vulkan associate a context with the
//! creating thread.  Every production context therefore leaves the registry
//! wrapped in this type.  The `Rc` marker deliberately makes the wrapper
//! neither `Send` nor `Sync`; safe Rust cannot transfer it to another thread.
//! Result-returning graphics operations also validate the owner and report an
//! `InvalidState` error instead of reaching an API object from the wrong
//! thread.
//!
//! The former non-fallible lifecycle operations now have checked `try_*`
//! counterparts.  The legacy hooks remain compatibility adapters while each
//! platform implementation migrates; they log and return a safe empty value
//! on an owner violation rather than panicking or reaching native state.

use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::thread::{self, ThreadId};

use crate::core::{Errc, Error, Result};
use crate::native::present::{
    GpuRecipeContext, GraphicsContextCaps, IGraphicsContext, PixelUploadSurface, PresentDamage,
    PresentSurface,
};

pub(crate) fn bind_to_current_thread(
    inner: Box<dyn IGraphicsContext>,
) -> Box<dyn IGraphicsContext> {
    Box::new(ThreadBoundGraphicsContext::new(inner))
}

pub(crate) struct ThreadBoundGraphicsContext {
    owner_thread: ThreadId,
    inner: ManuallyDrop<Box<dyn IGraphicsContext>>,
    /// Read-only metadata is captured on the creation thread and refreshed
    /// only after successful owner-thread lifecycle changes, so foreign-thread
    /// queries never touch the native context.
    caps: GraphicsContextCaps,
    // 把 live drawable 元数据保存为不可撕裂的单一快照。
    present_surface: PresentSurface,
    // `Rc` is intentionally !Send + !Sync. `Cell` makes the intent equally
    // explicit to readers inspecting the wrapper's auto-trait boundary.
    _thread_bound: PhantomData<Rc<Cell<()>>>,
}

impl ThreadBoundGraphicsContext {
    fn new(inner: Box<dyn IGraphicsContext>) -> Self {
        let caps = inner.caps();
        // 在 owner thread 一次读取完整 surface 元数据。
        let present_surface = inner.present_surface();
        Self {
            owner_thread: thread::current().id(),
            inner: ManuallyDrop::new(inner),
            caps,
            present_surface,
            _thread_bound: PhantomData,
        }
    }

    // 只刷新运行期可变的完整 surface 快照，静态 capability 保持构造期值。
    fn refresh_present_surface(&mut self) {
        // 生命周期变更成功后原子替换完整 surface 快照。
        self.present_surface = self.inner.present_surface();
    }

    fn require_owner(&self, operation: &str) -> Result<()> {
        let current = thread::current().id();
        if current == self.owner_thread {
            return Ok(());
        }
        Err(Error::new(
            Errc::InvalidState,
            format!(
                "graphics context operation {operation} must run on creation thread {:?}; current thread is {:?}",
                self.owner_thread, current
            ),
        ))
    }

    fn with_owner<T>(
        &mut self,
        operation: &str,
        run: impl FnOnce(&mut dyn IGraphicsContext) -> Result<T>,
    ) -> Result<T> {
        self.require_owner(operation)?;
        run(self.inner.as_mut())
    }

    fn log_legacy_rejection(operation: &str, error: &Error) {
        tracing::error!(
            "legacy graphics context {operation} rejected: {}",
            error.what()
        );
    }

    // 测试目标保留可注入 owner thread 的构造器，供线程归属契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn with_test_owner(
        inner: Box<dyn IGraphicsContext>,
        owner_thread: ThreadId,
    ) -> Self {
        let mut bound = Self::new(inner);
        bound.owner_thread = owner_thread;
        bound
    }
}

impl Drop for ThreadBoundGraphicsContext {
    fn drop(&mut self) {
        // SAFETY: Drop runs once; the inner Box is taken exactly once here.
        let mut inner = unsafe { ManuallyDrop::take(&mut self.inner) };
        if self.require_owner("drop").is_err() {
            // Wrong-thread Drop must not tear down native API objects. Leak the
            // context so its Drop cannot run on this foreign thread.
            Self::log_legacy_rejection(
                "drop",
                &Error::new(
                    Errc::InvalidState,
                    format!(
                        "graphics context operation drop must run on creation thread {:?}; current thread is {:?}",
                        self.owner_thread,
                        thread::current().id()
                    ),
                ),
            );
            std::mem::forget(inner);
            return;
        }
        if let Err(error) = inner.try_shutdown() {
            Self::log_legacy_rejection("drop", &error);
        }
        drop(inner);
    }
}

impl IGraphicsContext for ThreadBoundGraphicsContext {
    fn caps(&self) -> GraphicsContextCaps {
        self.caps
    }

    // 只有 owner thread 可以借用 inner 的完整 GPU recipe 视图。
    fn gpu_recipe_context(&mut self) -> Option<&mut dyn GpuRecipeContext> {
        // 该可选查询无错误返回通道，跨线程时保守地报告不支持。
        if self.require_owner("gpu_recipe_context").is_err() {
            // 禁止把 wrapper 自身暴露给错误线程。
            return None;
        }
        // 先确认真实 context 明确提供不可拆分的 GPU recipe 视图。
        self.inner.gpu_recipe_context()?;
        // 返回继续执行 owner-thread 检查与元数据刷新的原子 wrapper 视图。
        Some(self)
    }

    // 只有 owner thread 可以借用 inner 的 PixelUpload surface 视图。
    fn pixel_upload_surface(&mut self) -> Option<&mut dyn PixelUploadSurface> {
        // 无错误返回通道的 capability 查询在跨线程时保守返回不支持。
        if self.require_owner("pixel_upload_surface").is_err() {
            // 禁止把 wrapper 自身暴露给错误线程。
            return None;
        }
        // 只有真实 context 明确实现专用契约时 wrapper 才提供同一能力。
        self.inner.pixel_upload_surface()?;
        // 返回继续执行 owner-thread 检查与元数据同步的 wrapper 视图。
        Some(self)
    }

    // 返回 owner-thread 最近一次确认的完整 surface 元数据快照。
    fn present_surface(&self) -> PresentSurface {
        // 非失败查询只读取 wrapper 缓存，不跨线程触碰 native context。
        self.present_surface
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.with_owner("try_shutdown", |inner| inner.try_shutdown())
    }
}

// 在线程绑定边界实现不可拆分的 GPU-native recipe 视图。
impl GpuRecipeContext for ThreadBoundGraphicsContext {
    // 在 owner thread 借用真实 context 的唯一 thin RHI owner。
    fn rhi_context(
        // 借用 thread-bound wrapper。
        &mut self,
    ) -> Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
        // 显式保留错误线程的 typed failure，不把它降级成能力缺失。
        self.require_owner("gpu_recipe_rhi_context")?;
        // 构造后丢失原子视图属于 native context 状态破坏。
        let recipe = self.inner.gpu_recipe_context().ok_or_else(|| {
            // 返回稳定错误供恢复层重建完整 recipe owner。
            Error::new(
                // 使用状态错误区分注册缺口与可选能力。
                Errc::InvalidState,
                // 明确指出丢失的是完整 GPU recipe 视图。
                "graphics context lost its atomic GPU recipe context",
            )
        })?;
        // 让真实 recipe owner 返回 thin RHI 或其 typed failure。
        recipe.rhi_context()
    }

    // 在 owner thread 执行 resize，并仅在成功后同步外层 drawable 元数据。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 把实际 resize 委托给创建线程上的 native 专用视图。
        self.with_owner("gpu_recipe_resize_surface", |inner| {
            // recipe 宣称 GPU-native 却未提供原子视图属于状态破坏。
            let recipe = inner.gpu_recipe_context().ok_or_else(|| {
                // 返回稳定的 typed 状态错误，禁止恢复分裂兼容视图。
                Error::new(
                    // 使用状态错误交给恢复层。
                    Errc::InvalidState,
                    // 明确指出 native owner 丢失完整 GPU recipe。
                    "graphics context lost its atomic GPU recipe context",
                )
            })?;
            // 让真实 native owner 执行唯一 GraphicsSurface::resize 路径。
            recipe.resize_surface(width, height)
        })?;
        // RHI 可能改变物理 drawable，成功后原子刷新完整 surface 快照。
        self.refresh_present_surface();
        // 返回已经通过 owner-thread 和 surface generation 边界的成功结果。
        Ok(())
    }
}

// 在线程绑定边界实现 CPU PixelUpload 的专用 surface resize。
impl PixelUploadSurface for ThreadBoundGraphicsContext {
    // 把 resize 委托给真实 PixelUpload adapter 并刷新只读元数据。
    fn resize_pixel_upload_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 在创建线程上借用 inner 的专用 surface 契约。
        self.with_owner("resize_pixel_upload_surface", |inner| {
            // recipe 宣称 PixelUpload 却未提供专用 surface 属于状态破坏。
            let Some(surface) = inner.pixel_upload_surface() else {
                // 返回 typed 状态错误，禁止回退已经删除的兼容入口。
                return Err(Error::new(
                    // 使用稳定状态分类交给恢复层。
                    Errc::InvalidState,
                    // 明确指出 factory/context 契约不一致。
                    "PixelUpload graphics context does not expose its dedicated surface",
                ));
            };
            // 在同一 owner-thread 借用范围内执行 adapter resize。
            surface.resize_pixel_upload_surface(width, height)
        })?;
        // resize 成功后同步完整 drawable surface 快照。
        self.refresh_present_surface();
        // 返回已经完成线程检查和元数据同步的成功结果。
        Ok(())
    }

    // 在线程绑定边界内提交 CPU PixelUpload 像素。
    fn present_pixels(
        // 借用 thread-bound wrapper。
        &mut self,
        // 转发 premultiplied BGRA 像素。
        pixels: &[u32],
        // 转发物理像素宽度。
        width: i32,
        // 转发物理像素高度。
        height: i32,
        // 转发最终提交 damage。
        damage: PresentDamage,
    ) -> Result<()> {
        // 在 owner thread 上借用 inner 的专用 PixelUpload surface。
        self.with_owner("present_pixels", |inner| {
            // recipe 能力在构造后消失属于 native context 状态破坏。
            let Some(surface) = inner.pixel_upload_surface() else {
                // 返回 typed 状态错误，禁止回退已移除的统一 present。
                return Err(Error::new(
                    // 使用稳定状态分类交给恢复层。
                    Errc::InvalidState,
                    // 明确指出专用提交契约缺失。
                    "PixelUpload graphics context lost its dedicated presentation surface",
                ));
            };
            // 在同一 owner-thread 借用范围内执行像素提交。
            surface.present_pixels(pixels, width, height, damage)
        })
    }
}

// 集中验证 thread-bound RHI surface 生命周期的元数据事务边界。
#[cfg(test)]
mod tests {
    // 引入当前模块的 thread-bound wrapper 与原子 GPU recipe 契约。
    use super::{GpuRecipeContext, ThreadBoundGraphicsContext};
    // 引入构造测试错误与结果所需的核心类型。
    use crate::core::{Errc, Error, PresentCoherency, Result};
    // 引入测试 context 需要实现的最小图形上下文类型。
    use crate::native::present::{
        // 引入 GPU recipe 的后端身份。
        GraphicsApi,
        // 引入静态 context 能力快照。
        GraphicsContextCaps,
        // 引入统一 context 门面。
        IGraphicsContext,
        // 引入 live surface 原子快照。
        PresentSurface,
    };

    // 提供可独立控制 resize 成败的最小 RHI 生命周期 context。
    struct TestRhiLifecycleContext {
        // 保存 native owner 当前公开的完整 surface 快照。
        surface: PresentSurface,
        // 决定下一次 resize 是否返回 typed failure。
        fail_resize: bool,
    }

    // 实现测试 context 的统一 recipe 与元数据门面。
    impl IGraphicsContext for TestRhiLifecycleContext {
        // 声明该测试 owner 使用 GPU-native swapchain recipe。
        fn caps(&self) -> GraphicsContextCaps {
            // 使用 D3D11 身份表达 Windows 生产路径的静态能力。
            GraphicsContextCaps::gpu_native_swapchain(
                // 选择稳定存在的 D3D11 后端枚举。
                GraphicsApi::D3d11,
                // 测试不声明跨帧内容保持能力。
                PresentCoherency::FullOnly,
            )
        }

        // 显式暴露测试 owner 的完整 GPU recipe 视图。
        fn gpu_recipe_context(&mut self) -> Option<&mut dyn GpuRecipeContext> {
            // 返回当前 owner 作为不可拆分的 recipe 视图。
            Some(self)
        }

        // 返回当前完整 drawable 元数据快照。
        fn present_surface(&self) -> PresentSurface {
            // PresentSurface 可复制，因此测试不会借用内部 native 状态。
            self.surface
        }

        // 测试 owner 没有需要失败的 native teardown。
        fn try_shutdown(&mut self) -> Result<()> {
            // 保持 wrapper Drop 路径可验证且无额外副作用。
            Ok(())
        }
    }

    // 实现可成功或失败的原子 GPU recipe 视图。
    impl GpuRecipeContext for TestRhiLifecycleContext {
        // 测试不提供真实 thin RHI，直接返回稳定的状态错误。
        fn rhi_context(
            // 借用测试 owner。
            &mut self,
        ) -> Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
            // 本组测试只验证 resize 元数据事务，不伪造 RHI 实现。
            Err(Error::new(
                // 使用稳定状态错误表达测试视图的受限范围。
                Errc::InvalidState,
                // 保留可诊断的测试错误文本。
                "test GPU recipe does not expose a thin RHI",
            ))
        }

        // 按测试配置更新 surface 或返回 typed failure。
        fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
            // 失败分支必须在修改 native 元数据前返回。
            if self.fail_resize {
                // 使用稳定的图形状态错误供恢复层分类。
                return Err(Error::new(
                    // 标记测试中的 native surface 状态失败。
                    Errc::InvalidState,
                    // 保留可诊断的测试错误文本。
                    "injected RHI surface resize failure",
                ));
            }
            // 成功分支整体替换 native owner 的 surface 快照。
            self.surface = PresentSurface::identity(
                // 记录成功 resize 后的 drawable 宽度。
                width,
                // 记录成功 resize 后的 drawable 高度。
                height,
                // 测试固定使用 identity DPR。
                1.0,
                // 每次成功重建都推进 surface generation。
                self.surface.generation + 1,
            );
            // 报告 native surface 已完整重建。
            Ok(())
        }
    }

    // 构造带稳定初始快照的测试 context。
    fn test_context(fail_resize: bool) -> TestRhiLifecycleContext {
        // 返回可由每条测试独立拥有的 native owner。
        TestRhiLifecycleContext {
            // 固定初始 drawable extent、DPR 与 generation。
            surface: PresentSurface::identity(100, 80, 1.0, 7),
            // 注入当前测试所需的 resize 结果。
            fail_resize,
        }
    }

    // 验证成功 resize 后 wrapper 一次刷新完整 surface 快照。
    #[test]
    fn successful_rhi_resize_refreshes_present_surface_snapshot() {
        // 把可成功 resize 的 native owner 绑定到当前测试线程。
        let mut bound = ThreadBoundGraphicsContext::new(Box::new(test_context(false)));
        // 通过原子 GPU recipe 视图执行一次成功重建。
        let result = GpuRecipeContext::resize_surface(&mut bound, 320, 240);
        // 成功结果必须越过 thread-bound 边界返回调用方。
        assert!(result.is_ok());
        // 读取 wrapper 在成功事务后缓存的完整 surface 快照。
        let surface = bound.present_surface();
        // drawable 宽度必须与 native owner 的新快照一致。
        assert_eq!(surface.drawable_width, 320);
        // drawable 高度必须与 native owner 的新快照一致。
        assert_eq!(surface.drawable_height, 240);
        // 同尺寸或异尺寸重建都必须推进 generation。
        assert_eq!(surface.generation, 8);
    }

    // 验证失败 resize 不会污染 wrapper 已确认的 surface 快照。
    #[test]
    fn failed_rhi_resize_preserves_present_surface_snapshot() {
        // 把会拒绝 resize 的 native owner 绑定到当前测试线程。
        let mut bound = ThreadBoundGraphicsContext::new(Box::new(test_context(true)));
        // 保存失败事务前 wrapper 的原子快照。
        let before = bound.present_surface();
        // 通过原子 GPU recipe 视图触发可观察的 typed failure。
        let result = GpuRecipeContext::resize_surface(&mut bound, 640, 480);
        // 失败必须保留稳定的错误分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidState));
        // wrapper 只允许在成功后刷新，因此失败前后快照必须完全相等。
        assert_eq!(bound.present_surface(), before);
    }
}
