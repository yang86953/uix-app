//! 已验证 native 图形 recipe 的正交 owner 边界。

// 引入统一错误与结果类型。
use crate::core::{Errc, Error, Result};
// 引入 GPU 与 PixelUpload 的专用 owner。
use crate::native::present::{GpuRecipeOwner, PixelUploadRecipeOwner};
// 引入迁移期 context 与 recipe 轴。
use crate::native::present::{GraphicsContextCaps, IGraphicsContext, PresentMode, RasterMode};

// 保存离开 native factory 前已经完成构造门禁的具体 recipe owner。
pub(crate) enum GraphicsRecipeOwner {
    // GPU-native × swapchain recipe 只暴露已验证 GPU owner。
    Gpu(GpuRecipeOwner),
    // CPU × PixelUpload recipe 只暴露已验证上传 owner。
    PixelUpload(PixelUploadRecipeOwner),
}

// 把迁移期 context 收敛为 renderer 可消费的正交 owner。
impl GraphicsRecipeOwner {
    // 按构造期静态 recipe 选择唯一 owner，并检查式关闭不合法组合。
    pub(crate) fn try_new(mut context: Box<dyn IGraphicsContext>) -> Result<Self> {
        // 一次读取 context 的静态 recipe 事实用于分派。
        let caps = context.caps();
        // 只接受文档定义的两个正交 recipe 组合。
        match (caps.raster, caps.present) {
            // GPU recipe 交给专用 thin RHI owner 完成剩余门禁。
            (RasterMode::GpuNative, PresentMode::Swapchain) => {
                // 把已验证 GPU owner 包装为稳定枚举分支。
                GpuRecipeOwner::try_new(context).map(Self::Gpu)
            }
            // CPU PixelUpload recipe 交给专用 surface owner 完成剩余门禁。
            (RasterMode::Cpu, PresentMode::PixelUpload) => {
                // 把已验证 PixelUpload owner 包装为稳定枚举分支。
                PixelUploadRecipeOwner::try_new(context).map(Self::PixelUpload)
            }
            // 任何交叉组合都不能进入 renderer。
            (raster, present) => {
                // 保存原始 recipe 错误，供 cleanup failure 追加为原因。
                let error = Error::new(
                    // recipe 组合不属于支持值域，归类为参数错误。
                    Errc::InvalidArgument,
                    // 保留完整三轴事实便于 registry 诊断。
                    format!(
                        "GraphicsRecipeOwner rejects unsupported recipe: backend={} raster={} present={}",
                        caps.backend, raster, present
                    ),
                );
                // 拒绝前检查式释放已经创建的 native context。
                match context.try_shutdown() {
                    // shutdown 成功时返回原始 recipe 错误。
                    Ok(()) => Err(error),
                    // shutdown 失败时保留 cleanup 错误并链接 recipe 原因。
                    Err(cleanup_error) => Err(cleanup_error.with_source(error)),
                }
            }
        }
    }

    // 返回专用 owner 在构造期固化的静态 recipe 事实。
    pub(crate) fn caps(&self) -> GraphicsContextCaps {
        // 按枚举分支读取同一形态的 capability 快照。
        match self {
            // GPU owner 返回其不可漂移快照。
            Self::Gpu(owner) => owner.caps(),
            // PixelUpload owner 返回其不可漂移快照。
            Self::PixelUpload(owner) => owner.caps(),
        }
    }
}

// 验证非法正交组合在 native 边界被关闭并保持错误链。
#[cfg(test)]
mod tests {
    // 引入共享关闭状态。
    use std::{cell::Cell, rc::Rc};

    // 引入被测 owner 与 context 契约。
    use super::GraphicsRecipeOwner;
    // 引入错误、recipe 与 surface 测试值。
    use crate::core::{Errc, Error, PresentCoherency, PresentSurface};
    // 引入非法组合使用的 backend 身份。
    use crate::native::present::GraphicsApi;
    // 引入非法组合使用的 context capability 快照。
    use crate::native::present::GraphicsContextCaps;
    // 引入测试 context 契约。
    use crate::native::present::IGraphicsContext;
    // 引入非法组合使用的 present 轴。
    use crate::native::present::PresentMode;
    // 引入保守遮挡能力事实。
    use crate::native::present::PresentOcclusionSupport;
    // 引入非法组合使用的 raster 轴。
    use crate::native::present::RasterMode;

