//! 已验证 native 图形 recipe 的正交 owner 边界。

// 引入统一错误与结果类型。
use crate::core::{Errc, Error, Result};
// 引入 GPU 与 PixelUpload 的专用 owner。
use crate::platform::presentation::{GpuRecipeOwner, PixelUploadRecipeOwner};
// 引入类型化 context 与 recipe 轴。
use crate::platform::presentation::{
    GraphicsContextCaps, GraphicsRecipeContext, PresentMode, RasterMode,
};

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
