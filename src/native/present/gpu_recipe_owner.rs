//! 生产 GPU recipe 的已验证 owner 门面。

// 引入统一结果与原子 surface 快照。
use crate::core::Result;
// 引入组合 thin RHI 契约。
use crate::native::present::rhi::GraphicsContextRhi;
// 引入类型化 GPU context、recipe 事实与呈现模式。
use crate::native::present::{
    GpuRecipeContext, GraphicsContextCaps, PresentMode, PresentSurface, RasterMode,
};

// 保存已经通过 GPU-native recipe 门禁的原生 context owner。
pub(crate) struct GpuRecipeOwner {
    // 固化构造门禁验证过的静态 recipe 与 backend 事实。
    caps: GraphicsContextCaps,
    // 类型化 context 只留在本门面内部，draw backend 不再承担能力查询。
    context: Box<dyn GpuRecipeContext>,
}

// 为生产 GPU backend 提供不带可选能力分支的窄 owner 契约。
impl GpuRecipeOwner {
    // 校验顶层 owner 已捕获的静态 recipe，并在失败前检查式关闭。
    pub(super) fn try_new(
        // 接收仍由本门面唯一拥有的类型化 GPU context。
        mut context: Box<dyn GpuRecipeContext>,
        // 接收正交 owner 分派时已经捕获的静态 capability 快照。
        caps: GraphicsContextCaps,
    ) -> Result<Self> {
        // GPU owner 只接受 GPU-native × Swapchain 组合。
        if caps.raster != RasterMode::GpuNative || caps.present != PresentMode::Swapchain {
            // 保存稳定诊断，供 cleanup failure 追加为原因。
            let error = crate::core::Error::new(
                // recipe 组合不属于 GPU owner 值域。
                crate::core::Errc::InvalidArgument,
                // 保留完整静态 recipe 事实。
                format!(
                    "GpuRecipeOwner requires gpu_native x swapchain, got {} raster={} present={}",
                    caps.backend, caps.raster, caps.present
                ),
            );
            // 构造失败前检查式释放原生资源并保留原始 recipe 原因。
            return match context.try_shutdown() {
                // shutdown 成功时返回 recipe 不匹配错误。
                Ok(()) => Err(error),
                // shutdown 失败时链接 recipe 不匹配原因。
                Err(cleanup_error) => Err(cleanup_error.with_source(error)),
            };
        }
        // 类型系统已经证明 GPU recipe context 的完整形状。
        Ok(Self { caps, context })
    }

    // 返回构造期已验证的静态 recipe 事实。
    pub(crate) fn caps(&self) -> GraphicsContextCaps {
        // 返回构造期快照，禁止运行期 recipe 身份漂移。
        self.caps
    }

    // 返回当前 drawable extent、DPR、transform 与 generation 的原子快照。
    pub(crate) fn present_surface(&self) -> PresentSurface {
        // live 元数据不在门面内拆分或重复缓存。
        self.context.present_surface()
    }

    // 借用构造期类型已证明的组合 thin RHI。
    pub(crate) fn rhi_context(&mut self) -> Result<&mut dyn GraphicsContextRhi> {
        // 保留 recipe 内部 owner-thread 或设备状态的 typed failure。
        self.context.rhi_context()
    }

    // 通过不可拆分的 GPU recipe context 重建 surface。
    pub(crate) fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 让同一 thread-bound/native recipe owner 执行唯一 surface resize 事务。
        self.context.resize_surface(width, height)
    }

    // 在 owner-thread 边界检查式关闭原生资源。
    pub(crate) fn try_shutdown(&mut self) -> Result<()> {
        // 保留底层 typed teardown failure，不降级为日志。
        self.context.try_shutdown()
    }
}

// 验证类型化 GPU owner 的静态门禁与直接委托。
#[cfg(test)]
mod tests {
    // 引入共享计数与关闭状态。
    use std::{cell::Cell, rc::Rc};

    // 引入被测 owner。
    use super::GpuRecipeOwner;
    // 引入错误分类、coherency 与 surface 值。
    use crate::core::{Errc, Error, PresentCoherency, PresentSurface};
    // 引入类型化 GPU context 与 capability。
    use crate::native::present::{
        GpuRecipeContext, GraphicsApi, GraphicsContextCaps, GraphicsContextLifecycle,
    };

