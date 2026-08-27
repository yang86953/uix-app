//! CPU PixelUpload recipe 的已验证 owner 门面。

// 引入统一结果与呈现 damage。
use crate::core::Result;
// 引入类型化 context、静态 recipe 与原子 surface 快照。
use crate::platform::presentation::{
    GraphicsContextCaps, PixelUploadSurface, PresentDamage, PresentMode, PresentSurface, RasterMode,
};

// 保存已经通过 CPU PixelUpload recipe 门禁的原生 context owner。
pub(crate) struct PixelUploadRecipeOwner {
    // 固化构造门禁验证过的静态 recipe 与 backend 事实。
    caps: GraphicsContextCaps,
    // 类型化 context 只留在本门面内部，renderer 不再承担能力查询。
    context: Box<dyn PixelUploadSurface>,
}

// 为 PixelUpload presentation 提供不带可选能力分支的窄 owner 契约。
impl PixelUploadRecipeOwner {
    // 校验顶层 owner 已捕获的静态 recipe，并在失败前检查式关闭。
    pub(super) fn try_new(
        // 接收仍由本门面唯一拥有的类型化 PixelUpload context。
        mut context: Box<dyn PixelUploadSurface>,
        // 接收正交 owner 分派时已经捕获的静态 capability 快照。
        caps: GraphicsContextCaps,
    ) -> Result<Self> {
        // PixelUpload owner 只接受 CPU × PixelUpload 组合。
        if caps.raster != RasterMode::Cpu || caps.present != PresentMode::PixelUpload {
            // 保存稳定诊断，供 cleanup failure 追加为原因。
            let error = crate::core::Error::new(
                // recipe 组合不属于 PixelUpload owner 值域。
                crate::core::Errc::InvalidArgument,
                // 保留完整静态 recipe 事实。
                format!(
                    "PixelUploadRecipeOwner requires cpu x pixel_upload, got {} raster={} present={}",
                    caps.backend, caps.raster, caps.present
                ),
            );
            // 构造失败前检查式释放原生资源并保留原始原因。
            return match context.try_shutdown() {
                // shutdown 成功时返回 recipe 错误。
                Ok(()) => Err(error),
                // shutdown 失败时链接原始 recipe 错误。
                Err(cleanup_error) => Err(cleanup_error.with_source(error)),
            };
        }
        // 类型系统已经证明 PixelUpload surface 的完整形状。
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

    // 通过专用 PixelUpload surface 重建 native drawable。
    pub(crate) fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 归一化逻辑尺寸后交给唯一 native surface owner。
        self.context
            // 禁止非正尺寸越过 recipe owner 边界。
            .resize_pixel_upload_surface(width.max(1), height.max(1))
    }

    // 把 CPU retained pixels 提交给专用 PixelUpload surface。
    pub(crate) fn present_pixels(
        // 借用已经验证的 PixelUpload owner。
        &mut self,
        // 接收 premultiplied BGRA 像素。
        pixels: &[u32],
        // 接收物理像素宽度。
        width: i32,
        // 接收物理像素高度。
        height: i32,
        // 接收最终提交 damage。
        damage: PresentDamage,
    ) -> Result<()> {
        // 最终像素提交只经过 recipe 专用 surface。
        self.context.present_pixels(pixels, width, height, damage)
    }

    // 在 owner-thread 边界检查式关闭原生资源。
    pub(crate) fn try_shutdown(&mut self) -> Result<()> {
        // 保留底层 typed teardown failure，不降级为日志。
        self.context.try_shutdown()
    }
}
