//! `test-harness` 图形故障信号。
//!
//! 信号只在显式 feature 下编译；应用只能为目标窗口安排一次下一帧
//! `GraphicsDeviceLost`，真正的失败与恢复仍由窗口现有图形 FSM 执行。

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
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
    surface_lost_pending: AtomicBool,
}

impl GraphicsFaultSignal {
    pub(crate) fn attach_recovery_driver(&self) {
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

    // 安排下一次帧边界把 surface-lost 注入送入真实 adapter。
    pub(crate) fn arm_surface_lost(&self) -> Result<()> {
        // 没有恢复包装器时，不能把故障信号送入不存在的 owner thread。
        if !self.inner.attached.load(Ordering::Acquire) {
            return Err(Error::new(
                Errc::NotImplemented,
                "graphics fault injection requires an active recoverable GPU engine",
            ));
        }
        // 禁止同一时刻重复安排 surface 故障，保持一次注入一次恢复。
        self.inner
            .surface_lost_pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                Error::new(
                    Errc::InvalidState,
                    "a graphics surface-lost injection is already pending",
                )
            })?;
        Ok(())
    }

    // 读取并消费已经安排的 surface-lost 信号。
    pub(crate) fn take_surface_lost(&self) -> bool {
        self.inner
            .surface_lost_pending
            .swap(false, Ordering::AcqRel)
    }
}
