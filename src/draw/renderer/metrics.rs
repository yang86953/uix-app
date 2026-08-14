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
    /// 已执行布局阶段的累计次数。
    pub layout_calls: u64,
    /// 已执行绘制阶段的累计次数。
    pub paint_calls: u64,
    /// 已成功提交呈现的累计次数。
    pub present_calls: u64,
    /// 未执行呈现的空闲帧累计次数。
    pub idle_frames: u64,
    /// 最近一次帧决策对应的失效来源。
    pub last_invalidation: InvalidationSource,
}

impl RenderMetrics {
    /// 清空所有计数并恢复为空闲初始状态。
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// 记录一次布局阶段执行。
    pub fn record_layout(&mut self) {
        self.layout_calls += 1;
    }

    /// 记录一次绘制阶段执行。
    pub fn record_paint(&mut self) {
        self.paint_calls += 1;
    }

    /// 记录一次由指定失效来源触发的成功呈现。
    pub fn record_present(&mut self, source: InvalidationSource) {
        self.present_calls += 1;
        self.last_invalidation = source;
    }

    /// 记录一次没有待处理失效的空闲帧。
    pub fn record_idle(&mut self) {
        self.idle_frames += 1;
        self.last_invalidation = InvalidationSource::None;
    }

    /// 记录一次保留指定失效来源但未呈现的空闲帧。
    pub fn record_idle_with_source(&mut self, source: InvalidationSource) {
        self.idle_frames += 1;
        self.last_invalidation = source;
    }
}
