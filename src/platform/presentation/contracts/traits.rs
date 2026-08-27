//! 图形上下文公开契约。

use super::*;

// 把逻辑窗口尺寸按 DPR 转成薄 RHI 使用的物理 drawable extent。
// 只在共享 resize 事务或其纯转换测试可达时编译。
#[cfg(any(
    test,
    all(windows, feature = "d3d11"),
    feature = "opengles",
    feature = "vulkan"
))]
fn rhi_resize_extent_for_logical(
    logical_width: i32,
    logical_height: i32,
    device_pixel_ratio: f32,
) -> Result<crate::platform::presentation::rhi::RhiExtent, Error> {
    // 拒绝不能稳定转换为物理像素的 DPR，避免伪造 surface extent。
    if !device_pixel_ratio.is_finite() || device_pixel_ratio <= 0.0 {
        // 保持失败为 typed state error，让恢复层保留当前帧。
        return Err(Error::new(
            crate::core::error::Errc::InvalidState,
            "graphics context has an invalid device pixel ratio for RHI resize",
        ));
    }
    // 计算物理宽度并四舍五入到 drawable 整数像素。
    let physical_width = (logical_width.max(1) as f32 * device_pixel_ratio).round();
    // 计算物理高度并四舍五入到 drawable 整数像素。
    let physical_height = (logical_height.max(1) as f32 * device_pixel_ratio).round();
    // 拒绝溢出或非正物理尺寸，防止当前 i32 drawable 元数据接收截断值。
    if !physical_width.is_finite()
        || !physical_height.is_finite()
        || physical_width < 1.0
        || physical_height < 1.0
        || physical_width > i32::MAX as f32
        || physical_height > i32::MAX as f32
    {
        // 使用参数错误区分输入范围问题和 surface/device 故障。
        return Err(Error::new(
            crate::core::error::Errc::InvalidArgument,
            "graphics context RHI resize extent is outside the supported range",
        ));
    }
    // 返回经过范围检查的物理 extent。
    Ok(crate::platform::presentation::rhi::RhiExtent::new(
        physical_width as u32,
        physical_height as u32,
    ))
}

// 定义所有 recipe context 共同且不可选的呈现与释放生命周期。
pub(crate) trait GraphicsContextLifecycle {
    // 返回当前 drawable extent、DPR、transform 与 generation 的原子快照。
    fn present_surface(&self) -> PresentSurface;

    // 在 owner-thread 边界检查式关闭线程亲和原生资源。
    fn try_shutdown(&mut self) -> Result<(), Error>;
}

// 定义 GPU-native recipe 不可拆分的 thin RHI 与 surface 生命周期视图。
pub(crate) trait GpuRecipeContext: GraphicsContextLifecycle {
    // 返回当前可写 swapchain image 身份；FullOnly adapter 默认没有该证明。
    fn present_image(&self) -> Option<PresentImage> {
        // 默认保持无 image 身份，防止未迁移 adapter 误用 tracked history。
        None
    }

    // 借用当前 recipe 唯一的 thin RHI owner，并保留 typed failure。
    fn rhi_context(
        // 借用 GPU recipe owner。
        &mut self,
    ) -> Result<&mut dyn crate::platform::presentation::rhi::GraphicsContextRhi, Error>;

    // 按逻辑窗口尺寸重建同一 owner 的 thin RHI surface 并推进代际。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<(), Error>;
}

// 定义只属于 CPU PixelUpload recipe 的 surface 生命周期与提交契约。
pub(crate) trait PixelUploadSurface: GraphicsContextLifecycle {
    // 按逻辑窗口尺寸重建像素上传 surface。
    fn resize_pixel_upload_surface(&mut self, width: i32, height: i32) -> Result<(), Error>;

    // 把 CPU retained pixels 提交给当前 PixelUpload surface。
    fn present_pixels(
        // 借用当前 PixelUpload surface owner。
        &mut self,
        // 接收 premultiplied BGRA 像素。
        pixels: &[u32],
        // 接收物理像素宽度。
        width: i32,
        // 接收物理像素高度。
        height: i32,
        // 接收最终提交 damage。
        damage: PresentDamage,
    ) -> Result<(), Error>;
}

// 为直接拥有薄 RHI 的原生 context 执行共享 resize 事务。
// 与实际调用方保持同一 backend/test 构建边界，避免无 backend 编译该事务。
#[cfg(any(
    test,
    all(windows, feature = "d3d11"),
    feature = "opengles",
    feature = "vulkan"
))]
pub(crate) fn resize_native_rhi_surface(
    // 借用不可拆分的 GPU recipe owner。
    context: &mut dyn GpuRecipeContext,
    // 接收调用前取得的完整 live surface 快照。
    present_surface: PresentSurface,
    // 接收逻辑窗口宽度。
    width: i32,
    // 接收逻辑窗口高度。
    height: i32,
) -> Result<(), Error> {
    // 把非法或零尺寸归一化为窗口生命周期允许的最小逻辑尺寸。
    let logical_width = width.max(1);
    // 把非法或零尺寸归一化为窗口生命周期允许的最小逻辑尺寸。
    let logical_height = height.max(1);
    // 从单一 live surface 快照读取设备像素比，避免分离元数据发生撕裂。
    let device_pixel_ratio = present_surface.device_pixel_ratio;
    // 统一把逻辑尺寸和 DPR 转成经过范围证明的物理 RHI extent。
    let extent = rhi_resize_extent_for_logical(logical_width, logical_height, device_pixel_ratio)?;
    // 原子 GPU recipe 视图直接返回唯一 thin RHI owner 或 typed failure。
    let rhi = context.rhi_context()?;
    // 将逻辑窗口尺寸转换后的物理 extent 交给唯一 GraphicsSurface 生命周期。
    rhi.surface().resize(extent)?;
    // 只报告 RHI surface 重建成功，不在此处触碰最终 present。
    Ok(())
}
