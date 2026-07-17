//! 帧渲染结果 + CPU 子模块。
//!
//! 内部渲染引擎 trait 在 `crate::draw::traits::GraphicsEngine`。

use crate::core::{Errc, Error};
use crate::draw::backend::DamageRegion;

pub mod bootstrap;
pub mod cpu;
pub mod factory;
#[cfg(feature = "test-harness")]
pub(crate) mod graphics_test_harness;
pub mod present_upload;
pub mod recovering;
pub mod recovery;

pub use recovering::{GraphicsEngineRebuilder, RecoveringGraphicsEngine};
pub use recovery::{GraphicsRecovery, RecoveryAction};

/// 帧渲染结果。
#[derive(Debug, Clone, PartialEq)]
pub enum RenderOutcome {
    /// 零帧开销（无需呈现）
    Idle,
    /// 已渲染，需呈现（多矩形损伤）。
    /// 录制目标已就绪；所带区域是本轮实际清除/裁剪区域，调用方必须至少重绘该区域。
    /// 仅可由 `begin_frame` 返回，不表示 swapchain 或外部 presenter 已消费此帧。
    FrameReady(DamageRegion),
    /// Recording is complete but an external platform presenter still owns
    /// the one final submission. This is legal only as an `end_frame` result
    /// for [`crate::draw::traits::PresentationMode::ExternalPresenter`].
    PresentPending(DamageRegion),
    /// The sole final submission completed. This is legal only as an
    /// `end_frame` result after an engine-managed presenter has consumed the
    /// frame, or after the window session has completed an external present.
    Present(DamageRegion),
    /// A typed graphics lifecycle failure.  The frame was not committed and
    /// callers must retain invalidation for the recovery state machine.
    Failed(GraphicsFailure),
}

/// Graphics failures that determine the permitted recovery path at a frame
/// boundary.  The concrete platform error remains attached for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphicsFailure {
    SurfaceLost(Error),
    DeviceLost(Error),
    OutOfMemory(Error),
    Occluded(Error),
    Other(Error),
}

impl GraphicsFailure {
    pub fn from_error(error: Error) -> Self {
        match error.code() {
            Errc::GraphicsSurfaceLost => Self::SurfaceLost(error),
            Errc::GraphicsDeviceLost => Self::DeviceLost(error),
            Errc::GraphicsOutOfMemory | Errc::InsufficientResources => Self::OutOfMemory(error),
            Errc::GraphicsOccluded => Self::Occluded(error),
            _ => Self::Other(error),
        }
    }

    pub fn error(&self) -> &Error {
        match self {
            Self::SurfaceLost(error)
            | Self::DeviceLost(error)
            | Self::OutOfMemory(error)
            | Self::Occluded(error)
            | Self::Other(error) => error,
        }
    }
}
