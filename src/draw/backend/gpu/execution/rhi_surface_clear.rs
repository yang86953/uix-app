//! 主 surface retained texture 的局部清理 lowering。

// 引入统一错误类型。
use crate::core::{Errc, Error};
// 引入可执行的局部清理计划类型。
use crate::draw::backend::frame_plan::{
    FramePlan, FramePlanCommand, RenderPassPlan, RenderTargetRef,
};
// 引入薄 RHI 的颜色、目标、viewport 和 surface 代际类型。
use crate::platform::presentation::rhi::{
    LoadAction, RhiColor, RhiExtent, RhiScissor, TextureHandle,
};
// 引入所属 graphics backend Module 的主 surface 清理原语。
use super::super::GpuSolidRect;

// 引入当前 GPU backend 类型。
use super::GpuBackend;

// 把逻辑清理记录转换成当前 drawable 的物理整数矩形。
fn physical_clear_scissor(
    rect: GpuSolidRect,
    scale_x: f32,
    scale_y: f32,
    extent: RhiExtent,
) -> Option<RhiScissor> {
    // 清理记录只能表达有限的轴对齐正矩形。
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.w.is_finite()
        || !rect.h.is_finite()
        || rect.w <= 0.0
        || rect.h <= 0.0
        || !rect.radius.iter().all(|radius| *radius == 0.0)
        || !rect.rgba.iter().all(|channel| channel.is_finite())
        || !scale_x.is_finite()
        || !scale_y.is_finite()
    {
        // 不把无法证明的清理范围投递给原生 adapter。
        return None;
    }
    // RhiScissor 使用 i32，因此超大 drawable 必须回到兼容路径。
    if extent.width > i32::MAX as u32 || extent.height > i32::MAX as u32 {
        // 防止物理坐标转换发生窄整数回绕。
        return None;
    }
    // 读取物理 drawable 的浮点边界。
    let max_x = extent.width as f32;
    let max_y = extent.height as f32;
    // 用绝对远端换算并裁剪，避免缩放后少覆盖一列像素。
    let x0 = (rect.x * scale_x).floor().max(0.0).min(max_x);
    let y0 = (rect.y * scale_y).floor().max(0.0).min(max_y);
    let x1 = ((rect.x + rect.w) * scale_x).ceil().max(0.0).min(max_x);
    let y1 = ((rect.y + rect.h) * scale_y).ceil().max(0.0).min(max_y);
    // 完全不可见的清理不应被误译成全幅清理。
    if x1 <= x0 || y1 <= y0 {
        // 上层会把这类不确定记录交回兼容路径。
        return None;
    }
    // 浮点边界已经裁到 i32 范围，转换后保持非负。
    let x = x0 as i32;
    // 保存左上原点的物理顶部。
    let y = y0 as i32;
    // 保存已经取整的物理宽度。
    let width = (x1 as i32).checked_sub(x)?;
    // 保存已经取整的物理高度。
    let height = (y1 as i32).checked_sub(y)?;
    // 继续拒绝取整后为空的区域。
    if width <= 0 || height <= 0 {
        // 不生成无效的 FramePlan command。
        return None;
    }
    // 返回跨 adapter 统一的物理清理矩形。
    Some(RhiScissor {
        x,
        y,
        width,
        height,
    })
}

// 为 GpuBackend 提供 retained surface 的局部清理计划。
impl GpuBackend {
    // 将 pending clear rects 写入 retained texture，但不触发 swapchain present。
    pub(super) fn try_clear_rhi_surface_rects(
        &mut self,
        target: TextureHandle,
    ) -> Result<bool, Error> {
        // 没有局部清理时不改变当前 RHI 状态。
        if self.surface.pending_clear_rects.is_empty() {
            // 调用方可以继续处理 native queue。
            return Ok(true);
        }
        // 读取当前 RHI context 的 surface 代际和物理几何。
        let (surface, scale_x, scale_y) = {
            // 只有组合 RHI context 能写入 owner-thread retained texture。
            // 已验证 owner 丢失时返回 typed failure，不能伪造可回退能力。
            let context = self.gpu_ctx.rhi_context()?;
            // adapter 未声明 ClearRect 时不得把可选原语当成已执行。
            if !context.device_ref().device_capabilities().clear_rect {
                // 调用方继续使用兼容 clear_rects 路径。
                return Ok(false);
            }
            // 冻结计划必须匹配的 Surface 代际与物理范围。
            let surface = context.surface_ref().token();
            // 复用主 surface 的 mixed-DPI 几何规则。
            let (_, scale_x, scale_y) = super::super::submit::rhi_physical_geometry(
                surface.extent,
                self.surface.width,
                self.surface.height,
            );
            // 返回同一快照导出的代际与物理缩放事实。
            (surface, scale_x, scale_y)
        };
        // 创建只针对 retained texture 的 load pass。
        let mut pass = RenderPassPlan::new(RenderTargetRef::Texture(target), LoadAction::Load);
        // 以原始记录顺序追加局部清理命令。
        for rect in &self.surface.pending_clear_rects {
            // 将逻辑清理矩形转换成当前 drawable 的物理区域。
            let Some(scissor) = physical_clear_scissor(*rect, scale_x, scale_y, surface.extent)
            else {
                // 无法证明的清理不得被 RHI 忽略，交回 legacy 路径。
                return Ok(false);
            };
            // 使用 pending 记录中的颜色，当前生产路径为透明 premultiplied black。
            pass.push(FramePlanCommand::ClearRect {
                color: RhiColor::from_premultiplied_rgba(rect.rgba),
                scissor,
            });
        }
        // retained texture 清理只属于 device，不依赖 swapchain generation。
        let mut plan = FramePlan::offscreen();
        // 追加唯一离屏 pass，保持同一 owner-thread 的顺序提交。
        plan.push_pass(pass);
        // 通过 device 窄视图执行计划，不获取也不呈现 swapchain image。
        // device 在计划构造后失效时返回 typed failure。
        let context = self.gpu_ctx.rhi_device()?;
        // 返回底层执行的真实结果，禁止吞掉平台错误。
        match plan.execute_offscreen_on_device(context) {
            // 局部清理成功落入 retained texture。
            Ok(_) => Ok(true),
            // adapter 明确不支持 ClearRect 时安全回退到 legacy。
            Err(error) if error.code() == Errc::NotImplemented => Ok(false),
            // 其它错误必须保留，避免把设备/资源故障误报成回退成功。
            Err(error) => Err(error),
        }
    }
}
