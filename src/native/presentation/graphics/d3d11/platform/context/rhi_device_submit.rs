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
        if !self.rhi_device.pass_open {
            // 返回稳定的状态错误。
            return Err(Error::new(
                Errc::InvalidState,
                "D3d11 RHI render pass is not open",
            ));
        }
        // 释放 pass 级状态，不销毁底层资源。
        self.rhi_device.pass_open = false;
        // 清除当前目标引用。
        self.rhi_device.active_target = None;
        // 清除当前目标句柄。
        self.rhi_device.active_target_handle = None;
        // 清除当前 extent。
        self.rhi_device.active_extent = None;
        // 清除 pass 内资源绑定，下一 pass 必须显式重新绑定。
        self.rhi_device.bound_texture = None;
        self.rhi_device.bound_sampler = None;
        // 返回成功。
        Ok(())
    }

    pub(super) fn submit_impl(&mut self) -> Result<SubmissionHandle> {
        // 未结束的 pass 不能提交，避免隐含结束语义。
        if self.rhi_device.pass_open {
            // 返回稳定的状态错误。
            return Err(Error::new(
                Errc::InvalidState,
                "D3d11 RHI submit with open render pass",
            ));
        }
        // 取出本次提交序号。
        let serial = self.rhi_device.next_submission;
        // 推进提交序号并防止回绕到零句柄。
        self.rhi_device.next_submission = serial.saturating_add(1).max(1);
        // D3D11 immediate context 的命令顺序已经由 owner thread 建立。
        Ok(SubmissionHandle::from_raw(serial))
    }
}
