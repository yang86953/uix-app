//! OpenGL ES 薄 RHI 的局部颜色清理实现。

// 复用父 RHI device 的状态、错误辅助和 OpenGL 类型。
use super::*;

// 为 OpenGL ES RHI 提供保持 scissor 状态的局部清理原语。
impl OpenGlRhiDevice {
    // 在当前 pass 的目标上清理一个左上原点物理矩形。
    // 供同一 raster owner 的薄 RHI bridge 调用局部清理。
    pub(crate) fn clear_rect(
        &mut self,
        gl: &glow::Context,
        color: RhiColor,
        scissor: RhiScissor,
    ) -> Result<()> {
        // 局部清理只能发生在已经绑定目标的 render pass 内。
        if !self.pass_open {
            // 返回状态错误，避免对未知 framebuffer 发出清理。
            return Err(rhi_invalid("OpenGL RHI clear rect without active pass"));
        }
        // 统一验证颜色和矩形的基本契约。
        if !color.is_finite() || !scissor.is_valid() {
            // 返回参数错误，避免负坐标或非有限颜色进入原生 API。
            return Err(rhi_invalid("OpenGL RHI clear rect is invalid"));
        }
        // 当前 pass 必须有可用于边界检查的物理 extent。
        let extent = self
            .active_extent
            .ok_or_else(|| rhi_invalid("OpenGL RHI clear rect has no active extent"))?;
        // 检查清理矩形的右边界不会整数回绕。
        let right = scissor
            .x
            .checked_add(scissor.width)
            .ok_or_else(|| rhi_invalid("OpenGL RHI clear rect right edge overflow"))?;
        // 检查清理矩形的底边界不会整数回绕。
        let bottom = scissor
            .y
            .checked_add(scissor.height)
            .ok_or_else(|| rhi_invalid("OpenGL RHI clear rect bottom edge overflow"))?;
        // 矩形必须完全落在当前 render target 内。
        if right as u32 > extent.width || bottom as u32 > extent.height {
            // 返回稳定的越界参数错误。
            return Err(rhi_invalid("OpenGL RHI clear rect is outside target"));
        }
        // 保存调用方的 scissor，保证局部清理不破坏后续 draw 状态。
        let previous = self.scissor;
        // 暂时把原生 scissor 切换到待清理区域。
        self.set_scissor(gl, Some(scissor))?;
        // 发出透明或指定 premultiplied-alpha 颜色的矩形清理。
        unsafe {
            gl.clear_color(color.0[0], color.0[1], color.0[2], color.0[3]);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }
        // 恢复调用方原先的 scissor 状态。
        self.set_scissor(gl, previous)?;
        // 返回局部清理成功。
        Ok(())
    }
}
