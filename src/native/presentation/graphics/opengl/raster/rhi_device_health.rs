//! OpenGL ES 薄 RHI 的健康和可控故障边界。

// 引入统一错误分类和结果类型。
use crate::core::{Errc, Error, Result};
// 引入 OpenGL RHI 资源设备状态。
use super::OpenGlRhiDevice;

// 为 OpenGL ES RHI 提供 owner-thread 健康维护和 test-harness 注入。
impl OpenGlRhiDevice {
    // 执行一次不提交命令的设备健康维护。
    pub(crate) fn maintain(&mut self) -> Result<()> {
        // test-harness 故障沿用真实恢复层消费的 typed device-lost 分类。
        #[cfg(feature = "test-harness")]
        if std::mem::take(&mut self.device_lost_for_test) {
            // 返回设备丢失，禁止当前帧继续进入 OpenGL submit。
            return Err(Error::new(
                Errc::GraphicsDeviceLost,
                "OpenGL RHI test device lost before present",
            ));
        }
        // 当前 OpenGL adapter 没有额外的破坏性维护工作。
        Ok(())
    }

    // 安排下一次 OpenGL RHI 设备维护返回 device lost。
    #[cfg(feature = "test-harness")]
    pub(crate) fn arm_device_lost_for_test(&mut self) {
        // 只保存一次性故障标记，不直接破坏 GL 资源。
        self.device_lost_for_test = true;
    }

    // 安排下一次 surface acquire/present 返回 surface lost。
    #[cfg(feature = "test-harness")]
    pub(crate) fn arm_surface_lost_for_test(&mut self) {
        // 让共享 surface host 在真实 adapter 边界消费故障。
        self.surface_lost_for_test = true;
    }

    // 消费一次已经安排的 surface lost。
    #[cfg(feature = "test-harness")]
    pub(crate) fn take_surface_lost_for_test(&mut self) -> bool {
        // 取出标记并清零，避免同一故障污染后续恢复帧。
        std::mem::take(&mut self.surface_lost_for_test)
    }
}
