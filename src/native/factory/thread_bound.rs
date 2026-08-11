//! Factory 持有的原生图形 context 线程亲和边界。
//!
//! 每个已验证 recipe 都在离开 registry 前绑定到创建线程。包装器通过
//! `Rc` 标记保持 `!Send + !Sync`，所有可能触碰原生资源的操作继续返回
//! typed failure；错误线程上的 Drop 会泄漏 owner，避免在错误线程析构。

// 引入线程亲和标记使用的内部可变单元。
use std::cell::Cell;
// 引入阻止 wrapper 自动实现 Send 与 Sync 的类型标记。
use std::marker::PhantomData;
// 引入允许错误线程 Drop 安全泄漏原生 owner 的手动析构容器。
use std::mem::ManuallyDrop;
// 引入明确保持线程本地语义的引用计数类型。
use std::rc::Rc;
// 引入当前线程身份与稳定线程标识。
use std::thread::{self, ThreadId};

// 引入线程门禁使用的统一错误与结果。
use crate::core::{Errc, Error, Result};
// 引入类型化 recipe context、共享生命周期与呈现值。
use crate::native::present::{
    GpuRecipeContext, GraphicsContextLifecycle, GraphicsRecipeContext, PixelUploadSurface,
    PresentDamage, PresentSurface,
};

// 把类型化 recipe context 绑定到当前线程。
pub(crate) fn bind_to_current_thread(
    // 接收 registry 仍唯一拥有的类型化 context。
    context: GraphicsRecipeContext,
) -> GraphicsRecipeContext {
    // 保留 recipe 类型并为具体 trait object 建立线程绑定 wrapper。
    match context {
        // GPU recipe 继续只暴露不可拆分的 GPU 契约。
        GraphicsRecipeContext::Gpu(inner) => {
            // 返回绑定当前线程的 GPU owner。
            GraphicsRecipeContext::Gpu(Box::new(ThreadBoundGraphicsContext::new(inner)))
        }
        // PixelUpload recipe 继续只暴露专用上传契约。
        GraphicsRecipeContext::PixelUpload(inner) => {
            // 返回绑定当前线程的 PixelUpload owner。
            GraphicsRecipeContext::PixelUpload(Box::new(ThreadBoundGraphicsContext::new(inner)))
        }
    }
}

// 在线程边界内唯一持有某一类型化 recipe context。
pub(crate) struct ThreadBoundGraphicsContext<T: GraphicsContextLifecycle + ?Sized> {
    // 记录允许触碰原生资源的创建线程。
    owner_thread: ThreadId,
    // 保存只能在 owner thread 检查式释放的具体 trait object。
    inner: ManuallyDrop<Box<T>>,
    // 缓存最近一次成功事务确认的完整 drawable 快照。
    present_surface: PresentSurface,
    // 明确禁止 wrapper 跨线程移动或共享。
    _thread_bound: PhantomData<Rc<Cell<()>>>,
}

// 提供所有类型化 recipe 共用的线程生命周期实现。
impl<T: GraphicsContextLifecycle + ?Sized> ThreadBoundGraphicsContext<T> {
    // 使用唯一原生 context 构造线程亲和 wrapper。
    fn new(
        // 接收仍由 wrapper 唯一拥有的类型化 context。
        inner: Box<T>,
    ) -> Self {
        // 在 owner thread 一次读取完整 surface 元数据。
        let present_surface = inner.present_surface();
        // 固化线程身份、owner 与不可撕裂快照。
        Self {
            // 当前线程成为唯一 owner thread。
            owner_thread: thread::current().id(),
            // 延迟到受控 Drop 路径再释放 inner。
            inner: ManuallyDrop::new(inner),
            // 保存构造事务确认的 surface 快照。
            present_surface,
            // 建立静态线程本地边界。
            _thread_bound: PhantomData,
        }
    }

    // 只刷新运行期可变的完整 surface 快照。
    fn refresh_present_surface(&mut self) {
        // 生命周期变更成功后原子替换完整 surface 快照。
        self.present_surface = self.inner.present_surface();
    }

