//! 已验证 native 图形 recipe 的正交 owner 边界。

// 引入统一错误与结果类型。
use crate::core::{Errc, Error, Result};
// 引入 GPU 与 PixelUpload 的专用 owner。
use crate::native::present::{GpuRecipeOwner, PixelUploadRecipeOwner};
// 引入类型化 context 与 recipe 轴。
use crate::native::present::{GraphicsContextCaps, GraphicsRecipeContext, PresentMode, RasterMode};

// 保存离开 native factory 前已经完成构造门禁的具体 recipe owner。
pub(crate) enum GraphicsRecipeOwner {
    // GPU-native × swapchain recipe 只暴露已验证 GPU owner。
    Gpu(GpuRecipeOwner),
    // CPU × PixelUpload recipe 只暴露已验证上传 owner。
    PixelUpload(PixelUploadRecipeOwner),
}

// 把类型化 context 收敛为 renderer 可消费的正交 owner。
impl GraphicsRecipeOwner {
    // 按 registry 已验证的 context 类型与静态 recipe 选择唯一 owner。
    pub(crate) fn try_new(
        // 接收仍未离开 native factory 的类型化 recipe context。
        context: GraphicsRecipeContext,
        // 接收 registry 唯一读取并完成一致性验证的 capability 快照。
        caps: GraphicsContextCaps,
    ) -> Result<Self> {
        // 只接受 context 类型与静态 recipe 完全一致的两个组合。
        match (context, caps.raster, caps.present) {
            // 类型化 GPU recipe 交给专用 thin RHI owner。
            (
                GraphicsRecipeContext::Gpu(context),
                RasterMode::GpuNative,
                PresentMode::Swapchain,
            ) => {
                // 把同一次读取的静态快照交给 GPU owner。
                GpuRecipeOwner::try_new(context, caps).map(Self::Gpu)
            }
            // 类型化 CPU PixelUpload recipe 交给专用 surface owner。
            (
                GraphicsRecipeContext::PixelUpload(context),
                RasterMode::Cpu,
                PresentMode::PixelUpload,
            ) => {
                // 把同一次读取的静态快照交给 PixelUpload owner。
                PixelUploadRecipeOwner::try_new(context, caps).map(Self::PixelUpload)
            }
            // 任何类型与静态轴交叉组合都不能进入 renderer。
            (mut context, raster, present) => {
                // 保存原始 recipe 错误，供 cleanup failure 追加为原因。
                let error = Error::new(
                    // recipe 组合不属于支持值域，归类为参数错误。
                    Errc::InvalidArgument,
                    // 保留完整三轴事实便于 registry 诊断。
                    format!(
                        "GraphicsRecipeOwner rejects typed context mismatch: backend={} raster={} present={}",
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

// 验证类型化 context 与静态 recipe 轴必须一致。
#[cfg(test)]
mod tests {
    // 引入共享关闭状态。
    use std::{cell::Cell, rc::Rc};

    // 引入被测 owner。
    use super::GraphicsRecipeOwner;
    // 引入错误分类、coherency 与 surface 值。
    use crate::core::{Errc, PresentCoherency, PresentSurface};
    // 引入类型化 PixelUpload context 与 recipe 值。
    use crate::native::present::{
        GraphicsApi, GraphicsContextCaps, GraphicsContextLifecycle, GraphicsRecipeContext,
        PixelUploadSurface, PresentDamage,
    };

    // 提供可观察 checked shutdown 的最小 PixelUpload context。
    struct TypedPixelContext {
        // 保存外部可观察的关闭标记。
        shutdown: Rc<Cell<bool>>,
    }

    // 实现共同生命周期。
    impl GraphicsContextLifecycle for TypedPixelContext {
        // 返回稳定最小 surface 快照。
        fn present_surface(&self) -> PresentSurface {
            // 测试不触碰真实 drawable。
            PresentSurface::identity(1, 1, 1.0, 0)
        }

        // 记录 checked shutdown。
        fn try_shutdown(&mut self) -> crate::core::Result<()> {
            // 让 owner 消费 Box 后仍可观察释放。
            self.shutdown.set(true);
            // 测试 context 固定关闭成功。
            Ok(())
        }
    }

    // 实现最小 PixelUpload recipe 契约。
    impl PixelUploadSurface for TypedPixelContext {
        // 测试 resize 固定成功。
        fn resize_pixel_upload_surface(
            // 借用上传 owner。
            &mut self,
            // 忽略未参与断言的宽度。
            _width: i32,
            // 忽略未参与断言的高度。
            _height: i32,
        ) -> crate::core::Result<()> {
            // 最小生命周期接受 resize。
            Ok(())
        }

        // 测试提交固定成功。
        fn present_pixels(
            // 借用上传 owner。
            &mut self,
            // 忽略未参与断言的像素。
            _pixels: &[u32],
            // 忽略未参与断言的宽度。
            _width: i32,
            // 忽略未参与断言的高度。
            _height: i32,
            // 忽略未参与断言的 damage。
            _damage: PresentDamage,
        ) -> crate::core::Result<()> {
            // 最小生命周期接受提交。
            Ok(())
        }
    }

    // 验证类型化分支与静态 recipe 错配时先关闭 owner。
    #[test]
    fn rejects_typed_context_variant_mismatch_and_shuts_down() {
        // 创建外部可观察的关闭标记。
        let shutdown = Rc::new(Cell::new(false));
        // 构造类型化 PixelUpload 分支。
        let context = GraphicsRecipeContext::PixelUpload(Box::new(TypedPixelContext {
            // 共享关闭标记。
            shutdown: Rc::clone(&shutdown),
        }));
        // 故意提供 GPU-native × Swapchain capability。
        let caps = GraphicsContextCaps::gpu_native_swapchain(
            // 使用 D3D11 backend 身份。
            GraphicsApi::D3d11,
            // 测试只需要完整重绘 coherency。
            PresentCoherency::FullOnly,
        );
        // 正交 owner 必须拒绝类型与静态轴错配。
        let result = GraphicsRecipeOwner::try_new(context, caps);
        // 错配保持参数错误分类。
        assert!(matches!(result, Err(error) if error.code() == Errc::InvalidArgument));
        // 拒绝前必须执行 checked shutdown。
        assert!(shutdown.get());
    }

    // 验证匹配的 PixelUpload 类型与静态轴进入专用 owner。
    #[test]
    fn accepts_matching_typed_pixel_upload_recipe() {
        // 创建外部可观察的关闭标记。
        let shutdown = Rc::new(Cell::new(false));
        // 构造类型化 PixelUpload 分支。
        let context = GraphicsRecipeContext::PixelUpload(Box::new(TypedPixelContext {
            // 共享关闭标记。
            shutdown,
        }));
        // 提供匹配的 CPU × PixelUpload capability。
        let caps = GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan);
        // 匹配类型必须构造成功。
        let owner = GraphicsRecipeOwner::try_new(context, caps)
            // 构造失败表示类型化分派回归。
            .expect("matching typed PixelUpload recipe must construct");
        // owner 固化的 capability 必须保持同一快照。
        assert_eq!(owner.caps(), caps);
    }
}
