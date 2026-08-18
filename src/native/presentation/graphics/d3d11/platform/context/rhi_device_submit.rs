//! D3D11 薄 RHI 的 pass 收尾与提交实现。
//!
//! 结束显式 render pass 与提交 immediate context 命令序列；二者都拒绝在
//! 错误状态下执行，保持 FramePlan 的显式边界语义。

// 引入统一错误和结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入薄 RHI 的提交句柄类型。
use crate::native::present::rhi::SubmissionHandle;

// 引入 context 父模块的 D3D11 状态。
use super::D3d11Context;

// 为 D3D11 context 提供 pass 收尾与提交的固有实现段。
impl D3d11Context {
    pub(super) fn end_render_pass_impl(&mut self) -> Result<()> {
        // 拒绝没有开始 pass 的结束调用。
        self.rhi_device.pass.require_open()?;
        // 显式解除可能残留的采样输入和输出目标，禁止依赖 D3D11 隐式冲突处理。
        // SAFETY: 三个调用只清除当前 owner-thread immediate context 的绑定槽，不借用或销毁资源。
        unsafe {
            // 清除零号 shader resource view。
            self.context.PSSetShaderResources(0, Some(&[None]));
            // 清除零号 sampler state。
            self.context.PSSetSamplers(0, Some(&[None]));
            // 清除当前 render target view。
            self.context.OMSetRenderTargets(None, None);
        }
        // 清除 Adapter 私有的原生目标引用。
        self.rhi_device.active_target = None;
        // 由共享状态机原子清除目标、几何与采样绑定事实。
        self.rhi_device.pass.end()?;
        // 返回成功。
        Ok(())
    }

    pub(super) fn submit_impl(&mut self) -> Result<SubmissionHandle> {
        // 未结束的 pass 不能提交，避免隐含结束语义。
        self.rhi_device.pass.require_closed()?;
        // D3D11 immediate context 的命令顺序已经由 owner thread 建立，再由共享状态机签发身份。
        self.rhi_device.submission_sequence.issue()
    }

    // 校验 Surface present 使用的是同一组合 context 最近一次成功 submit。
    pub(in super::super) fn validate_submission_impl(
        // 只读借用 context，校验不得改变 Device 或 Surface 状态。
        &self,
        // 接收 FramePlan 从 submit 原样传递的类型化身份。
        submission: SubmissionHandle,
    ) -> Result<()> {
        // D3D11 必须执行与 OpenGL 相同的共享最新值规则。
        if !self.rhi_device.submission_sequence.is_latest(submission) {
            // 在进入 DXGI Present 前返回稳定的参数错误。
            return Err(Error::new(
                // 外部或迟到身份属于调用契约错误，而不是设备丢失。
                Errc::InvalidArgument,
                // 保留 Adapter 名称便于定位原生边界。
                "D3d11 RHI present submission is stale",
            ));
        }
        // 返回统一成功结果。
        Ok(())
    }
}
