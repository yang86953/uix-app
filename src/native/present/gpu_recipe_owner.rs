//! 生产 GPU recipe 的已验证 owner 门面。

// 引入统一结果与原子 surface 快照。
use crate::core::Result;
// 引入正交的 Device、Surface 与完整帧组合契约。
use crate::native::present::rhi::{GraphicsContextRhi, GraphicsDevice, GraphicsSurface};
// 引入类型化 GPU context、recipe 事实与呈现模式。
use crate::native::present::{
    GpuRecipeContext, GraphicsContextCaps, PresentImage, PresentMode, PresentSurface, RasterMode,
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
    // 在 owner 构造门禁拒绝 context 时执行唯一的检查式关闭路径。
    fn reject_context(
        // 接收仍待释放的类型化 GPU context。
        mut context: Box<dyn GpuRecipeContext>,
        // 接收触发拒绝的原始 typed error。
        error: crate::core::Error,
    ) -> Result<Self> {
        // 构造失败前检查式释放原生资源并保留原始拒绝原因。
        match context.try_shutdown() {
            // shutdown 成功时返回原始门禁错误。
            Ok(()) => Err(error),
            // shutdown 失败时链接原始门禁原因。
            Err(cleanup_error) => Err(cleanup_error.with_source(error)),
        }
    }

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
            // 交给唯一拒绝路径释放 context 并返回 recipe 不匹配错误。
            return Self::reject_context(context, error);
        }
        // 从 context 的实际 Surface 角色读取权威 capability，禁止信任独立快照。
        let surface_capabilities = context
            // 取得同一 GPU recipe owner 的组合 thin RHI。
            .rhi_context()
            // 立即复制 Surface 值快照，避免把可变借用带入后续门禁。
            .map(|rhi| rhi.surface_ref().surface_capabilities());
        // context 无法提供它承诺的 thin RHI 时必须拒绝并检查式关闭。
        let surface_capabilities = match surface_capabilities {
            // 保存已经脱离 context 借用的 Surface capability 值。
            Ok(capabilities) => capabilities,
            // 保留底层 typed failure 并走统一关闭路径。
            Err(error) => return Self::reject_context(context, error),
        };
        // registry recipe 只能冻结同一 Surface 的事实，不得形成第二个真相来源。
        if caps.present_coherency != surface_capabilities.present_coherency {
            // 构造稳定的 capability 漂移错误供 factory 与测试定位。
            let error = crate::core::Error::new(
                // 同一 candidate 内部事实冲突属于无效组合参数。
                crate::core::Errc::InvalidArgument,
                // 同时报告静态快照和实际 Surface 事实。
                format!(
                    "GpuRecipeOwner surface coherency mismatch: recipe={:?} surface={:?}",
                    // 输出 registry 冻结的 recipe 事实。
                    caps.present_coherency,
                    // 输出 thin RHI Surface 返回的权威事实。
                    surface_capabilities.present_coherency
                ),
            );
            // capability 漂移不得进入 Drawing，拒绝前仍检查式关闭 owner。
            return Self::reject_context(context, error);
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

    // 返回当前可写 swapchain image 身份，供 graphics backend 规划 per-image 修复。
    pub(crate) fn present_image(&self) -> Option<PresentImage> {
        // 直接读取类型化 context 的 live image 身份，不在 owner 内重复缓存。
        self.context.present_image()
    }

    // 激活并借用只允许资源、命令和 submit 的 thin RHI device。
    pub(crate) fn rhi_device(&mut self) -> Result<&mut dyn GraphicsDevice> {
        // 复用组合借用的唯一激活边界，再收窄为 device 角色。
        Ok(self.rhi_context()?.device())
    }

    // 借用只允许 acquire、resize 与 present 的 thin RHI surface。
    pub(crate) fn rhi_surface(&mut self) -> Result<&mut dyn GraphicsSurface> {
        // 从同一原生 owner 返回窄 surface 角色，不复制任何状态。
        Ok(self.context.rhi_context()?.surface())
    }

    // 激活并借用完整帧事务必须使用的组合 thin RHI。
    pub(crate) fn rhi_context(&mut self) -> Result<&mut dyn GraphicsContextRhi> {
        // 从类型化原生 owner 取得不可拆分的组合 context。
        let context = self.context.rhi_context()?;
        // 激活只建立 thread-current 可用性，不把 DeviceLost 健康检查混入资源释放。
        GraphicsDevice::activate(context.device())?;
        // 组合借用只应跨越资源事务或 acquire、device submit 与最终 present。
        Ok(context)
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
    use crate::core::{Errc, PresentCoherency, PresentDamage, PresentSurface};
    // 引入最小记录型 Device/Surface 实现需要的薄 RHI 值和契约。
    use crate::native::present::rhi::{
        DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities, GraphicsSurface, LoadAction,
        RenderTargetHandle, RhiExtent, RhiScissor, RhiViewport, SubmissionHandle, SurfaceFrame,
        SurfaceToken, TextureCopy,
    };
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
        // 记录 owner 借用前执行的原生 context 激活次数。
        activate_count: Rc<Cell<u32>>,
    }

    // 为测试 context 提供最小薄 RHI Device，并记录激活边界。
    impl GraphicsDevice for TypedGpuContext {
        // 返回完整 GPU 基线，owner 测试不验证具体原语能力。
        fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
            // 使用共享基线避免伪造平台能力字段。
            GraphicsDeviceCapabilities::full_gpu_baseline()
        }

        // 记录每次 owner 对 Device 或组合 context 的激活。
        fn activate(&mut self) -> crate::core::Result<()> {
            // 增加外部可观察的激活次数。
            self.activate_count.set(self.activate_count.get() + 1);
            // 测试激活固定成功。
            Ok(())
        }

        // 测试不执行真实 render pass。
        fn begin_render_pass(
            // 借用测试 device。
            &mut self,
            // 忽略不透明目标身份。
            _target: RenderTargetHandle,
            // 忽略加载动作。
            _load: LoadAction,
        ) -> crate::core::Result<()> {
            // 返回无原生副作用的成功结果。
            Ok(())
        }

        // 测试不设置真实 viewport。
        fn set_viewport(&mut self, _viewport: RhiViewport) -> crate::core::Result<()> {
            // 返回无原生副作用的成功结果。
            Ok(())
        }

        // 测试不设置真实 scissor。
        fn set_scissor(&mut self, _scissor: Option<RhiScissor>) -> crate::core::Result<()> {
            // 返回无原生副作用的成功结果。
            Ok(())
        }

        // 测试不执行真实 draw。
        fn draw(&mut self, _packet: DrawPacket) -> crate::core::Result<()> {
            // 返回无原生副作用的成功结果。
            Ok(())
        }

        // 测试不执行真实 texture copy。
        fn copy_texture(&mut self, _copy: TextureCopy) -> crate::core::Result<()> {
            // 返回无原生副作用的成功结果。
            Ok(())
        }

        // 测试不维护真实 pass 状态。
        fn end_render_pass(&mut self) -> crate::core::Result<()> {
            // 返回无原生副作用的成功结果。
            Ok(())
        }

        // 返回稳定的测试提交身份。
        fn submit(&mut self) -> crate::core::Result<SubmissionHandle> {
            // 使用非零句柄满足共享提交契约。
            Ok(SubmissionHandle::from_raw(1))
        }
    }

    // 为测试 context 提供不触碰窗口系统的最小 Surface。
    impl GraphicsSurface for TypedGpuContext {
        // 返回稳定的一像素 surface token。
        fn token(&self) -> SurfaceToken {
            // 使用固定 generation 和正 extent。
            SurfaceToken::new(0, RhiExtent::new(1, 1))
        }

        // 返回与当前 token 匹配的测试 frame。
        fn acquire(&mut self) -> crate::core::Result<SurfaceFrame> {
            // 使用非零目标身份模拟成功 acquire。
            Ok(SurfaceFrame::new(
                // 保持 frame 与当前 surface 同代。
                self.token(),
                // 测试 target 不映射任何原生对象。
                RenderTargetHandle::from_raw(1),
            ))
        }

        // 返回请求 extent 对应的新测试 token。
        fn resize(&mut self, extent: RhiExtent) -> crate::core::Result<SurfaceToken> {
            // 测试只验证角色借用，不保存动态 surface 状态。
            Ok(SurfaceToken::new(1, extent))
        }

        // 测试 present 不触碰任何原生窗口。
        fn present(
            // 借用测试 surface。
            &mut self,
            // 忽略测试 frame。
            _frame: SurfaceFrame,
            // 忽略测试提交身份。
            _submission: SubmissionHandle,
            // 忽略测试 damage。
            _damage: PresentDamage,
        ) -> crate::core::Result<()> {
            // 返回无原生副作用的成功结果。
            Ok(())
        }
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
        // 返回由同一测试 owner 实现的组合 thin RHI。
        fn rhi_context(
            // 借用测试 GPU owner。
            &mut self,
        ) -> crate::core::Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
            // 不创建第二个 Device 或 Surface 实例。
            Ok(self)
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
    fn test_context() -> (
        // 返回可交给 owner 的类型化 context。
        TypedGpuContext,
        // 返回 resize 次数观察句柄。
        Rc<Cell<u32>>,
        // 返回 shutdown 结果观察句柄。
        Rc<Cell<bool>>,
        // 返回原生 context 激活次数观察句柄。
        Rc<Cell<u32>>,
    ) {
        // 创建 resize 计数器。
        let resize_count = Rc::new(Cell::new(0));
        // 创建 shutdown 标记。
        let shutdown = Rc::new(Cell::new(false));
        // 创建 owner-context 激活计数器。
        let activate_count = Rc::new(Cell::new(0));
        // 返回 context 与外部观察句柄。
        (
            // 组装测试 context。
            TypedGpuContext {
                // 共享 resize 计数器。
                resize_count: Rc::clone(&resize_count),
                // 共享 shutdown 标记。
                shutdown: Rc::clone(&shutdown),
                // 共享激活计数器。
                activate_count: Rc::clone(&activate_count),
            },
            // 返回 resize 观察句柄。
            resize_count,
            // 返回 shutdown 观察句柄。
            shutdown,
            // 返回激活次数观察句柄。
            activate_count,
        )
    }

    // 验证错误静态 recipe 会在拒绝前关闭类型化 owner。
    #[test]
    fn rejects_non_gpu_recipe_and_shuts_down() {
        // 构造可观察的类型化 GPU context。
        let (context, _resize_count, shutdown, _activate_count) = test_context();
        // 故意使用 CPU PixelUpload capability。
        let caps = GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan);
        // GPU owner 必须拒绝静态 recipe 错配。
        let result = GpuRecipeOwner::try_new(Box::new(context), caps);
        // 错配保持参数错误分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
        // 拒绝前必须执行 checked shutdown。
        assert!(shutdown.get());
    }

    // 验证静态 recipe 不得与实际 Surface capability 形成两个真相来源。
    #[test]
    // 使用默认 FullOnly Surface 与伪造 TrackedSwapchain recipe 制造明确漂移。
    fn rejects_surface_coherency_drift_and_shuts_down() {
        // 构造默认报告 FullOnly Surface capability 的测试 context。
        let (context, _resize_count, shutdown, _activate_count) = test_context();
        // 故意让 registry recipe 声称不存在的 tracked swapchain 证明。
        let caps = GraphicsContextCaps::gpu_native_swapchain(
            // 使用 D3D11 身份模拟创建边界错误冻结的快照。
            GraphicsApi::D3d11,
            // 与测试 Surface 默认 FullOnly 事实冲突。
            PresentCoherency::TrackedSwapchain,
        );
        // GPU owner 必须在 Drawing 取得 context 前拒绝 capability 漂移。
        let result = GpuRecipeOwner::try_new(Box::new(context), caps);
        // 漂移必须保持稳定的无效参数分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
        // 拒绝候选 context 前必须完成 checked shutdown。
        assert!(shutdown.get());
    }

    // 验证合法类型化 owner 直接委托 lifecycle 操作。
    #[test]
    fn delegates_typed_gpu_lifecycle_without_capability_queries() {
        // 构造可观察的类型化 GPU context。
        let (context, resize_count, shutdown, _activate_count) = test_context();
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

    // 验证 Device 和组合 context 借用会激活 owner，而 Surface 借用保持角色正交。
    #[test]
    fn activates_device_borrows_without_mixing_surface_role() {
        // 构造带激活计数器的类型化 GPU context。
        let (context, _resize_count, _shutdown, activate_count) = test_context();
        // 使用合法 GPU-native × Swapchain capability。
        let caps = GraphicsContextCaps::gpu_native_swapchain(
            // 使用 D3D11 身份代表任意生产 GPU recipe。
            GraphicsApi::D3d11,
            // 测试只需要完整重绘 coherency。
            PresentCoherency::FullOnly,
        );
        // 构造唯一 GPU recipe owner。
        let mut owner = GpuRecipeOwner::try_new(Box::new(context), caps)
            // 合法 recipe 构造失败属于测试错误。
            .expect("typed GPU recipe must construct");
        // 借用窄 Device 角色。
        owner
            // Device 借用必须执行 owner-context 激活。
            .rhi_device()
            // 测试 context 固定激活成功。
            .expect("device borrow should activate");
        // 第一次 Device 借用必须只激活一次。
        assert_eq!(activate_count.get(), 1);
        // 借用完整 FramePlan 组合 context。
        owner
            // 组合借用同样必须执行 owner-context 激活。
            .rhi_context()
            // 测试 context 固定激活成功。
            .expect("context borrow should activate");
        // 第二次组合借用必须再激活当前 owner。
        assert_eq!(activate_count.get(), 2);
        // 只借用 Surface 角色。
        owner
            // Surface 元数据和 resize 不得偷偷触发 Device 激活。
            .rhi_surface()
            // 测试 Surface 借用固定成功。
            .expect("surface borrow should not activate device");
        // Surface 借用必须保持两个角色正交。
        assert_eq!(activate_count.get(), 2);
    }
}