    // 提供只记录生命周期调用的类型化 GPU context。
    struct TypedGpuContext {
        // 记录 resize 调用次数。
        resize_count: Rc<Cell<u32>>,
        // 记录 checked shutdown 是否执行。
        shutdown: Rc<Cell<bool>>,
    }

    // 实现 GPU context 的共同生命周期。
    impl GraphicsContextLifecycle for TypedGpuContext {
        // 返回稳定的最小 surface 快照。
        fn present_surface(&self) -> PresentSurface {
            // 测试不触碰真实 drawable。
            PresentSurface::identity(1, 1, 1.0, 0)
        }

        // 记录 checked shutdown。
        fn try_shutdown(&mut self) -> crate::core::Result<()> {
            // 让 Box 被消费后仍可观察释放动作。
            self.shutdown.set(true);
            // 测试 context 固定关闭成功。
            Ok(())
        }
    }

    // 实现不可选的 GPU recipe 契约。
    impl GpuRecipeContext for TypedGpuContext {
        // 本组单元测试不伪造 thin RHI。
        fn rhi_context(
            // 借用测试 GPU owner。
            &mut self,
        ) -> crate::core::Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
            // 仅 owner 构造与 resize 委托属于本测试范围。
            Err(Error::new(
                // 使用状态错误表达受限测试实现。
                Errc::InvalidState,
                // 保留可诊断文本。
                "typed GPU owner test has no thin RHI",
            ))
        }

        // 记录一次类型化 resize 委托。
        fn resize_surface(&mut self, _width: i32, _height: i32) -> crate::core::Result<()> {
            // 增加可观察调用次数。
            self.resize_count.set(self.resize_count.get() + 1);
            // 测试 resize 固定成功。
            Ok(())
        }
    }

    // 构造共享观察状态与类型化 context。
    fn test_context() -> (TypedGpuContext, Rc<Cell<u32>>, Rc<Cell<bool>>) {
        // 创建 resize 计数器。
        let resize_count = Rc::new(Cell::new(0));
        // 创建 shutdown 标记。
        let shutdown = Rc::new(Cell::new(false));
        // 返回 context 与外部观察句柄。
        (
            // 组装测试 context。
            TypedGpuContext {
                // 共享 resize 计数器。
                resize_count: Rc::clone(&resize_count),
                // 共享 shutdown 标记。
                shutdown: Rc::clone(&shutdown),
            },
            // 返回 resize 观察句柄。
            resize_count,
            // 返回 shutdown 观察句柄。
            shutdown,
        )
    }

    // 验证错误静态 recipe 会在拒绝前关闭类型化 owner。
    #[test]
    fn rejects_non_gpu_recipe_and_shuts_down() {
        // 构造可观察的类型化 GPU context。
        let (context, _resize_count, shutdown) = test_context();
        // 故意使用 CPU PixelUpload capability。
        let caps = GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan);
        // GPU owner 必须拒绝静态 recipe 错配。
        let result = GpuRecipeOwner::try_new(Box::new(context), caps);
        // 错配保持参数错误分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
        // 拒绝前必须执行 checked shutdown。
        assert!(shutdown.get());
    }

    // 验证合法类型化 owner 直接委托 lifecycle 操作。
    #[test]
    fn delegates_typed_gpu_lifecycle_without_capability_queries() {
        // 构造可观察的类型化 GPU context。
        let (context, resize_count, shutdown) = test_context();
        // 使用合法 GPU-native × Swapchain capability。
        let caps = GraphicsContextCaps::gpu_native_swapchain(
            // 使用 D3D11 身份代表生产 GPU recipe。
            GraphicsApi::D3d11,
            // 测试只需要完整重绘 coherency。
            PresentCoherency::FullOnly,
        );
        // 类型已证明的 context 必须直接进入 owner。
        let mut owner = GpuRecipeOwner::try_new(Box::new(context), caps)
            // 合法 recipe 构造失败属于测试错误。
            .expect("typed GPU recipe must construct");
        // 执行一次 GPU surface resize。
        owner
            // 传入任意正尺寸验证直接委托。
            .resize_surface(8, 6)
            // 类型化委托应成功。
            .expect("typed GPU resize must succeed");
        // resize 只能到达底层一次。
        assert_eq!(resize_count.get(), 1);
        // 显式关闭 owner。
        owner
            // 保留 typed shutdown 返回通道。
            .try_shutdown()
            // 测试 context 固定关闭成功。
            .expect("typed GPU shutdown must succeed");
        // 验证 checked shutdown 已到达底层。
        assert!(shutdown.get());
    }
}
