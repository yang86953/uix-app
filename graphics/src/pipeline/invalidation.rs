//! 渲染失效队列 — 所有渲染触发的统一入口（Phase 2 / Phase 6）。

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use uix_platform::Rect;

use crate::types::DirtyRegion;

/// 节点标识（与 UI 层 WidgetId 对齐，graphics 不依赖 uix-ui）。
pub type NodeId = usize;

/// 共享失效队列句柄（State 绑定 paint 失效时使用，每棵树一个实例）。
pub type InvalidationQueueHandle = Arc<Mutex<InvalidationQueue>>;

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

    /// 创建共享句柄（WidgetTree 持有）。
    pub fn shared() -> InvalidationQueueHandle {
        Arc::new(Mutex::new(Self::new()))
    }

    /// 上报失效；同一节点的 Paint 矩形会合并。
    pub fn push(&mut self, inv: Invalidation) {
        if let Invalidation::Paint { id, rect } = &inv {
            if let Some(existing) = self
                .items
                .iter_mut()
                .find(|i| matches!(i, Invalidation::Paint { id: eid, .. } if *eid == *id))
            {
                if let Invalidation::Paint {
                    rect: existing_rect, ..
                } = existing
                {
                    *existing_rect = merge_paint_rect(*existing_rect, *rect);
                }
                return;
            }
        }
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

    /// 是否含全帧 Paint（`rect: None`）。
    pub fn needs_full_frame(&self) -> bool {
        self.items
            .iter()
            .any(|i| matches!(i, Invalidation::Paint { rect: None, .. }))
    }

    /// 含 Layout 失效的节点 id 集合。
    pub fn layout_roots(&self) -> HashSet<NodeId> {
        self.items
            .iter()
            .filter_map(|i| match i {
                Invalidation::Layout(id) => Some(*id),
                _ => None,
            })
            .collect()
    }

    /// 节点是否有 Paint 失效。
    pub fn node_needs_paint(&self, id: NodeId) -> bool {
        if self.needs_full_frame() {
            return true;
        }
        self.items.iter().any(|i| match i {
            Invalidation::Paint { id: pid, .. } => *pid == id,
            _ => false,
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

/// 通过共享句柄推送 Paint 失效（State 绑定用）。
pub fn invalidate_paint_handle(
    handle: &InvalidationQueueHandle,
    id: NodeId,
    rect: Option<Rect>,
) {
    if let Ok(mut q) = handle.lock() {
        q.push(Invalidation::Paint { id, rect });
    }
}

fn merge_paint_rect(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (None, _) | (_, None) => None,
        (Some(ra), Some(rb)) => Some(union_rect(ra, rb)),
    }
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    let x1 = a.x.min(b.x);
    let y1 = a.y.min(b.y);
    let x2 = (a.x + a.w).max(b.x + b.w);
    let y2 = (a.y + a.h).max(b.y + b.h);
    Rect::new(x1, y1, x2 - x1, y2 - y1)
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
        assert!(q.needs_full_frame());
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

    #[test]
    fn merge_same_node_paint_rects() {
        let mut q = InvalidationQueue::new();
        q.push(Invalidation::Paint {
            id: 3,
            rect: Some(Rect::new(0.0, 0.0, 10.0, 10.0)),
        });
        q.push(Invalidation::Paint {
            id: 3,
            rect: Some(Rect::new(5.0, 5.0, 10.0, 10.0)),
        });
        assert_eq!(q.items.len(), 1);
        let region = q.dirty_region();
        assert_eq!(region.rects().len(), 1);
        let r = region.rects()[0];
        assert!((r.x - 0.0).abs() < 1e-6);
        assert!((r.w - 15.0).abs() < 1e-6);
    }

    #[test]
    fn node_needs_paint_targeted() {
        let mut q = InvalidationQueue::new();
        q.push(Invalidation::Paint {
            id: 7,
            rect: Some(Rect::new(0.0, 0.0, 5.0, 5.0)),
        });
        assert!(q.node_needs_paint(7));
        assert!(!q.node_needs_paint(8));
    }
}
