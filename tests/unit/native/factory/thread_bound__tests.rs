// 引入被测 GPU wrapper 与共享生命周期契约。
use super::{GpuRecipeContext, GraphicsContextLifecycle, ThreadBoundGraphicsContext};
// 引入测试错误与结果类型。
use crate::core::{Errc, Error, Result};
// 引入原子 surface 快照。
use crate::platform::presentation::PresentSurface;

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
    ) -> Result<&mut dyn crate::platform::presentation::rhi::GraphicsContextRhi> {
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