    // 验证当前操作仍在创建线程执行。
    fn require_owner(&self, operation: &str) -> Result<()> {
        // 捕获当前线程以构造稳定诊断。
        let current = thread::current().id();
        // owner thread 可以继续触碰原生资源。
        if current == self.owner_thread {
            // 返回线程门禁成功。
            return Ok(());
        }
        // 错误线程保持 typed state failure。
        Err(Error::new(
            // 线程归属破坏属于运行期状态错误。
            Errc::InvalidState,
            // 保留操作名与两侧线程身份。
            format!(
                "graphics context operation {operation} must run on creation thread {:?}; current thread is {:?}",
                self.owner_thread, current
            ),
        ))
    }

    // 在 owner thread 上借用具体类型化 context。
    fn with_owner<U>(
        // 借用 thread-bound wrapper。
        &mut self,
        // 接收诊断使用的操作名。
        operation: &str,
        // 接收只在 owner thread 执行的闭包。
        run: impl FnOnce(&mut T) -> Result<U>,
    ) -> Result<U> {
        // 在借用 inner 前验证线程归属。
        self.require_owner(operation)?;
        // 把真实类型化 owner 交给受控操作。
        run(self.inner.as_mut())
    }

    // 记录无错误返回通道的析构拒绝。
    fn log_drop_rejection(operation: &str, error: &Error) {
        // 让诊断可见但不在 Drop 路径 panic。
        tracing::error!(
            "thread-bound graphics context {operation} rejected: {}",
            error.what()
        );
    }

    // 为单元测试注入不同 owner thread 身份。
    #[cfg(test)]
    fn with_test_owner(
        // 接收测试仍唯一拥有的类型化 context。
        inner: Box<T>,
        // 接收测试要模拟的 owner thread。
        owner_thread: ThreadId,
    ) -> Self {
        // 先按生产路径构造 wrapper。
        let mut bound = Self::new(inner);
        // 仅在测试构建覆盖线程身份。
        bound.owner_thread = owner_thread;
        // 返回可验证错误线程门禁的 wrapper。
        bound
    }
}

// 确保原生 owner 只在正确线程执行 checked shutdown。
impl<T: GraphicsContextLifecycle + ?Sized> Drop for ThreadBoundGraphicsContext<T> {
    // 执行 wrapper 的非失败析构边界。
    fn drop(&mut self) {
        // SAFETY: Drop 只执行一次，inner 也只在这里取出一次。
        let mut inner = unsafe { ManuallyDrop::take(&mut self.inner) };
        // 错误线程绝不能触碰原生析构 API。
        if let Err(error) = self.require_owner("drop") {
            // 记录线程违规供诊断。
            Self::log_drop_rejection("drop", &error);
            // 泄漏 owner，避免 Box 在错误线程继续析构。
            std::mem::forget(inner);
            // 结束错误线程析构路径。
            return;
        }
        // 在 owner thread 执行 checked shutdown。
        if let Err(error) = inner.try_shutdown() {
            // Drop 无返回通道，因此保留结构化日志。
            Self::log_drop_rejection("drop", &error);
        }
        // checked shutdown 后释放 Rust owner。
        drop(inner);
    }
}

// 为任意类型化 recipe 提供共同生命周期。
impl<T: GraphicsContextLifecycle + ?Sized> GraphicsContextLifecycle
    for ThreadBoundGraphicsContext<T>
{
    // 返回最近一次成功事务确认的完整 surface 快照。
    fn present_surface(&self) -> PresentSurface {
        // 非失败查询只读取 wrapper 缓存，不触碰 native context。
        self.present_surface
    }

    // 在 owner thread 检查式关闭原生资源。
    fn try_shutdown(&mut self) -> Result<()> {
        // 保留底层 typed teardown failure。
        self.with_owner("try_shutdown", |inner| inner.try_shutdown())
    }
}

