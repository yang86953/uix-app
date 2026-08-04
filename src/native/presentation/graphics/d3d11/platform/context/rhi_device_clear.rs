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
        // 局部清理只能发生在已经绑定目标的 render pass 内。
        if !self.rhi_device.pass_open {
            // 返回状态错误，避免对未知目标发出 ClearView。
            return Err(Error::new(
                Errc::InvalidState,
                "D3d11 RHI clear rect without active pass",
            ));
        }
        // 统一验证颜色和矩形的基本契约。
        if !color.is_finite() || !scissor.is_valid() {
            // 返回参数错误，避免负坐标或非有限颜色进入原生 API。
            return Err(rhi_invalid("D3d11 RHI clear rect is invalid"));
        }
        // 读取当前 pass 的物理目标尺寸。
        let Some(extent) = self.rhi_device.active_extent else {
            // pass 状态不完整时拒绝继续清理。
            return Err(Error::new(
                Errc::InvalidState,
                "D3d11 RHI clear rect has no active extent",
            ));
        };
        // 检查清理矩形的右边界不会整数回绕。
        let right = scissor
            .x
            .checked_add(scissor.width)
            .ok_or_else(|| rhi_invalid("D3d11 RHI clear rect right edge overflow"))?;
        // 检查清理矩形的底边界不会整数回绕。
        let bottom = scissor
            .y
            .checked_add(scissor.height)
            .ok_or_else(|| rhi_invalid("D3d11 RHI clear rect bottom edge overflow"))?;
        // 矩形必须完全落在当前 render target 内。
        if right as u32 > extent.width || bottom as u32 > extent.height {
            // 返回稳定的越界参数错误。
            return Err(rhi_invalid("D3d11 RHI clear rect is outside target"));
        }
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
            context.ClearView(target, &color.0, Some(&[rect]));
        }
        // 返回局部清理成功。
        Ok(())
    }
}
