//! 生产 GPU recipe 的已验证 owner 门面。

// 引入统一错误、结果与原子 surface 快照。
use crate::core::{Errc, Error, Result};
// 引入组合 thin RHI 契约。
use crate::native::present::rhi::GraphicsContextRhi;
// 引入迁移期 context、recipe 事实与呈现模式。
use crate::native::present::{
    GraphicsContextCaps, IGraphicsContext, PresentMode, PresentSurface, RasterMode,
};

// 保存已经通过 GPU-native recipe 门禁的原生 context owner。
pub(crate) struct GpuRecipeOwner {
    // 固化构造门禁验证过的静态 recipe 与 backend 事实。
    caps: GraphicsContextCaps,
    // 兼容 context 只留在本门面内部，draw backend 不再直接依赖可选视图。
    context: Box<dyn IGraphicsContext>,
}

// 为生产 GPU backend 提供不带可选能力分支的窄 owner 契约。
impl GpuRecipeOwner {
    // 校验顶层 owner 已捕获的静态 recipe 与原子 GPU recipe 视图，并在失败前检查式关闭。
    pub(super) fn try_new(
        // 接收仍由本门面唯一拥有的迁移期 context。
        mut context: Box<dyn IGraphicsContext>,
        // 接收正交 owner 分派时已经捕获的静态 capability 快照。
        caps: GraphicsContextCaps,
    ) -> Result<Self> {
        // GPU owner 只接受 GPU-native × swapchain 组合。
        if caps.raster != RasterMode::GpuNative || caps.present != PresentMode::Swapchain {
            // 保存稳定诊断，避免关闭借用影响错误文本。
            let message = format!(
                "GpuRecipeOwner requires gpu_native x swapchain, got {} raster={} present={}",
                caps.backend, caps.raster, caps.present
            );
            // 构造失败前检查式释放原生资源。
            context.try_shutdown()?;
            // 返回 recipe 不匹配的 typed 参数错误。
            return Err(Error::new(Errc::InvalidArgument, message));
        }
        // 构造期只需一次证明 thin RHI 与 lifecycle 的不可拆分 owner 存在。
        if context.gpu_recipe_context().is_none() {
            // 保存完整 GPU recipe 缺失错误，供 cleanup 失败时链接原始原因。
            let error = Error::new(
                // recipe 已声明 GPU 但缺失原子 owner，属于构造状态破坏。
                Errc::InvalidState,
                // 保留具体 backend 便于诊断不完整 adapter 注册。
                format!(
                    "GraphicsBackend {} GPU recipe lacks an atomic recipe context",
                    caps.backend
                ),
            );
            // 构造失败前检查式释放原生资源并保留原始原因。
            return match context.try_shutdown() {
                // shutdown 成功时返回完整 recipe owner 缺失错误。
                Ok(()) => Err(error),
                // shutdown 失败时链接原子 owner 缺失错误。
                Err(cleanup_error) => Err(cleanup_error.with_source(error)),
            };
        }
        // 只有通过完整门禁的 context 才能进入 draw backend。
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

    // 借用构造期已验证的组合 thin RHI；运行期破坏保持 typed error。
    pub(crate) fn rhi_context(&mut self) -> Result<&mut dyn GraphicsContextRhi> {
        // 保存 backend 身份，避免可变借用后再次访问 context。
        let backend = self.caps.backend;
        // 将唯一可选兼容查询收口为 GPU owner 的稳定 Result 契约。
        let recipe = self.context.gpu_recipe_context().ok_or_else(|| {
            // 构造后丢失原子视图属于可恢复层识别的状态破坏。
            Error::new(
                // 使用稳定状态分类交给恢复层。
                Errc::InvalidState,
                // 明确指出丢失的是不可拆分的 recipe owner。
                format!("GraphicsBackend {backend} GPU recipe lost its atomic context"),
            )
        })?;
        // 保留 recipe 内部 owner-thread 或设备状态的 typed failure。
        recipe.rhi_context()
    }

    // 通过不可拆分的 GPU recipe 视图重建 surface。
    pub(crate) fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 保存 backend 身份，供原子视图丢失时构造稳定错误。
        let backend = self.caps.backend;
        // 将唯一可选兼容查询收口在 native owner 边界内。
        let recipe = self.context.gpu_recipe_context().ok_or_else(|| {
            // GPU recipe 构造后丢失原子视图属于状态破坏。
            Error::new(
                // 使用稳定状态分类交给恢复层。
                Errc::InvalidState,
                // 明确指出 thin RHI 与 lifecycle 必须整体存在。
                format!("GraphicsBackend {backend} GPU recipe lost its atomic context"),
            )
        })?;
        // 让同一 thread-bound/native recipe owner 执行唯一 surface resize 事务。
        recipe.resize_surface(width, height)
    }

