//! 帧更新策略 — 决定本帧如何清除和绘制。

use uix_platform::Rect;

/// 帧更新策略。
#[derive(Debug, Clone)]
pub enum UpdateStrategy {
    /// 全屏清除 + 完整重绘。
    /// 适用：窗口首次绘制、分辨率变更。
    FullRedraw,

    /// 精确更新：清除指定区域内旧内容，只重绘这些区域。
    /// 适用：Widget 状态变更、动画单帧。
    DirtyRects(Vec<Rect>),

    /// 增量叠加：不清除，在上一帧内容上叠画新内容。
    /// 适用：光标闪烁、拖拽预览、通知弹出。
    Overlay(Vec<Rect>),
}

impl UpdateStrategy {
    /// 是否为本帧指定了脏区域。
    pub fn rects(&self) -> Option<&[Rect]> {
        match self {
            UpdateStrategy::FullRedraw => None,
            UpdateStrategy::DirtyRects(rects) => Some(rects),
            UpdateStrategy::Overlay(rects) => Some(rects),
        }
    }

    /// 是否需要清除。
    pub fn should_clear(&self) -> bool {
        matches!(self, UpdateStrategy::FullRedraw | UpdateStrategy::DirtyRects(_))
    }
}
