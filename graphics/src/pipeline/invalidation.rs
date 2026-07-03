//! 渲染失效队列 — 所有渲染触发的统一入口（Phase 2）。

use uix_platform::Rect;

use crate::types::DirtyRegion;

/// 节点标识（与 UI 层 WidgetId 对齐，graphics 不依赖 uix-ui）。
pub type NodeId = usize;

/// 滚动增量（Composite invalidation 附带）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollDelta {
    pub dx: f32,
    pub dy: f32,
}

/// 渲染失效信号。
#[derive(Debug, Clone, PartialEq)]
pub enum Invalidation {
    /// 布局可能变化（尺寸、位置、子树结构）。
    Layout(NodeId),
    /// 视觉变化，布局不变。
    Paint {
        id: NodeId,
        /// `None` 表示整个 widget 的 dirty_rect。
        rect: Option<Rect>,
    },
    /// 合成层操作（如 scroll memmove 后的 exposed strip）。
    Composite {
        rect: Rect,
        scroll: Option<ScrollDelta>,
    },
}

/// 失效队列：合并重复 Paint、判定 0 帧。
#[derive(Debug, Clone, Default)]
pub struct InvalidationQueue {
    items: Vec<Invalidation>,
}

impl InvalidationQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// 上报失效。
    pub fn push(&mut self, inv: Invalidation) {
        self.items.push(inv);
    }

    /// 队列是否无任何失效（0 帧判定入口）。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 是否含 Layout 失效。
    pub fn has_layout(&self) -> bool {
        self.items
            .iter()
            .any(|i| matches!(i, Invalidation::Layout(_)))
    }

    /// 是否含 Paint 或 Composite 失效（需要绘制）。
    pub fn has_paint_or_composite(&self) -> bool {
        self.items.iter().any(|i| {
            matches!(
                i,
                Invalidation::Paint { .. } | Invalidation::Composite { .. }
            )
        })
    }

    /// 将 Paint / Composite 矩形合并为 `DirtyRegion`（供渲染裁剪）。
    pub fn dirty_region(&self) -> DirtyRegion {
        let mut region = DirtyRegion::empty();
        for item in &self.items {
            match item {
                Invalidation::Paint { rect: Some(r), .. } => region.add_rect(*r),
                Invalidation::Composite { rect, .. } => region.add_rect(*rect),
                Invalidation::Layout(_) => {}
                Invalidation::Paint { rect: None, .. } => {
                    return DirtyRegion::full();
                }
            }
        }
        region
    }

    /// 取出首个 Composite 滚动参数（event_loop scroll memmove 用）。
    pub fn scroll_composite(&self) -> Option<(Rect, ScrollDelta)> {
        self.items.iter().find_map(|i| {
            if let Invalidation::Composite { rect, scroll } = i {
                scroll.map(|s| (*rect, s))
            } else {
                None
            }
        })
    }

    /// 清空队列（帧末或 reset 时调用）。
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_queue_is_idle() {
        let q = InvalidationQueue::new();
        assert!(q.is_empty());
        assert!(!q.has_layout());
        assert!(!q.has_paint_or_composite());
    }

    #[test]
    fn paint_merges_to_dirty_region() {
        let mut q = InvalidationQueue::new();
        q.push(Invalidation::Paint {
            id: 1,
            rect: Some(Rect::new(0.0, 0.0, 10.0, 10.0)),
        });
        q.push(Invalidation::Paint {
            id: 2,
            rect: Some(Rect::new(20.0, 0.0, 10.0, 10.0)),
        });
        let region = q.dirty_region();
        assert!(!region.is_empty());
        assert_eq!(region.rects().len(), 2);
    }

    #[test]
    fn layout_only_does_not_imply_paint() {
        let mut q = InvalidationQueue::new();
        q.push(Invalidation::Layout(0));
        assert!(!q.is_empty());
        assert!(q.has_layout());
        assert!(!q.has_paint_or_composite());
        assert!(q.dirty_region().is_empty());
    }

    #[test]
    fn paint_none_expands_full_frame() {
        let mut q = InvalidationQueue::new();
        q.push(Invalidation::Paint { id: 0, rect: None });
        assert!(q.dirty_region().full_frame);
    }

    #[test]
    fn composite_scroll_extract() {
        let mut q = InvalidationQueue::new();
        let frame = Rect::new(0.0, 0.0, 100.0, 200.0);
        q.push(Invalidation::Composite {
            rect: frame,
            scroll: Some(ScrollDelta { dx: 0.0, dy: -10.0 }),
        });
        let (r, s) = q.scroll_composite().expect("scroll");
        assert_eq!(r, frame);
        assert!((s.dy + 10.0).abs() < 1e-6);
    }
}