    // 在 owner-thread 边界检查式关闭原生资源。
    pub(crate) fn try_shutdown(&mut self) -> Result<()> {
        // 保留底层 typed teardown failure，不降级为日志。
        self.context.try_shutdown()
    }
}

// 验证构造门禁会拒绝错误 recipe 与缺失原子 GPU 视图的 context。
#[cfg(test)]
mod tests {
    // 引入共享关闭状态所需的 Cell 与 Rc。
    use std::{cell::Cell, rc::Rc};

    // 引入被测 owner。
    use super::GpuRecipeOwner;
    // 引入错误分类与 coherency 事实。
    use crate::core::{Errc, PresentCoherency};
    // 引入最小 thin RHI 测试适配器所需契约和值。
    use crate::native::present::rhi::{
        DrawPacket, GraphicsCapabilities, GraphicsDevice, GraphicsSurface, LoadAction,
        RenderTargetHandle, RhiExtent, RhiScissor, RhiViewport, SubmissionHandle, SurfaceFrame,
        SurfaceToken, TextureCopy,
    };
    // 引入最小测试 context 所需契约。
    use crate::native::present::{
        GpuRecipeContext, GraphicsApi, GraphicsContextCaps, IGraphicsContext, PresentDamage,
        PresentSurface,
    };

    // 提供可选择是否暴露完整 GPU recipe 视图的最小 context。
    struct MissingRhiContext {
        // 允许测试在 owner 消费 Box 后观察 checked shutdown。
        shutdown: Rc<Cell<bool>>,
        // 控制测试 context 声明的静态 recipe。
        caps: GraphicsContextCaps,
        // 控制测试 context 是否暴露完整 GPU recipe 视图。
        expose_gpu_recipe: bool,
    }

    // 为完整 GPU recipe 成功场景提供最小 device 事实。
    impl GraphicsDevice for MissingRhiContext {
        // 返回满足 GPU owner 类型要求的事实能力快照。
        fn capabilities(&self) -> GraphicsCapabilities {
            // 测试不执行 probe，只需提供稳定 GPU baseline。
            GraphicsCapabilities::full_gpu_baseline()
        }

        // 接受最小测试 render pass；owner 构造不会执行该方法。
        fn begin_render_pass(
            // 借用测试 device owner。
            &mut self,
            // 忽略未执行的 render target。
            _target: RenderTargetHandle,
            // 忽略未执行的 load action。
            _load: LoadAction,
        ) -> crate::core::Result<()> {
            // 最小适配器固定报告成功。
            Ok(())
        }

        // 接受最小测试 viewport；owner 构造不会执行该方法。
        fn set_viewport(&mut self, _viewport: RhiViewport) -> crate::core::Result<()> {
            // 最小适配器固定报告成功。
            Ok(())
        }

        // 接受最小测试 scissor；owner 构造不会执行该方法。
        fn set_scissor(&mut self, _scissor: Option<RhiScissor>) -> crate::core::Result<()> {
            // 最小适配器固定报告成功。
            Ok(())
        }

        // 接受最小测试 draw packet；owner 构造不会执行该方法。
        fn draw(&mut self, _packet: DrawPacket) -> crate::core::Result<()> {
            // 最小适配器固定报告成功。
            Ok(())
        }

        // 接受最小测试 texture copy；owner 构造不会执行该方法。
        fn copy_texture(&mut self, _copy: TextureCopy) -> crate::core::Result<()> {
            // 最小适配器固定报告成功。
            Ok(())
        }

        // 结束最小测试 render pass；owner 构造不会执行该方法。
        fn end_render_pass(&mut self) -> crate::core::Result<()> {
            // 最小适配器固定报告成功。
            Ok(())
        }

