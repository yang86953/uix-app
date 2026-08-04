//! D3D11 薄 RHI 的 viewport 与 scissor 状态编码。

#![allow(dead_code)]

// 复用父模块的 D3D11 context、RHI 状态和错误辅助。
use super::*;

// 为 D3D11 context 提供 pass 状态设置的 inherent helper。
impl D3d11Context {
    // 设置当前 pass viewport。
    pub(super) fn rhi_set_viewport(&mut self, viewport: RhiViewport) -> Result<()> {
        // 拒绝直接调用传入的非有限 viewport。
        if !viewport.is_valid() {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("D3d11 RHI viewport is invalid"));
        }
        // SAFETY: viewport 是本函数验证过的值，context 属于 owner thread。
        unsafe {
            self.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: viewport.width,
                Height: viewport.height,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
        }
        // 返回设置成功。
        Ok(())
    }

    // 设置当前 pass scissor。
    pub(super) fn rhi_set_scissor(&mut self, scissor: Option<RhiScissor>) -> Result<()> {
        // 仅对显式 scissor 做输入校验。
        if let Some(scissor) = scissor {
            // 拒绝负坐标和非正尺寸。
            if !scissor.is_valid() {
                // 返回稳定的参数错误。
                return Err(rhi_invalid("D3d11 RHI scissor is invalid"));
            }
            // SAFETY: scissor 是本函数验证过的值，context 属于 owner thread。
            unsafe {
                self.context.RSSetScissorRects(Some(&[RECT {
                    left: scissor.x,
                    top: scissor.y,
                    right: scissor.x.saturating_add(scissor.width),
                    bottom: scissor.y.saturating_add(scissor.height),
                }]));
            }
        } else {
            // 使用当前 target 的完整 extent 清除历史 scissor。
            let Some(extent) = self.rhi_device.active_extent else {
                // 没有 pass 时不能猜测 viewport。
                return Err(Error::new(
                    Errc::InvalidState,
                    "D3d11 RHI scissor without active pass",
                ));
            };
            // SAFETY: extent 来自已绑定的 surface 或 RHI texture，context 属于 owner thread。
            unsafe {
                self.context.RSSetScissorRects(Some(&[RECT {
                    left: 0,
                    top: 0,
                    right: extent.width as i32,
                    bottom: extent.height as i32,
                }]));
            }
        }
        // 返回设置成功。
        Ok(())
    }
}
