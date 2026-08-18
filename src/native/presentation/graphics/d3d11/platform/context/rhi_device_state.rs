//! D3D11 薄 RHI 的 viewport 与 scissor 状态编码。

#![allow(dead_code)]

// 复用父模块的 D3D11 context、RHI 状态和错误辅助。
use super::*;

// 为 D3D11 context 提供 pass 状态设置的 inherent helper。
impl D3d11Context {
    // 设置当前 pass viewport。
    pub(super) fn rhi_set_viewport(&mut self, viewport: RhiViewport) -> Result<()> {
        // 由共享状态机验证 pass 顺序和物理目标边界。
        self.rhi_device.pass.validate_viewport(viewport)?;
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
        // 读取共享状态机冻结的物理目标范围。
        let extent = self.rhi_device.pass.extent()?;
        // 先由共享状态机统一验证并记录左上原点区域。
        self.rhi_device.pass.set_scissor(scissor)?;
        // 仅对显式 scissor 编码有限原生矩形。
        if let Some(scissor) = scissor {
            // 读取共享几何 Component 的唯一 checked 矩形投影。
            let (left, top, right, bottom) = scissor
                // D3D11 不得用饱和加法掩盖远端边界溢出。
                .native_rect()
                // 防御未来路径绕过 pass 状态机。
                .ok_or_else(|| rhi_invalid("D3d11 RHI scissor rect is invalid"))?;
            // SAFETY: scissor 是本函数验证过的值，context 属于 owner thread。
            unsafe {
                self.context.RSSetScissorRects(Some(&[RECT {
                    left,
                    top,
                    right,
                    bottom,
                }]));
            }
        } else {
            // 读取共享 extent 已验证的完整原生矩形远端边界。
            let (right, bottom) = extent
                // 禁止 Adapter 自行把 u32 截断为 i32。
                .native_size_i32()
                // 活动 pass 理论上已证明有效，仍保留稳定错误。
                .ok_or_else(|| rhi_invalid("D3d11 RHI target extent is invalid"))?;
            // 使用已经在入口验证的完整 target extent 清除历史 scissor。
            // SAFETY: extent 来自已绑定的 surface 或 RHI texture，context 属于 owner thread。
            unsafe {
                self.context.RSSetScissorRects(Some(&[RECT {
                    left: 0,
                    top: 0,
                    right,
                    bottom,
                }]));
            }
        }
        // 返回设置成功。
        Ok(())
    }
}