        // 返回固定测试 submission；owner 构造不会执行该方法。
        fn submit(&mut self) -> crate::core::Result<SubmissionHandle> {
            // 使用非零不透明 submission handle。
            Ok(SubmissionHandle::from_raw(1))
        }
    }

    // 为完整 GPU recipe 成功场景提供不触碰真实 OS surface 的最小契约。
    impl GraphicsSurface for MissingRhiContext {
        // 返回固定测试 surface token。
        fn token(&self) -> SurfaceToken {
            // 使用 1×1 正 extent 与初始代际。
            SurfaceToken::new(0, RhiExtent::new(1, 1))
        }

        // 返回固定 acquired frame；owner 构造不会执行该方法。
        fn acquire(&mut self) -> crate::core::Result<SurfaceFrame> {
            // 使用非零不透明 target handle。
            Ok(SurfaceFrame::new(
                self.token(),
                RenderTargetHandle::from_raw(1),
            ))
        }

        // 返回请求 extent 的下一代 token；owner 构造不会执行该方法。
        fn resize(&mut self, extent: RhiExtent) -> crate::core::Result<SurfaceToken> {
            // 保留测试传入 extent 并推进代际。
            Ok(SurfaceToken::new(1, extent))
        }

        // 接受最小测试提交；owner 构造不会执行该方法。
        fn present(
            // 借用测试 surface owner。
            &mut self,
            // 忽略未执行的 acquired frame。
            _frame: SurfaceFrame,
            // 忽略未执行的 submission handle。
            _submission: SubmissionHandle,
            // 忽略未执行的 damage。
            _damage: PresentDamage,
        ) -> crate::core::Result<()> {
            // 最小适配器固定报告成功。
            Ok(())
        }
    }

    // 实现构造门禁消费的最小兼容 context 契约。
    impl IGraphicsContext for MissingRhiContext {
        // 返回测试指定的静态 recipe 事实。
        fn caps(&self) -> GraphicsContextCaps {
            // 复制无动态状态的能力快照。
            self.caps
        }

        // 按测试场景选择是否暴露完整 GPU recipe 视图。
        fn gpu_recipe_context(
            // 借用测试 context。
            &mut self,
        ) -> Option<&mut dyn GpuRecipeContext> {
            // 单个开关只能暴露同时拥有 RHI 与 resize 的原子视图。
            self.expose_gpu_recipe.then_some(self)
        }

        // 返回稳定的最小 surface 元数据。
        fn present_surface(&self) -> PresentSurface {
            // 测试不涉及真实 drawable 或 surface generation。
            PresentSurface::identity(1, 1, 1.0, 0)
        }

        // 记录构造拒绝路径执行了 checked shutdown。
        fn try_shutdown(&mut self) -> crate::core::Result<()> {
            // 将共享观察状态置为已关闭。
            self.shutdown.set(true);
            // 测试 context 的关闭固定成功。
            Ok(())
        }
    }

    // 为测试 context 实现不可拆分的 thin RHI 与 lifecycle 契约。
    impl GpuRecipeContext for MissingRhiContext {
        // 借用同一测试实例实现的组合 thin RHI。
        fn rhi_context(
            // 借用测试 recipe owner。
            &mut self,
        ) -> crate::core::Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
            // 暴露完整视图时同一实例必然同时拥有 device 与 surface。
            Ok(self)
        }

