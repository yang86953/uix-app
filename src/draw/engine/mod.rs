//! 帧渲染结果 + CPU 子模块。
//!
//! 内部渲染引擎 trait 在 `crate::draw::traits::GraphicsEngine`。

use crate::core::{Errc, Error};
use crate::draw::backend::DamageRegion;

pub mod bootstrap;
pub mod cpu;
pub mod factory;
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
    /// The recording target is ready. This is legal only as a `begin_frame`
    /// result; it never means that a swapchain or external presenter has
    /// consumed the frame.
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
    Other(Error),
}

impl GraphicsFailure {
    pub fn from_error(error: Error) -> Self {
        match error.code() {
            Errc::GraphicsSurfaceLost => Self::SurfaceLost(error),
            Errc::GraphicsDeviceLost => Self::DeviceLost(error),
            Errc::GraphicsOutOfMemory | Errc::InsufficientResources => Self::OutOfMemory(error),
            _ => Self::Other(error),
        }
    }

    pub fn error(&self) -> &Error {
        match self {
            Self::SurfaceLost(error)
            | Self::DeviceLost(error)
            | Self::OutOfMemory(error)
            | Self::Other(error) => error,
        }
    }
}
