//! 生产 GPU recipe 的已验证 owner 门面。

// 引入统一结果与原子 surface 快照。
use crate::core::Result;
// 引入正交的 Device、Surface 与完整帧组合契约。
use crate::platform::presentation::rhi::{GraphicsContextRhi, GraphicsDevice, GraphicsSurface};
// 引入类型化 GPU context、recipe 事实与呈现模式。
use crate::platform::presentation::{
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
