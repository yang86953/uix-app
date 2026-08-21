//! 帧渲染结果与类型化失败。

use crate::core::{Errc, Error};
use crate::draw::backend::DamageRegion;

/// 帧渲染结果。
#[derive(Debug, Clone, PartialEq)]
pub enum RenderOutcome {
    /// 零帧开销（无需呈现）。
    Idle,
    /// 已渲染，需呈现（多矩形损伤）。
    /// 录制目标已就绪；所带区域是本轮实际清除/裁剪区域，调用方必须至少重绘该区域。
    /// 仅可由 `begin_frame` 返回，不表示 swapchain 或外部 presenter 已消费此帧。
    FrameReady(DamageRegion),
    /// 录制已经完成，但外部平台 presenter 仍拥有唯一最终提交。
    ///
    /// 仅可作为 [`crate::draw::PresentationMode::ExternalPresenter`] 的
    /// `end_frame` 结果。
    PresentPending(DamageRegion),
    /// 唯一最终提交已经完成。
    ///
    /// 仅可作为 backend presenter 消费帧后的 `end_frame` 结果，或由窗口会话完成外部
    /// present 后建立。
    Present(DamageRegion),
    /// 类型化图形生命周期失败；帧尚未提交，调用方必须为恢复状态机保留失效区域。
    Failed(GraphicsFailure),
}

/// 在帧边界决定允许恢复路径的图形失败分类。
///
/// 具体平台错误始终附带在分类中供诊断使用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphicsFailure {
    /// 当前呈现 surface 已失效，需要重建 surface 相关资源。
    SurfaceLost(Error),
    /// platform 已完成受控重建；当前脏帧只需在新代际重试。
    SurfaceChanged(Error),
    /// 当前图形设备已失效，需要重建设备及其资源。
    DeviceLost(Error),
    /// 图形内存或等价资源不足，不能继续提交当前帧。
    OutOfMemory(Error),
    /// surface 当前被遮挡或暂时不可呈现。
    Occluded(Error),
    /// 不属于已知图形恢复分类的失败。
    Other(Error),
}

impl GraphicsFailure {
    /// 根据稳定错误码把具体错误归入图形恢复分类。
    pub fn from_error(error: Error) -> Self {
        match error.code() {
            Errc::GraphicsSurfaceLost => Self::SurfaceLost(error),
            Errc::GraphicsSurfaceChanged => Self::SurfaceChanged(error),
            Errc::GraphicsDeviceLost => Self::DeviceLost(error),
            Errc::GraphicsOutOfMemory | Errc::InsufficientResources => Self::OutOfMemory(error),
            Errc::GraphicsOccluded => Self::Occluded(error),
            _ => Self::Other(error),
        }
    }

    /// 返回分类携带的原始错误。
    pub fn error(&self) -> &Error {
        match self {
            Self::SurfaceLost(error)
            | Self::SurfaceChanged(error)
            | Self::DeviceLost(error)
            | Self::OutOfMemory(error)
            | Self::Occluded(error)
            | Self::Other(error) => error,
        }
    }
}