        // 使用共享 extent 规则重建测试 surface。
        fn resize_surface(&mut self, width: i32, height: i32) -> crate::core::Result<()> {
            // 在可变借用前取得测试 owner 的完整 surface 快照。
            let present_surface = IGraphicsContext::present_surface(self);
            // 复用生产 adapter 使用的唯一逻辑尺寸转换边界。
            crate::native::present::resize_native_rhi_surface(
                // 借用同一原子 GPU recipe owner。
                self,
                // 传入本次 resize 之前的稳定元数据。
                present_surface,
                // 转发逻辑宽度。
                width,
                // 转发逻辑高度。
                height,
            )
        }
    }

    // 验证错误 recipe 在进入 draw backend 前被拒绝并关闭。
    #[test]
    fn rejects_pixel_upload_recipe_before_backend_construction() {
        // 保存 owner 消费后仍可观察的关闭标记。
        let shutdown = Rc::new(Cell::new(false));
        // 构造 CPU PixelUpload recipe，故意违反 GPU owner 门禁。
        let context = MissingRhiContext {
            // 共享关闭状态给测试断言。
            shutdown: Rc::clone(&shutdown),
            // 使用合法但不属于 GPU backend 的 PixelUpload recipe。
            caps: GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan),
            // recipe 门禁会先拒绝，无需暴露 GPU recipe 视图。
            expose_gpu_recipe: false,
        };
        // 在转移 context 所有权前复制顶层分派已经读取的静态快照。
        let caps = context.caps;
        // 尝试构造 GPU owner 并取得稳定失败。
        let result = GpuRecipeOwner::try_new(Box::new(context), caps);
        // recipe 不匹配必须保持参数错误分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
        // 构造拒绝前必须检查式关闭 native owner。
        assert!(shutdown.get());
    }

    // 验证 GPU recipe 缺少原子 recipe owner 时不会进入 backend。
    #[test]
    fn rejects_gpu_recipe_without_atomic_context() {
        // 保存 owner 消费后仍可观察的关闭标记。
        let shutdown = Rc::new(Cell::new(false));
        // 构造声明 GPU recipe 但不暴露完整 recipe 视图的错误 context。
        let context = MissingRhiContext {
            // 共享关闭状态给测试断言。
            shutdown: Rc::clone(&shutdown),
            // 声明正式 GPU-native × swapchain recipe。
            caps: GraphicsContextCaps::gpu_native_swapchain(
                // 使用默认 Windows 参考 backend 身份。
                GraphicsApi::D3d11,
                // 测试只需要稳定的完整重绘 coherency。
                PresentCoherency::FullOnly,
            ),
            // 让该用例停在原子 GPU recipe 门禁。
            expose_gpu_recipe: false,
        };
        // 在转移 context 所有权前复制顶层分派已经读取的静态快照。
        let caps = context.caps;
        // 尝试构造 GPU owner 并取得稳定失败。
        let result = GpuRecipeOwner::try_new(Box::new(context), caps);
        // 必需原子 owner 缺失必须保持状态错误分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidState));
        // 构造拒绝前必须检查式关闭 native owner。
        assert!(shutdown.get());
    }

    // 验证原子 GPU recipe 视图能同时提供 thin RHI 与 resize。
    #[test]
    fn accepts_gpu_recipe_with_atomic_context() {
        // 保存 owner 完成显式关闭后可观察的标记。
        let shutdown = Rc::new(Cell::new(false));
        // 构造同时实现 thin RHI 与 resize 的完整 GPU recipe context。
        let context = MissingRhiContext {
            // 共享关闭状态给测试断言。
            shutdown: Rc::clone(&shutdown),
            // 声明正式 GPU-native × swapchain recipe。
            caps: GraphicsContextCaps::gpu_native_swapchain(
                // 使用默认 Windows 参考 backend 身份。
                GraphicsApi::D3d11,
                // 测试只需要稳定的完整重绘 coherency。
                PresentCoherency::FullOnly,
            ),
            // 一次暴露不可拆分的 GPU recipe 视图。
            expose_gpu_recipe: true,
        };
        // 在转移 context 所有权前复制顶层分派已经读取的静态快照。
        let caps = context.caps;
        // 完整原子视图必须通过 GPU owner 构造门禁。
        let mut owner = match GpuRecipeOwner::try_new(Box::new(context), caps) {
            // 保存成功构造的唯一 owner。
            Ok(owner) => owner,
            // 任何失败都说明原子视图门禁错误拒绝合法实现。
            Err(error) => panic!("atomic GPU recipe context must be accepted: {error:?}"),
        };
        // 同一 owner 必须立即提供经过 Result 收口的 thin RHI。
        assert!(owner.rhi_context().is_ok());
        // 同一 owner 必须执行逻辑 surface resize。
        assert!(owner.resize_surface(2, 3).is_ok());
        // 显式执行 checked shutdown，保持测试资源生命周期完整。
        assert!(owner.try_shutdown().is_ok());
        // 唯一 native owner 必须由同一关闭边界处理。
        assert!(shutdown.get());
    }
}
