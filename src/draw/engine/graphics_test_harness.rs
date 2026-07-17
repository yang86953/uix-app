//! `test-harness` 图形故障信号。
//!
//! 信号只在显式 feature 下编译；应用只能为目标窗口安排一次下一帧
//! `GraphicsDeviceLost`，真正的失败与恢复仍由窗口现有图形 FSM 执行。

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::core::{Errc, Error, Result};

#[derive(Clone, Default)]
pub(crate) struct GraphicsFaultSignal {
    inner: Arc<GraphicsFaultSignalInner>,
}

#[derive(Default)]
struct GraphicsFaultSignalInner {
    attached: AtomicBool,
    device_lost_pending: AtomicBool,
}

impl GraphicsFaultSignal {
    pub(crate) fn attach_recovering_engine(&self) {
        self.inner.attached.store(true, Ordering::Release);
    }

    pub(crate) fn arm_device_lost(&self) -> Result<()> {
        if !self.inner.attached.load(Ordering::Acquire) {
            return Err(Error::new(
                Errc::NotImplemented,
                "graphics fault injection requires an active recoverable GPU engine",
            ));
        }
        self.inner
            .device_lost_pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                Error::new(
                    Errc::InvalidState,
                    "a graphics device-lost injection is already pending",
                )
            })?;
        Ok(())
    }

    pub(crate) fn take_device_lost(&self) -> bool {
        self.inner.device_lost_pending.swap(false, Ordering::AcqRel)
    }
}
