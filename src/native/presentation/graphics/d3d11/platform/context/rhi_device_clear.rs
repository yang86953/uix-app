//! D3D11 薄 RHI 的局部颜色清理实现。

// 复用父 RHI device 的状态、错误辅助和 D3D11 context。
use super::*;
// 引入 COM 接口转换，用于访问 D3D11.1 的 ClearView。
use ::windows::core::Interface;
// 引入支持矩形清理的 D3D11.1 context 接口。
use ::windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext1;

// 为 D3D11 context 提供保持矩形语义的局部清理原语。
impl D3d11Context {
    // 在当前 pass 的目标上清理一个左上原点物理矩形。
    pub(super) fn rhi_clear_rect(&mut self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
        // 由共享状态机统一验证 pass、预乘颜色和左上原点区域。
        self.rhi_device.pass.validate_clear(color, scissor)?;
        // 读取已由共享契约证明可被 D3D11 RECT 精确表达的远端边界。
        let (right, bottom) = scissor
            // Adapter 只消费共享几何值，不重新决定溢出规则。
            .far_edges()
            // 理论上不可达的失败仍转换成稳定的类型化错误。
            .ok_or_else(|| rhi_invalid("D3d11 RHI clear rect edges are invalid"))?;
        // 读取当前 render target view，禁止对空目标执行清理。
        let Some(target) = self.rhi_device.active_target.as_ref() else {
            // 保持 pass 状态和目标句柄的一致性要求。
            return Err(Error::new(
                Errc::InvalidState,
                "D3d11 RHI clear rect has no active target",
            ));
        };
        // D3D11.0 的 immediate context 没有 ClearView，能力不足时安全回退。
        let context = self
            .context
            .cast::<ID3D11DeviceContext1>()
            .map_err(|_| rhi_not_implemented("clear_rect requires ID3D11DeviceContext1"))?;
        // 把 RHI 左上原点矩形直接交给 ClearView，避免改变 raster scissor 状态。
        let rect = RECT {
            left: scissor.x,
            top: scissor.y,
            right,
            bottom,
        };
        // SAFETY: target/context 属于同一 owner-thread D3D11 device，矩形和颜色已验证。
        unsafe {
            context.ClearView(target, &color.components(), Some(&[rect]));
        }
        // 返回局部清理成功。
        Ok(())
    }
}
