//! 渲染失效队列 — 所有渲染触发的统一入口（Phase 2 / Phase 6）。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::core::Rect;
use crate::draw::scene::NodeId;

use crate::core::DirtyRegion;

/// 共享失效队列句柄（State 绑定 paint 失效时使用，每棵树一个实例）。
pub type InvalidationQueueHandle = Arc<Mutex<InvalidationQueue>>;

/// 滚动增量（Composite invalidation 附带）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollDelta {
    /// 水平方向滚动增量。
    pub dx: f32,
    /// 垂直方向滚动增量。
    pub dy: f32,
}

/// 渲染失效信号。
#[derive(Debug, Clone, PartialEq)]
pub enum Invalidation {
    /// 布局可能变化（尺寸、位置、子树结构）。
    Layout(NodeId),
    /// 视觉变化，布局不变。
    Paint {
        /// 发生视觉变化的场景节点身份。
        id: NodeId,
        /// `None` 表示整个节点的 dirty_rect。
        rect: Option<Rect>,
    },
    /// 合成层操作（如 scroll memmove 后的 exposed strip）。
    Composite {
        /// 需要重新合成的逻辑区域。
        rect: Rect,
        /// 可选的滚动位移，用于复用既有像素并重绘暴露区域。
        scroll: Option<ScrollDelta>,
    },
    /// 合成器拓扑已经变化，必须重建完整目标，但不把普通场景节点报告为绘制失效。
    ///
    /// 根级 overlay 成员变化使用该信号，避免把层级工作与普通树内容变化混为一谈。
    FullComposite,
}

/// 失效队列：合并重复 Layout / Paint、判定 0 帧。
#[derive(Debug, Clone, Default)]
pub struct InvalidationQueue {
    pub(crate) items: Vec<Invalidation>,
    layout_ids: HashSet<NodeId>,
    paint_indices: HashMap<NodeId, usize>,
    revision: u64,
}

impl InvalidationQueue {
    /// 创建修订号为零的空失效队列。
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建共享句柄（WidgetTree 持有）。
    pub fn shared() -> InvalidationQueueHandle {
        Arc::new(Mutex::new(Self::new()))
    }

    /// 上报失效；同一节点的 Layout 去重，Paint 矩形合并。
    pub fn push(&mut self, inv: Invalidation) {
        self.revision = self.revision.wrapping_add(1);
        match &inv {
            Invalidation::Layout(id) => {
                if !self.layout_ids.insert(*id) {
                    return;
                }
            }
            Invalidation::Paint { id, rect } => {
                if let Some(existing) = self
                    .paint_indices
                    .get(id)
                    .and_then(|&index| self.items.get_mut(index))
                {
                    if let Invalidation::Paint {
                        rect: existing_rect,
                        ..
                    } = existing
                    {
                        *existing_rect = merge_paint_rect(*existing_rect, *rect);
                    }
                    return;
                }
                self.paint_indices.insert(*id, self.items.len());
            }
            Invalidation::Composite { .. } | Invalidation::FullComposite => {}
        }
        self.items.push(inv);
    }

    /// 批量上报失效；调用方可在一个锁临界区内完成一轮更新。
    pub fn extend(&mut self, invalidations: impl IntoIterator<Item = Invalidation>) {
        for invalidation in invalidations {
            self.push(invalidation);
        }
    }

    /// 队列是否无任何失效（0 帧判定入口）。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 返回单调回绕的变更标记，用于保留帧绘制或呈现期间新产生的失效。
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// 是否含 Layout 失效。
    pub fn has_layout(&self) -> bool {
        !self.layout_ids.is_empty()
    }

    /// 帧诊断：返回失效条目数及最大 Paint 失效节点身份与矩形。
    pub(crate) fn diag_largest_paint(&self) -> (usize, Option<(NodeId, Rect)>) {
        // 累计失效条目数。
        let mut count = 0;
        // 追踪面积最大的 Paint 失效。
        let mut largest: Option<(NodeId, Rect)> = None;
        // 遍历全部失效条目。
        for item in &self.items {
            count += 1;
            // 只统计带显式矩形的 Paint 失效。
            if let Invalidation::Paint { id, rect: Some(r) } = item {
                // 保留面积更大的矩形。
                if largest
                    .as_ref()
                    .is_none_or(|(_, cur)| r.w * r.h > cur.w * cur.h)
                {
                    largest = Some((*id, *r));
                }
            }
        }
        (count, largest)
    }

    /// 是否含 Paint 或 Composite 失效（需要绘制）。
    pub fn has_paint_or_composite(&self) -> bool {
        self.items.iter().any(|i| {
            matches!(
                i,
                Invalidation::Paint { .. }
                    | Invalidation::Composite { .. }
                    | Invalidation::FullComposite
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
        let mut roots = HashSet::new();
        self.layout_roots_into(&mut roots);
        roots
    }

    /// 将 Layout 根写入调用方复用的集合。
    pub(crate) fn layout_roots_into(&self, roots: &mut HashSet<NodeId>) {
        roots.clear();
        roots.extend(self.layout_ids.iter().copied());
    }

    /// 将当前已有 Paint 节点身份写入调用方复用集合。
    pub(crate) fn paint_ids_into(&self, ids: &mut HashSet<NodeId>) {
        ids.clear();
        ids.extend(self.paint_indices.keys().copied());
    }

    /// 节点是否有 Paint 失效。
    pub fn node_needs_paint(&self, id: NodeId) -> bool {
        if self.needs_full_frame() {
            return true;
        }
        self.paint_indices.contains_key(&id)
    }

    /// 将 Paint / Composite 矩形合并为 `DirtyRegion`（供渲染裁剪）。
    pub fn dirty_region(&self) -> DirtyRegion {
        let mut region = DirtyRegion::empty();
        for item in &self.items {
            match item {
                Invalidation::Paint { rect: Some(r), .. } => region.add_rect(*r),
                Invalidation::Composite { rect, .. } => region.add_rect(*rect),
                Invalidation::FullComposite => return DirtyRegion::full(),
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
        self.revision = self.revision.wrapping_add(1);
        self.items.clear();
        self.layout_ids.clear();
        self.paint_indices.clear();
    }

    /// 仅当采样修订号之后没有生产者推送失效时清空队列。
    ///
    /// 修订号不匹配时同时保留既有与新增工作，供下一帧保守收敛。
    pub fn clear_if_revision(&mut self, revision: u64) -> bool {
        if self.revision != revision {
            return false;
        }
        self.clear();
        true
    }

    /// 仅移除 Layout 项（layout() 收敛后消费；保留 Paint / Composite 供 present）。
    pub fn clear_layout(&mut self) {
        let previous_len = self.items.len();
        self.items.retain(|i| !matches!(i, Invalidation::Layout(_)));
        if self.items.len() != previous_len {
            self.revision = self.revision.wrapping_add(1);
        }
        self.layout_ids.clear();
        self.rebuild_paint_indices();
    }

    fn rebuild_paint_indices(&mut self) {
        self.paint_indices.clear();
        for (index, item) in self.items.iter().enumerate() {
            if let Invalidation::Paint { id, .. } = item {
                self.paint_indices.insert(*id, index);
            }
        }
    }
}

/// 通过共享句柄推送 Paint 失效（State 绑定用）。
pub fn invalidate_paint_handle(handle: &InvalidationQueueHandle, id: NodeId, rect: Option<Rect>) {
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
