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
    // 兼容 context 只留在本门面内部，draw backend 不再直接依赖可选视图。
    context: Box<dyn IGraphicsContext>,
}

// 为生产 GPU backend 提供不带可选能力分支的窄 owner 契约。
impl GpuRecipeOwner {
    // 校验 context 的静态 recipe 与组合 thin RHI，并在失败前检查式关闭。
    pub(crate) fn try_new(mut context: Box<dyn IGraphicsContext>) -> Result<Self> {
        // 一次读取静态 recipe 事实，避免校验期间拼装多个快照。
        let caps = context.caps();
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
        // 构造期必须证明组合 thin RHI 存在。
        if context.rhi_context().is_none() {
            // 保存具体 backend，供状态破坏诊断使用。
            let backend = caps.backend;
            // 缺失必需视图时同样先检查式关闭 native owner。
            context.try_shutdown()?;
            // 返回稳定状态错误，禁止退回 legacy adapter 路径。
            return Err(Error::new(
                Errc::InvalidState,
                format!("GraphicsBackend {backend} GPU recipe lacks a thin RHI owner"),
            ));
        }
        // 只有通过完整门禁的 context 才能进入 draw backend。
        Ok(Self { context })
    }

    // 返回构造期已验证的静态 recipe 事实。
    pub(crate) fn caps(&self) -> GraphicsContextCaps {
        // 静态事实仍由唯一 native context 提供。
        self.context.caps()
    }

    // 返回当前 drawable extent、DPR、transform 与 generation 的原子快照。
    pub(crate) fn present_surface(&self) -> PresentSurface {
        // live 元数据不在门面内拆分或重复缓存。
        self.context.present_surface()
    }

    // 借用构造期已验证的组合 thin RHI；运行期破坏保持 typed error。
    pub(crate) fn rhi_context(&mut self) -> Result<&mut dyn GraphicsContextRhi> {
        // 保存 backend 身份，避免可变借用后再次访问 context。
        let backend = self.context.caps().backend;
        // 将迁移期 Option 收口为 GPU owner 的稳定 Result 契约。
        self.context.rhi_context().ok_or_else(|| {
            // 构造后丢失必需视图属于可恢复层识别的状态破坏。
            Error::new(
                Errc::InvalidState,
                format!("GraphicsBackend {backend} GPU recipe lost its thin RHI owner"),
            )
        })
    }

    // 通过 recipe 专用生命周期视图重建 surface。
    pub(crate) fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 保存 backend 身份，供缺失生命周期视图时构造稳定错误。
        let backend = self.context.caps().backend;
        // 将可选兼容查询收口在 native owner 边界内。
        let lifecycle = self.context.rhi_surface_lifecycle().ok_or_else(|| {
            // GPU recipe 构造后丢失 resize 视图属于状态破坏。
            Error::new(
                Errc::InvalidState,
                format!("GraphicsBackend {backend} GPU recipe lost its RHI surface lifecycle"),
            )
        })?;
        // 让 thread-bound/native owner 执行唯一 surface resize 事务。
        lifecycle.resize_rhi_surface(width, height)
    }

    // 在 owner-thread 边界检查式关闭原生资源。
    pub(crate) fn try_shutdown(&mut self) -> Result<()> {
        // 保留底层 typed teardown failure，不降级为日志。
        self.context.try_shutdown()
    }
}

// 验证构造门禁会拒绝错误 recipe 与缺失 thin RHI 的 context。
#[cfg(test)]
mod tests {
    // 引入共享关闭状态所需的 Cell 与 Rc。
    use std::{cell::Cell, rc::Rc};

    // 引入被测 owner。
    use super::GpuRecipeOwner;
    // 引入错误分类与 coherency 事实。
    use crate::core::{Errc, PresentCoherency};
    // 引入最小测试 context 所需契约。
    use crate::native::present::{
        GraphicsApi, GraphicsContextCaps, IGraphicsContext, PresentSurface,
    };

    // 提供不暴露 thin RHI 的最小 context。
    struct MissingRhiContext {
        // 允许测试在 owner 消费 Box 后观察 checked shutdown。
        shutdown: Rc<Cell<bool>>,
        // 控制测试 context 声明的静态 recipe。
        caps: GraphicsContextCaps,
    }

    // 实现构造门禁消费的最小兼容 context 契约。
    impl IGraphicsContext for MissingRhiContext {
        // 返回测试指定的静态 recipe 事实。
        fn caps(&self) -> GraphicsContextCaps {
            // 复制无动态状态的能力快照。
            self.caps
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
        };
        // 尝试构造 GPU owner 并取得稳定失败。
        let result = GpuRecipeOwner::try_new(Box::new(context));
        // recipe 不匹配必须保持参数错误分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
        // 构造拒绝前必须检查式关闭 native owner。
        assert!(shutdown.get());
    }

    // 验证 GPU recipe 缺少组合 thin RHI 时不会进入 backend。
    #[test]
    fn rejects_gpu_recipe_without_thin_rhi_owner() {
        // 保存 owner 消费后仍可观察的关闭标记。
        let shutdown = Rc::new(Cell::new(false));
        // 构造声明 GPU recipe 但不实现 rhi_context 的错误 context。
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
        };
        // 尝试构造 GPU owner 并取得稳定失败。
        let result = GpuRecipeOwner::try_new(Box::new(context));
        // 必需 thin RHI 缺失必须保持状态错误分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidState));
        // 构造拒绝前必须检查式关闭 native owner。
        assert!(shutdown.get());
    }
}
