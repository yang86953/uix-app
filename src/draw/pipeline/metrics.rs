//! 渲染度量 — Phase 0 回归基线计数器。

/// 触发本帧渲染的失效来源（Debug overlay 显示用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InvalidationSource {
    /// 无渲染（0 帧）。
    #[default]
    None,
    /// 首帧必须绘制。
    FirstFrame,
    /// WidgetTree 脏区域非空。
    DirtyRegion,
    /// 动画 tick 产生脏区域（非空时触发 present）。
    AnimationPolling,
    /// 布局相关 OS 事件（resize 等）。
    LayoutEvent,
}

impl InvalidationSource {
    /// Debug HUD 短标签。
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "idle",
            Self::FirstFrame => "first_frame",
            Self::DirtyRegion => "dirty_region",
            Self::AnimationPolling => "animation",
            Self::LayoutEvent => "layout_event",
        }
    }
}

/// 帧级渲染统计（layout / paint / present / idle）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderMetrics {
    pub layout_calls: u64,
    pub paint_calls: u64,
    pub present_calls: u64,
    pub idle_frames: u64,
    pub last_invalidation: InvalidationSource,
}

impl RenderMetrics {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn record_layout(&mut self) {
        self.layout_calls += 1;
    }

    pub fn record_paint(&mut self) {
        self.paint_calls += 1;
    }

    pub fn record_present(&mut self, source: InvalidationSource) {
        self.present_calls += 1;
        self.last_invalidation = source;
    }

    pub fn record_idle(&mut self) {
        self.idle_frames += 1;
        self.last_invalidation = InvalidationSource::None;
    }

    pub fn record_idle_with_source(&mut self, source: InvalidationSource) {
        self.idle_frames += 1;
        self.last_invalidation = source;
    }
}

