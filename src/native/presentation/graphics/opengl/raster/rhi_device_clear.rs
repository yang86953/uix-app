//! OpenGL ES 薄 RHI 的局部颜色清理实现。

// 复用父 RHI device 的状态、错误辅助和 OpenGL 类型。
use super::*;

// 把共享颜色清理输出契约逐项翻译为 OpenGL ES 状态。
//
// # Safety
// 调用者必须保证当前 owner thread 的 GL context current。
pub(super) unsafe fn apply_color_clear_contract(
    // 借用当前 owner-thread OpenGL context。
    gl: &glow::Context,
    // 接收两个 Adapter 共用的清理输出状态。
    contract: RhiColorClearContract,
) {
    // SAFETY：调用者保证当前 owner thread 的 GL context current。
    unsafe {
        // 穷尽映射共享颜色写掩码集合。
        match contract.write_mask {
            // 清理必须覆盖红、绿、蓝与 alpha 全部通道。
            PipelineColorWriteMask::All => gl.color_mask(true, true, true, true),
        }
        // 穷尽映射共享清理抖动集合。
        match contract.dither {
            // 禁止 OpenGL ES 默认 dither 改变八位目标最低位。
            PipelineDitherState::Disabled => gl.disable(glow::DITHER),
        }
    }
}

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
        // 由共享状态机统一验证 pass、预乘颜色和左上原点区域。
        self.pass.validate_clear(color, scissor)?;
        // 保存调用方的 scissor，保证局部清理不破坏后续 draw 状态。
        let previous = self.pass.scissor();
        // 暂时把原生 scissor 切换到待清理区域。
        self.set_scissor(gl, Some(scissor))?;
        // SAFETY：共享清理契约只包含可由当前 GL context 机械编码的封闭状态。
        unsafe { apply_color_clear_contract(gl, UIX_COLOR_CLEAR_CONTRACT) };
        // 发出透明或指定 premultiplied-alpha 颜色的矩形清理。
        // SAFETY：调用者保证当前线程绑定有效 GL 上下文（与同设备其他 GL 调用一致）；
        // 颜色分量已通过共享预乘值校验；scissor 由上方 set_scissor 成功切换；
        // 共享 pass 状态保证 framebuffer 已绑定，清理只影响该 framebuffer 的 color buffer。
        unsafe {
            let [red, green, blue, alpha] = color.components();
            // 将共享预乘通道逐项传给 OpenGL，不执行 Adapter 私有转换。
            gl.clear_color(red, green, blue, alpha);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }
        // 恢复调用方原先的 scissor 状态。
        self.set_scissor(gl, previous)?;
        // 返回局部清理成功。
        Ok(())
    }
}
