//! D3D11 薄 RHI 的设备健康维护边界。

// 引入统一结果类型，保留 GraphicsDevice::maintain 的错误语义。
use crate::core::Result;
// 引入父模块的 DXGI 分类函数和 D3D11 context 状态。
use super::{D3d11Context, map_dxgi_device_removed_reason};
// 引入可控注入使用的标准设备移除 HRESULT。
#[cfg(feature = "test-harness")]
use ::windows::Win32::Graphics::Dxgi::DXGI_ERROR_DEVICE_REMOVED;

// 把设备移除查询保持在 owner thread 的 context 内。
impl D3d11Context {
    // 查询 D3D11 设备是否已经移除或重置。
    pub(super) fn maintain_rhi_device(&mut self) -> Result<()> {
        // checked shutdown 后不得继续执行 owner-thread device 维护。
        self.ensure_active()?;
        // 测试注入复用同一 HRESULT 分类，真实 present 仍会经过本入口。
        #[cfg(feature = "test-harness")]
        if std::mem::take(&mut self.rhi_device_lost_for_test) {
            return map_dxgi_device_removed_reason(DXGI_ERROR_DEVICE_REMOVED);
        }
        // GetDeviceRemovedReason 不提交新命令，只读取当前设备健康状态。
        // SAFETY: device 是当前 context 持有的存活 COM 接口，调用不接收裸指针且在 owner thread 执行。
        let reason = unsafe { self.device.GetDeviceRemovedReason() };
        // 健康设备直接通过维护边界，不生成额外错误对象。
        let Err(error) = reason else {
            return Ok(());
        };
        // 复用 DXGI HRESULT 分类，向恢复层保留 GraphicsDeviceLost 语义。
        map_dxgi_device_removed_reason(error.code())
    }

    // 安排下一次最终 present 前的 D3D11 设备丢失预检。
    #[cfg(feature = "test-harness")]
    pub(super) fn arm_rhi_device_lost_for_test(&mut self) {
        self.rhi_device_lost_for_test = true;
    }
}
