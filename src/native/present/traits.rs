//! 图形上下文公开契约。

use super::*;

// 把逻辑窗口尺寸按 DPR 转成薄 RHI 使用的物理 drawable extent。
fn rhi_resize_extent_for_logical(
    logical_width: i32,
    logical_height: i32,
    device_pixel_ratio: f32,
) -> Result<crate::native::present::rhi::RhiExtent, Error> {
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
    Ok(crate::native::present::rhi::RhiExtent::new(
        physical_width as u32,
        physical_height as u32,
    ))
}

// 定义 GPU-native recipe 不可拆分的 thin RHI 与 surface 生命周期视图。
pub(crate) trait GpuRecipeContext {
    // 借用当前 recipe 唯一的 thin RHI owner，并保留 typed failure。
    fn rhi_context(
        // 借用 GPU recipe owner。
        &mut self,
    ) -> Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi, Error>;

    // 按逻辑窗口尺寸重建同一 owner 的 thin RHI surface 并推进代际。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<(), Error>;
}

// 定义只属于 CPU PixelUpload recipe 的 surface 生命周期与提交契约。
pub(crate) trait PixelUploadSurface {
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

pub trait IGraphicsContext {
    fn caps(&self) -> GraphicsContextCaps;

    // 返回不可拆分的 GPU-native recipe 视图；其它 recipe 保持 None。
    // native::present 模块本身是 crate 私有边界，thin RHI 不作为外部句柄暴露。
    #[allow(private_interfaces)]
    fn gpu_recipe_context(&mut self) -> Option<&mut dyn GpuRecipeContext> {
        // CPU PixelUpload 与不支持 GPU recipe 的 context 默认不暴露该契约。
        None
    }

    // 返回 CPU PixelUpload recipe 的专用 surface 视图。
    #[allow(private_interfaces)]
    fn pixel_upload_surface(&mut self) -> Option<&mut dyn PixelUploadSurface> {
        // GPU-native 与不支持像素上传的 context 默认不暴露该契约。
        None
    }

    // 返回当前 drawable extent、DPR、transform 与 generation 的原子快照。
    fn present_surface(&self) -> PresentSurface;

    /// Checked shutdown boundary for thread-affine native resources.
    ///
    /// Callers and Drop paths must use this method. Teardown failures stay
    /// typed so recovery can retain the previous owner instead of logging only.
    fn try_shutdown(&mut self) -> Result<(), Error>;
}

// 为直接拥有薄 RHI 的原生 context 执行共享 resize 事务。
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
    rhi.resize(extent)?;
    // 只报告 RHI surface 重建成功，不在此处触碰最终 present。
    Ok(())
}

// 验证逻辑尺寸到物理 RHI extent 的纯转换契约。
#[cfg(test)]
mod tests {
    // 引入待验证的内部转换函数。
    use super::rhi_resize_extent_for_logical;

    // 验证有效 DPR 能按比例放大逻辑窗口尺寸。
    #[test]
    fn rhi_resize_extent_scales_logical_dimensions() {
        // 计算 900×640 在 1.5 DPR 下的物理 extent。
        let extent = match rhi_resize_extent_for_logical(900, 640, 1.5) {
            // 有效输入必须成功转换。
            Ok(extent) => extent,
            // 转换失败说明尺寸计算边界有缺口。
            Err(error) => panic!("valid DPR must produce a physical RHI extent: {error:?}"),
        };
        // 验证宽度按同一 DPR 转换。
        assert_eq!(extent.width, 1350);
        // 验证高度按同一 DPR 转换。
        assert_eq!(extent.height, 960);
    }

    // 验证无效 DPR 不会被静默修正为可用 surface。
    #[test]
    fn rhi_resize_extent_rejects_invalid_device_pixel_ratio() {
        // 传入零 DPR，要求返回明确的 state error。
        let result = rhi_resize_extent_for_logical(1200, 800, 0.0);
        // 验证调用方可以按 typed error 分类恢复。
        assert!(matches!(
            result,
            Err(error) if error.code() == crate::core::Errc::InvalidState
        ));
    }

    // 验证物理 extent 不会超过 adapter 当前的 i32 drawable 表示范围。
    #[test]
    fn rhi_resize_extent_rejects_adapter_extent_overflow() {
        // 让最大逻辑尺寸在 2.0 DPR 下产生超出 i32 的物理宽高。
        let result = rhi_resize_extent_for_logical(i32::MAX, i32::MAX, 2.0);
        // 验证范围错误不会被截断后交给 native surface。
        assert!(matches!(
            result,
            Err(error) if error.code() == crate::core::Errc::InvalidArgument
        ));
    }
}
