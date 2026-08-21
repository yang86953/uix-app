//! 主 surface retained texture 的滚动搬移 lowering。

// 引入统一错误类型。
use crate::core::{Errc, Error};
// 引入只写入 retained texture 的 FramePlan 类型。
use crate::draw::backend::frame_plan::FramePlan;
// 引入薄 RHI 的组合 context、目标和搬移原语。
use crate::platform::presentation::rhi::{
    RhiExtent, RhiTextureTransfer, TextureHandle, TextureMove,
};

// 引入当前 GPU backend 和待处理的逻辑搬移记录。
use super::GpuBackend;
use super::surface::PendingScrollCopy;

// 把一个非负逻辑数按当前 DPR 转成精确的物理整数。
fn scale_integral(value: i64, scale: f32) -> Option<u32> {
    // 负值、非有限缩放和超过物理句柄范围的坐标都不能进入 TextureMove。
    if value < 0 || !scale.is_finite() || scale <= 0.0 {
        // 交回兼容路径，由上层决定是否全帧重绘。
        return None;
    }
    // 计算逻辑像素边界对应的物理位置。
    let physical = value as f32 * scale;
    // 物理位置必须有限且已经落在整数像素边界。
    if !physical.is_finite() || (physical - physical.round()).abs() > 0.001 {
        // 非整数 DPR 映射不能被整数 TextureMove 无损表达。
        return None;
    }
    // 转换前限制到通用 RHI 的 u32 坐标范围。
    let rounded = physical.round();
    if rounded < 0.0 || rounded > u32::MAX as f32 {
        // 防止窄整数转换回绕。
        return None;
    }
    // 返回精确的物理整数边界。
    Some(rounded as u32)
}

