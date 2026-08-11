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
    // 按 registry 已验证的静态 recipe 选择唯一 owner，并检查式关闭不合法组合。
    pub(crate) fn try_new(
        // 接收仍未离开 native factory 的迁移期 context。
        mut context: Box<dyn IGraphicsContext>,
        // 接收 registry 唯一读取并完成一致性验证的 capability 快照。
        caps: GraphicsContextCaps,
    ) -> Result<Self> {
        // 只接受文档定义的两个正交 recipe 组合。
        match (caps.raster, caps.present) {
            // GPU recipe 交给专用 thin RHI owner 完成剩余门禁。
            (RasterMode::GpuNative, PresentMode::Swapchain) => {
                // 把同一次读取的静态快照交给 GPU 门禁并包装为稳定枚举分支。
                GpuRecipeOwner::try_new(context, caps).map(Self::Gpu)
            }
            // CPU PixelUpload recipe 交给专用 surface owner 完成剩余门禁。
            (RasterMode::Cpu, PresentMode::PixelUpload) => {
                // 把同一次读取的静态快照交给 PixelUpload 门禁并包装为稳定枚举分支。
                PixelUploadRecipeOwner::try_new(context, caps).map(Self::PixelUpload)
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
    // 引入 GPU recipe 的原子视图契约。
    use crate::native::present::GpuRecipeContext;
    // 引入 PixelUpload 的专用 surface 契约。
    use crate::native::present::PixelUploadSurface;
    // 引入 PixelUpload 最终提交使用的 damage 值。
    use crate::native::present::PresentDamage;
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

    // 提供由外部静态快照驱动的合法正交 recipe context。
    struct RecipeContext {
        // 保存当前测试分支声明的静态 recipe 事实。
        caps: GraphicsContextCaps,
    }

    // 为外部快照测试实现迁移期 context 契约。
    impl IGraphicsContext for RecipeContext {
        // 仅为 GPU-native × Swapchain 分支暴露原子 GPU recipe 视图。
        fn gpu_recipe_context(&mut self) -> Option<&mut dyn GpuRecipeContext> {
            // 先读取复制型 recipe 轴，避免返回 self 后继续借用字段。
            let is_gpu = matches!(
                // 使用同一静态快照判定合法 GPU 分支。
                (self.caps.raster, self.caps.present),
                // 只接受文档定义的 GPU-native × Swapchain 组合。
                (RasterMode::GpuNative, PresentMode::Swapchain)
            );
            // 合法 GPU 分支把同一实例作为不可拆分 recipe owner。
            is_gpu.then_some(self)
        }

        // 仅为 CPU × PixelUpload 分支暴露专用 surface 视图。
        fn pixel_upload_surface(&mut self) -> Option<&mut dyn PixelUploadSurface> {
            // 先读取复制型 recipe 轴，避免返回 self 后继续借用字段。
            let is_pixel_upload = matches!(
                // 使用同一静态快照判定合法 PixelUpload 分支。
                (self.caps.raster, self.caps.present),
                // 只接受文档定义的 CPU × PixelUpload 组合。
                (RasterMode::Cpu, PresentMode::PixelUpload)
            );
            // 合法 PixelUpload 分支把同一实例作为唯一 surface owner。
            is_pixel_upload.then_some(self)
        }

        // 返回不参与构造分派的稳定最小 surface 快照。
        fn present_surface(&self) -> PresentSurface {
            // 使用 identity drawable 避免引入真实平台资源。
            PresentSurface::identity(1, 1, 1.0, 0)
        }

        // 测试 context 的 checked shutdown 固定成功。
        fn try_shutdown(&mut self) -> crate::core::Result<()> {
            // 本测试不持有需要释放的 native 资源。
            Ok(())
        }
    }

    // 为 GPU 分支提供只用于构造门禁的原子 recipe 视图。
    impl GpuRecipeContext for RecipeContext {
        // 返回稳定错误，证明构造门禁不会提前借用真实 thin RHI。
        fn rhi_context(
            // 借用 recipe 测试 context。
            &mut self,
        ) -> crate::core::Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
            // 本测试只验证静态快照次数，不伪造 RHI 资源。
            Err(Error::new(
                // 使用稳定状态分类表达受限测试视图。
                Errc::InvalidState,
                // 保留可诊断的测试错误文本。
                "counting recipe context has no thin RHI",
            ))
        }

        // 接受不参与本测试的最小 surface resize。
        fn resize_surface(&mut self, _width: i32, _height: i32) -> crate::core::Result<()> {
            // 构造门禁不会调用 resize，固定成功即可满足窄契约。
            Ok(())
        }
    }

    // 为 PixelUpload 分支提供只用于构造门禁的专用 surface 视图。
    impl PixelUploadSurface for RecipeContext {
        // 接受不参与本测试的最小 PixelUpload resize。
        fn resize_pixel_upload_surface(
            // 借用 recipe 测试 context。
            &mut self,
            // 忽略构造门禁不会传入的逻辑宽度。
            _width: i32,
            // 忽略构造门禁不会传入的逻辑高度。
            _height: i32,
        ) -> crate::core::Result<()> {
            // 构造门禁不会调用 resize，固定成功即可满足窄契约。
            Ok(())
        }

        // 接受不参与本测试的最小像素提交。
        fn present_pixels(
            // 借用 recipe 测试 context。
            &mut self,
            // 忽略构造门禁不会传入的像素数据。
            _pixels: &[u32],
            // 忽略构造门禁不会传入的物理宽度。
            _width: i32,
            // 忽略构造门禁不会传入的物理高度。
            _height: i32,
            // 忽略构造门禁不会传入的最终 damage。
            _damage: PresentDamage,
        ) -> crate::core::Result<()> {
            // 构造门禁不会调用 present，固定成功即可满足窄契约。
            Ok(())
        }
    }

    // 构造 registry 应拒绝的 GPU raster × PixelUpload 静态交叉组合。
    fn invalid_recipe_caps() -> GraphicsContextCaps {
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

    // 为非法 recipe 测试实现最小迁移期 context。
    impl IGraphicsContext for InvalidRecipeContext {
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
        // 在模拟 registry 边界构造故意非法的静态快照。
        let caps = invalid_recipe_caps();
        // 尝试构造正交 recipe owner。
        let result = GraphicsRecipeOwner::try_new(Box::new(context), caps);
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
        // 在模拟 registry 边界构造故意非法的静态快照。
        let caps = invalid_recipe_caps();
        // 取得带原因链的构造失败。
        let error = match GraphicsRecipeOwner::try_new(Box::new(context), caps) {
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

    // 验证 GPU 正交 owner 只消费外部提供的静态 capability。
    #[test]
    fn gpu_recipe_owner_reuses_provided_static_caps() {
        // 构造合法 GPU-native × Swapchain 测试 context。
        let context = RecipeContext {
            // 使用 Windows 参考 backend 与完整提交 coherency。
            caps: GraphicsContextCaps::gpu_native_swapchain(
                // 使用稳定 D3D11 backend 身份。
                GraphicsApi::D3d11,
                // 本测试不验证局部 present。
                PresentCoherency::FullOnly,
            ),
        };
        // 模拟 registry 把已验证字段快照直接传入 owner，且不调用 trait 查询。
        let caps = context.caps;
        // 通过顶层正交 owner 构造 GPU 分支。
        let owner = GraphicsRecipeOwner::try_new(Box::new(context), caps);
        // 合法 GPU recipe 必须进入唯一 GPU owner 分支。
        assert!(matches!(&owner, Ok(GraphicsRecipeOwner::Gpu(_))));
        // 解包已经通过分支断言的 GPU owner。
        let owner = owner.expect("GPU recipe owner must exist");
        // 顶层分派必须保留外部提供的完整静态快照。
        assert_eq!(owner.caps(), caps);
    }

    // 验证 PixelUpload 正交 owner 只消费外部提供的静态 capability。
    #[test]
    fn pixel_upload_recipe_owner_reuses_provided_static_caps() {
        // 构造合法 CPU × PixelUpload 测试 context。
        let context = RecipeContext {
            // 使用 Vulkan 身份代表当前跨平台 PixelUpload recipe。
            caps: GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan),
        };
        // 模拟 registry 把已验证字段快照直接传入 owner，且不调用 trait 查询。
        let caps = context.caps;
        // 通过顶层正交 owner 构造 PixelUpload 分支。
        let owner = GraphicsRecipeOwner::try_new(Box::new(context), caps);
        // 合法 PixelUpload recipe 必须进入唯一上传 owner 分支。
        assert!(matches!(&owner, Ok(GraphicsRecipeOwner::PixelUpload(_))));
        // 解包已经通过分支断言的 PixelUpload owner。
        let owner = owner.expect("PixelUpload recipe owner must exist");
        // 顶层分派必须保留外部提供的完整静态快照。
        assert_eq!(owner.caps(), caps);
    }
}