    // 构造故意声明非法 raster × present 组合的测试 context。
    struct InvalidRecipeContext {
        // 允许 owner 消费 Box 后继续观察 shutdown。
        shutdown: Rc<Cell<bool>>,
        // 控制 shutdown 是否返回独立 cleanup failure。
        fail_shutdown: bool,
    }

    // 为非法 recipe 测试实现最小迁移期 context。
    impl IGraphicsContext for InvalidRecipeContext {
        // 返回 GPU raster × PixelUpload 的非法交叉组合。
        fn caps(&self) -> GraphicsContextCaps {
            // 直接构造不属于两个合法构造器的静态事实。
            GraphicsContextCaps {
                // 使用稳定 D3D11 backend 身份。
                backend: GraphicsApi::D3d11,
                // GPU raster 故意与 PixelUpload present 交叉。
                raster: RasterMode::GpuNative,
                // PixelUpload 只允许与 CPU raster 组合。
                present: PresentMode::PixelUpload,
                // 非法组合不声明遮挡能力。
                present_occlusion: PresentOcclusionSupport::Unsupported,
                // 测试使用保守完整提交 coherency。
                present_coherency: PresentCoherency::FullOnly,
            }
        }

        // 返回不参与本测试的稳定 surface 快照。
        fn present_surface(&self) -> PresentSurface {
            // 使用最小 identity drawable。
            PresentSurface::identity(1, 1, 1.0, 0)
        }

        // 记录构造拒绝执行 checked shutdown。
        fn try_shutdown(&mut self) -> crate::core::Result<()> {
            // 先记录 shutdown 已被调用。
            self.shutdown.set(true);
            // 按用例选择成功或独立 cleanup failure。
            if self.fail_shutdown {
                // 返回稳定状态错误供原因链断言。
                Err(Error::new(Errc::InvalidState, "recipe test cleanup failed"))
            } else {
                // 成功关闭不添加额外错误。
                Ok(())
            }
        }
    }

    // 验证非法组合在 renderer 之前被拒绝并关闭。
    #[test]
    fn rejects_invalid_recipe_and_checks_shutdown() {
        // 创建跨 Box 生命周期共享的关闭标记。
        let shutdown = Rc::new(Cell::new(false));
        // 构造 shutdown 成功的非法 context。
        let context = InvalidRecipeContext {
            // 共享关闭观察状态。
            shutdown: Rc::clone(&shutdown),
            // 本用例只验证原始 recipe 错误。
            fail_shutdown: false,
        };
        // 尝试构造正交 recipe owner。
        let result = GraphicsRecipeOwner::try_new(Box::new(context));
        // 非法组合必须返回参数错误。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
        // 返回前必须执行 checked shutdown。
        assert!(shutdown.get());
    }

    // 验证 cleanup failure 不覆盖原始 recipe 原因。
    #[test]
    fn cleanup_failure_keeps_invalid_recipe_as_source() {
        // 创建跨 Box 生命周期共享的关闭标记。
        let shutdown = Rc::new(Cell::new(false));
        // 构造 shutdown 失败的非法 context。
        let context = InvalidRecipeContext {
            // 共享关闭观察状态。
            shutdown: Rc::clone(&shutdown),
            // 强制返回 cleanup failure。
            fail_shutdown: true,
        };
        // 取得带原因链的构造失败。
        let error = match GraphicsRecipeOwner::try_new(Box::new(context)) {
            // 成功属于测试失败。
            Ok(_) => panic!("invalid recipe must be rejected"),
            // 保存 typed 构造失败。
            Err(error) => error,
        };
        // 外层保留 cleanup failure 分类。
        assert_eq!(error.code(), Errc::InvalidState);
        // 原始非法 recipe 必须位于原因链根部。
        assert_eq!(error.root_cause().code(), Errc::InvalidArgument);
        // 原因链必须包含一层原始错误。
        assert_eq!(error.depth(), 1);
        // cleanup 失败前仍已经执行 shutdown 入口。
        assert!(shutdown.get());
    }
}