// 在线程绑定边界实现不可拆分的 GPU-native recipe。
impl GpuRecipeContext for ThreadBoundGraphicsContext<dyn GpuRecipeContext> {
    // 在 owner thread 借用真实 context 的唯一 thin RHI owner。
    fn rhi_context(
        // 借用 thread-bound GPU wrapper。
        &mut self,
    ) -> Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
        // 先拒绝错误线程，再直接借用类型已证明的 GPU owner。
        self.require_owner("gpu_recipe_rhi_context")?;
        // 保留 adapter 的 typed RHI failure。
        self.inner.rhi_context()
    }

    // 在 owner thread 执行 GPU surface resize。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 把唯一 resize 事务委托给创建线程上的 GPU owner。
        self.with_owner("gpu_recipe_resize_surface", |inner| {
            // 直接调用不可选的 GPU 生命周期契约。
            inner.resize_surface(width, height)
        })?;
        // 只在成功后刷新完整 drawable 快照。
        self.refresh_present_surface();
        // 返回已越过线程和元数据事务边界的成功结果。
        Ok(())
    }
}

// 在线程绑定边界实现 CPU PixelUpload 专用 surface。
impl PixelUploadSurface for ThreadBoundGraphicsContext<dyn PixelUploadSurface> {
    // 在 owner thread 执行 PixelUpload surface resize。
    fn resize_pixel_upload_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 把唯一 resize 事务委托给创建线程上的上传 owner。
        self.with_owner("resize_pixel_upload_surface", |inner| {
            // 直接调用不可选的 PixelUpload 生命周期契约。
            inner.resize_pixel_upload_surface(width, height)
        })?;
        // 只在成功后刷新完整 drawable 快照。
        self.refresh_present_surface();
        // 返回已越过线程和元数据事务边界的成功结果。
        Ok(())
    }

    // 在线程绑定边界提交 CPU PixelUpload 像素。
    fn present_pixels(
        // 借用 thread-bound PixelUpload wrapper。
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
        // 在 owner thread 直接调用不可选的 PixelUpload 提交契约。
        self.with_owner("present_pixels", |inner| {
            // 保留 adapter 的 typed presentation failure。
            inner.present_pixels(pixels, width, height, damage)
        })
    }
}

// 集中验证 thread-bound GPU surface 生命周期事务。
#[cfg(test)]
mod tests {
    // 引入被测 GPU wrapper 与共享生命周期契约。
    use super::{GpuRecipeContext, GraphicsContextLifecycle, ThreadBoundGraphicsContext};
    // 引入测试错误与结果类型。
    use crate::core::{Errc, Error, Result};
    // 引入原子 surface 快照。
    use crate::native::present::PresentSurface;

    // 提供可独立控制 resize 成败的最小 GPU context。
    struct TestRhiLifecycleContext {
        // 保存 native owner 当前公开的完整 surface 快照。
        surface: PresentSurface,
        // 决定下一次 resize 是否返回 typed failure。
        fail_resize: bool,
    }

    // 实现测试 context 的共同生命周期。
    impl GraphicsContextLifecycle for TestRhiLifecycleContext {
        // 返回当前完整 drawable 元数据快照。
        fn present_surface(&self) -> PresentSurface {
            // PresentSurface 可复制，不借用内部 native 状态。
            self.surface
        }

        // 测试 owner 没有需要失败的 native teardown。
        fn try_shutdown(&mut self) -> Result<()> {
            // 保持 wrapper Drop 路径可验证。
            Ok(())
        }
    }

    // 实现可成功或失败的原子 GPU recipe。
    impl GpuRecipeContext for TestRhiLifecycleContext {
        // 测试不提供真实 thin RHI。
        fn rhi_context(
            // 借用测试 owner。
            &mut self,
        ) -> Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
            // 本组测试只验证 resize 元数据事务。
            Err(Error::new(
                // 使用稳定状态错误表达测试边界。
                Errc::InvalidState,
                // 保留可诊断的测试错误文本。
                "test GPU recipe does not expose a thin RHI",
            ))
        }