// 复用 CPU surface 的裁剪规则，把逻辑 source/destination 变成物理 move。
// 把一个逻辑 source/destination copy 降低为同一纹理上的物理搬移。
// 允许原生 Canvas2D 与 DrawSurface 共享同一套 scroll lowering 证明。
pub(crate) fn lower_scroll_copy(
    copy: PendingScrollCopy,
    target: TextureHandle,
    logical_width: i32,
    logical_height: i32,
    scale_x: f32,
    scale_y: f32,
    extent: RhiExtent,
) -> Result<Option<TextureMove>, Error> {
    // 拒绝不能表达为整数像素搬移的逻辑输入。
    if !copy.source.x.is_finite()
        || !copy.source.y.is_finite()
        || !copy.source.w.is_finite()
        || !copy.source.h.is_finite()
        || !copy.destination.x.is_finite()
        || !copy.destination.y.is_finite()
        || copy.source.w <= 0.0
        || copy.source.h <= 0.0
    {
        // 保留原有 copy_region 的 no-op/fallback 边界，不伪造坐标。
        return Err(Error::new(
            Errc::NotImplemented,
            "RHI scroll copy has non-finite or empty logical geometry",
        ));
    }
    // 与 PixelSurface 保持相同的饱和整数化语义。
    let source_x = copy.source.x as i32 as i64;
    // 保存逻辑 source 顶部。
    let source_y = copy.source.y as i32 as i64;
    // 保存逻辑 source 宽度。
    let copy_width = copy.source.w as i32 as i64;
    // 保存逻辑 source 高度。
    let copy_height = copy.source.h as i32 as i64;
    // 空整数区域等价于 CPU no-op。
    if copy_width <= 0 || copy_height <= 0 {
        // 不提交空 TextureMove。
        return Ok(None);
    }
    // 保存逻辑目标左上角。
    let destination_x = copy.destination.x as i32 as i64;
    // 保存逻辑目标顶部。
    let destination_y = copy.destination.y as i32 as i64;
    // 计算 source 到 destination 的整数平移。
    let delta_x = destination_x - source_x;
    // 计算 source 到 destination 的整数垂直平移。
    let delta_y = destination_y - source_y;
    // 保存逻辑 surface 边界。
    let surface_width = i64::from(logical_width.max(1));
    // 保存逻辑 surface 高度边界。
    let surface_height = i64::from(logical_height.max(1));
    // 裁剪 source 和 destination 的共同可见区域。
    let x0 = source_x.max(0).max(-delta_x);
    // 裁剪垂直 source 和 destination 的共同可见区域。
    let y0 = source_y.max(0).max(-delta_y);
    // 计算不包含右边界的 source 终点。
    let x1 = (source_x + copy_width)
        .min(surface_width)
        .min(surface_width - delta_x);
    // 计算不包含底边界的 source 终点。
    let y1 = (source_y + copy_height)
        .min(surface_height)
        .min(surface_height - delta_y);
    // 完全越界等价于 CPU no-op。
    if x0 >= x1 || y0 >= y1 {
        // 不提交空 TextureMove。
        return Ok(None);
    }
    // 将 source 左边界转换成精确物理整数。
    let source_x = scale_integral(x0, scale_x).ok_or_else(|| {
        Error::new(
            Errc::NotImplemented,
            "RHI scroll copy source x is not an integral physical pixel",
        )
    })?;
    // 将 source 顶边界转换成精确物理整数。
    let source_y = scale_integral(y0, scale_y).ok_or_else(|| {
        Error::new(
            Errc::NotImplemented,
            "RHI scroll copy source y is not an integral physical pixel",
        )
    })?;
    // 将 destination 左边界转换成精确物理整数。
    let destination_x = scale_integral(x0 + delta_x, scale_x).ok_or_else(|| {
        Error::new(
            Errc::NotImplemented,
            "RHI scroll copy destination x is not an integral physical pixel",
        )
    })?;
    // 将 destination 顶边界转换成精确物理整数。
    let destination_y = scale_integral(y0 + delta_y, scale_y).ok_or_else(|| {
        Error::new(
            Errc::NotImplemented,
            "RHI scroll copy destination y is not an integral physical pixel",
        )
    })?;
    // 将共同可见宽度转换成精确物理整数。
    let width = scale_integral(x1 - x0, scale_x).ok_or_else(|| {
        Error::new(
            Errc::NotImplemented,
            "RHI scroll copy width is not an integral physical pixel",
        )
    })?;
    // 将共同可见高度转换成精确物理整数。
    let height = scale_integral(y1 - y0, scale_y).ok_or_else(|| {
        Error::new(
            Errc::NotImplemented,
            "RHI scroll copy height is not an integral physical pixel",
        )
    })?;
    // 物理宽高必须非空。
    if width == 0 || height == 0 {
        // 不提交空 TextureMove。
        return Ok(None);
    }
    // 检查 source 和 destination 均落在 retained texture 内。
    if source_x
        .checked_add(width)
        .is_none_or(|right| right > extent.width)
        || source_y
            .checked_add(height)
            .is_none_or(|bottom| bottom > extent.height)
        || destination_x
            .checked_add(width)
            .is_none_or(|right| right > extent.width)
        || destination_y
            .checked_add(height)
            .is_none_or(|bottom| bottom > extent.height)
    {
        // 物理裁剪不完整时不能把 source/destination 错位提交。
        return Err(Error::new(
            Errc::NotImplemented,
            "RHI scroll copy physical bounds exceed retained texture",
        ));
    }
    // 返回同一纹理上的重叠安全搬移。
    Ok(Some(TextureMove::new(
        // 从 retained 目标读取。
        target,
        // 写回同一个 retained 目标。
        target,
        // 将已裁剪的两端原点与唯一尺寸封闭为传输。
        RhiTextureTransfer::from_xy(
            // 保存源横坐标。
            source_x,
            // 保存源纵坐标。
            source_y,
            // 保存目标横坐标。
            destination_x,
            // 保存目标纵坐标。
            destination_y,
            // 保存共同物理尺寸。
            RhiExtent::new(width, height),
        ),
    )))
}

