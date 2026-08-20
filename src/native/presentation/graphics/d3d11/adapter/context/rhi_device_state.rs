//! D3D11 薄 RHI 的 DrawPacket 动态栅格状态编码。

#![allow(dead_code)]

// 复用父模块的 D3D11 context、RHI 状态和错误辅助。
use super::*;

// 为 D3D11 context 提供当前 Draw 原子栅格状态的 inherent helper。
impl D3d11Context {
    // 一次编码当前 DrawPacket 独占的 viewport 与 scissor。
    pub(super) fn rhi_apply_draw_raster(&mut self, raster: DrawRasterState) -> Result<()> {
        // 由共享状态机统一验证 pass、目标和完整栅格关系。
        self.rhi_device.pass.validate_draw_raster(raster)?;
        // 只读投影当前 Draw 的 viewport。
        let viewport = raster.viewport();
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
        // 只读投影当前 Draw 明确选择的裁剪状态。
        let scissor = raster.scissor();
        // 读取共享状态机冻结的物理目标范围。
        let extent = self.rhi_device.pass.extent()?;
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
        // 返回完整栅格状态编码成功。
        Ok(())
    }
}