        // 按测试配置更新 surface 或返回 typed failure。
        fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
            // 失败分支必须在修改 native 元数据前返回。
            if self.fail_resize {
                // 返回可由恢复层分类的 typed failure。
                return Err(Error::new(
                    // 标记 native surface 状态失败。
                    Errc::InvalidState,
                    // 保留可诊断的注入错误文本。
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
                // 每次成功重建都推进 generation。
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

    // 把具体测试 context 擦除为生产使用的 GPU trait object 形状。
    fn bound_test_context(
        // 接收当前测试需要注入的 resize 结果。
        fail_resize: bool,
    ) -> ThreadBoundGraphicsContext<dyn GpuRecipeContext> {
        // 在进入 wrapper 前完成类型化 trait object 擦除。
        let context: Box<dyn GpuRecipeContext> = Box::new(test_context(fail_resize));
        // 使用与生产 bind 函数相同的 trait object 形状构造 wrapper。
        ThreadBoundGraphicsContext::new(context)
    }

    // 验证成功 resize 后 wrapper 一次刷新完整 surface 快照。
    #[test]
    fn successful_rhi_resize_refreshes_present_surface_snapshot() {
        // 把可成功 resize 的 native owner 绑定到当前测试线程。
        let mut bound = bound_test_context(false);
        // 通过原子 GPU recipe 执行一次成功重建。
        let result = GpuRecipeContext::resize_surface(&mut bound, 320, 240);
        // 成功结果必须越过 thread-bound 边界返回调用方。
        assert!(result.is_ok());
        // 读取成功事务后缓存的完整 surface 快照。
        let surface = bound.present_surface();
        // drawable 宽度必须与 native owner 的新快照一致。
        assert_eq!(surface.drawable_width, 320);
        // drawable 高度必须与 native owner 的新快照一致。
        assert_eq!(surface.drawable_height, 240);
        // 成功重建必须推进 generation。
        assert_eq!(surface.generation, 8);
    }

    // 验证失败 resize 不会污染 wrapper 已确认的 surface 快照。
    #[test]
    fn failed_rhi_resize_preserves_present_surface_snapshot() {
        // 把会拒绝 resize 的 native owner 绑定到当前测试线程。
        let mut bound = bound_test_context(true);
        // 保存失败事务前 wrapper 的原子快照。
        let before = bound.present_surface();
        // 通过原子 GPU recipe 触发 typed failure。
        let result = GpuRecipeContext::resize_surface(&mut bound, 640, 480);
        // 失败必须保留稳定错误分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidState));
        // wrapper 只允许在成功后刷新快照。
        assert_eq!(bound.present_surface(), before);
    }

    // 验证错误线程操作在触碰 native owner 前被拒绝。
    #[test]
    fn wrong_thread_resize_returns_typed_failure() {
        // 从临时线程取得与当前测试线程不同的身份。
        let foreign_owner = std::thread::spawn(|| std::thread::current().id())
            // 临时线程不应 panic。
            .join()
            // 失败时给出明确测试诊断。
            .expect("thread identity probe must complete");
        // 注入不同 owner 身份而不跨线程移动 !Send wrapper。
        let mut bound: ThreadBoundGraphicsContext<dyn GpuRecipeContext> =
            ThreadBoundGraphicsContext::with_test_owner(
                // 提供已经擦除为 GPU trait object 的真实 owner。
                Box::new(test_context(false)) as Box<dyn GpuRecipeContext>,
                // 使用临时线程身份触发门禁。
                foreign_owner,
            );
        // 当前线程上的 resize 必须被 typed owner gate 拒绝。
        let result = GpuRecipeContext::resize_surface(&mut bound, 320, 240);
        // 禁止把线程违规降级为能力缺失或 panic。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidState));
        // 避免测试 Drop 在故意注入的错误 owner 身份下泄漏 context。
        bound.owner_thread = std::thread::current().id();
    }
}