// 为 GpuBackend 提供 retained surface 的 scroll TextureMove 计划。
impl GpuBackend {
    // 把待处理滚动写入 retained texture，但不触发 swapchain present。
    pub(super) fn try_apply_rhi_surface_scroll_copies(
        &mut self,
        target: TextureHandle,
    ) -> Result<bool, Error> {
        // 没有待处理滚动时不改变当前 RHI 状态。
        if self.surface.pending_scroll_copies.is_empty() {
            // 调用方可以继续处理其它 native queue。
            return Ok(true);
        }
        // 未初始化的 retained texture 逻辑上仍是透明空白，滚动空白区域等价于 no-op。
        if self.surface.needs_gpu_clear {
            // 消费记录但保留 needs_gpu_clear，让后续首个绘制仍执行整目标清理。
            self.surface.pending_scroll_copies.clear();
            return Ok(true);
        }
        // 读取当前 RHI context 的物理几何和 surface token。
        let (surface, scale_x, scale_y) = {
            // 只有组合 RHI context 能写入 owner-thread retained texture。
            // 已验证 owner 丢失时返回 typed failure，不伪造搬移成功。
            let context = self.gpu_ctx.rhi_context()?;
            // 冻结计划必须匹配的 Surface 代际与物理范围。
            let surface = context.surface_ref().token();
            // 复用主 surface 的 mixed-DPI 几何规则。
            let (_, scale_x, scale_y) = super::super::submit::rhi_physical_geometry(
                surface.extent,
                self.surface.width,
                self.surface.height,
            );
            // 返回同一快照导出的代际与物理几何。
            (surface, scale_x, scale_y)
        };
        // 预先降低所有搬移，任何一条不安全都不消费原始记录。
        let mut movements = Vec::with_capacity(self.surface.pending_scroll_copies.len());
        for copy in &self.surface.pending_scroll_copies {
            // 将逻辑 copy 按 CPU 规则裁剪并映射到物理 texture。
            match lower_scroll_copy(
                *copy,
                target,
                self.surface.width,
                self.surface.height,
                scale_x,
                scale_y,
                surface.extent,
            ) {
                // 保存有效的物理 move。
                Ok(Some(movement)) => movements.push(movement),
                // CPU no-op 不需要生成 RHI command。
                Ok(None) => {}
                // 不可证明的几何交回兼容路径。
                Err(error) if error.code() == Errc::NotImplemented => return Ok(false),
                // 其它错误必须原样传播。
                Err(error) => return Err(error),
            }
        }
        // 所有 copy 都是 CPU no-op 时消费记录但不提交空计划。
        if movements.is_empty() {
            // 保持和 CPU surface 一样的 no-op 结果。
            self.surface.pending_scroll_copies.clear();
            return Ok(true);
        }
        // 在移动集合被按序转入 FramePlan 前保存本次可验证命令数量。
        let movement_count = movements.len();
        // retained texture 移动只属于 device，不依赖 swapchain generation。
        let mut plan = FramePlan::offscreen();
        // 按 lowering 顺序追加 retained pixels 的移动。
        for movement in movements {
            // 保留调用方已经确定的 scroll 顺序。
            plan.push_move(movement);
        }
        // 通过 device 窄视图执行，不获取也不呈现 swapchain image。
        // device 在计划构造后失效时返回 typed failure。
        let context = self.gpu_ctx.rhi_device()?;
        // 返回底层执行的真实结果，禁止吞掉设备错误。
        plan.execute_offscreen_on_device(context)?;
        // 只有共享 FramePlan 已经成功执行后才发布测试证据。
        #[cfg(feature = "test-harness")]
        {
            // 同一帧可能包含多个滚动视口，因此按成功计划累计。
            self.executed_texture_moves_in_frame = self
                // 保留本帧先前已经成功执行的移动数量。
                .executed_texture_moves_in_frame
                // 饱和累计当前计划，避免诊断计数理论上的 usize 回绕。
                .saturating_add(movement_count);
        }
        // 只有 move plan 成功提交后才消费原始逻辑 copy。
        self.surface.pending_scroll_copies.clear();
        // retained texture 现在有待最终合成的内容。
        self.rhi_surface_frame_pending_present = true;
        // 告知调用方当前 boundary 已安全写入 retained texture。
        Ok(true)
    }
}
