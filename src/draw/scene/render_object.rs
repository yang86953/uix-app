//! RenderObjectTree — 从 ScenePaint 构建的 DisplayList 缓存索引。

use std::collections::{HashMap, HashSet};

use crate::core::Rect;

use crate::draw::painting::PaintContext;
use crate::draw::painting::{DisplayList, PaintPass};
use crate::draw::scene::NodeId;
use crate::draw::scene::ScenePaint;

/// 单个 widget 的 Content 阶段绘制缓存。
#[derive(Debug, Clone)]
pub struct RenderObjectEntry {
    /// 最近一次场景同步得到的节点布局框。
    pub frame: Rect,
    /// 可重放的 Content 阶段绘制命令；尚未录制或不可缓存时为空。
    pub display_list: Option<DisplayList>,
    /// 当前缓存是否因场景或节点变化而不可重放。
    pub is_dirty: bool,
}

impl RenderObjectEntry {
    fn new(frame: Rect) -> Self {
        Self {
            frame,
            display_list: None,
            is_dirty: true,
        }
    }
}

/// RenderObject 索引树：node_id → Content DisplayList。
///
/// 与 Widget 树解耦，仅通过 `ScenePaint` 只读快照同步。
#[derive(Debug, Default)]
pub struct RenderObjectTree {
    entries: HashMap<NodeId, RenderObjectEntry>,
    /// 每帧复用的 Paint 失效身份快照，避免为每个节点重复同步与分配。
    dirty_nodes: HashSet<NodeId>,
    synced_version: u64,
}

impl RenderObjectTree {
    /// 创建尚未同步任何场景节点的空索引树。
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回当前索引中的可见场景节点数量。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 判断索引中是否没有可见场景节点。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 返回指定节点的 Content 缓存条目。
    pub fn get(&self, id: NodeId) -> Option<&RenderObjectEntry> {
        self.entries.get(&id)
    }

    /// 与场景同步：结构变更时重建索引，每帧刷新脏标记。
    pub fn sync(&mut self, scene: &impl ScenePaint) {
        let version = scene.tree_version();
        if version != self.synced_version {
            self.rebuild(scene);
            self.synced_version = version;
        }
        let full_paint = scene.paint_invalidation_snapshot_into(&mut self.dirty_nodes);
        Self::refresh_dirty(&mut self.entries, &self.dirty_nodes, full_paint, scene);
    }

    /// 尝试重放 Content DisplayList；成功则跳过 widget 直接绘制。
    pub fn try_replay(&self, id: NodeId, ctx: &mut PaintContext<'_>) -> bool {
        let Some(entry) = self.entries.get(&id) else {
            return false;
        };
        if entry.is_dirty {
            return false;
        }
        let Some(list) = entry.display_list.as_ref() else {
            return false;
        };
        if list.is_empty() {
            return false;
        }
        list.replay(ctx);
        true
    }

    /// 绘制 widget Content 并更新/复用 DisplayList 缓存。
    pub fn paint_content(
        &mut self,
        id: NodeId,
        frame: Rect,
        scene: &impl ScenePaint,
        ctx: &mut PaintContext<'_>,
    ) {
        let self_dirty = scene.node_dirty(id);
        let can_replay = if !self_dirty {
            self.try_replay(id, ctx)
        } else {
            false
        };
        if can_replay {
            return;
        }

        let entry = self
            .entries
            .entry(id)
            .or_insert_with(|| RenderObjectEntry::new(frame));
        entry.frame = frame;
        entry.is_dirty = false;

        let mut list = DisplayList::new();
        ctx.set_paint_pass(PaintPass::Content);
        ctx.with_recorder(&mut list, |ctx| {
            scene.paint(id, frame, ctx);
        });
        entry.display_list = ctx.recording_complete().then_some(list);
    }

    fn rebuild(&mut self, scene: &impl ScenePaint) {
        let mut next = HashMap::new();
        if let Some(root) = scene.root_id() {
            Self::collect_nodes(scene, root, &mut next);
        }
        // bounds 变化时丢弃旧 DisplayList
        for (id, entry) in &mut next {
            if let Some(old) = self.entries.get(id) {
                if old.frame == entry.frame {
                    entry.display_list = old.display_list.clone();
                    entry.is_dirty = old.is_dirty;
                }
            }
        }
        self.entries = next;
    }

    fn collect_nodes(
        scene: &impl ScenePaint,
        id: NodeId,
        out: &mut HashMap<NodeId, RenderObjectEntry>,
    ) {
        if !scene.node_visible(id) {
            return;
        }
        let frame = scene.node_frame(id);
        out.insert(id, RenderObjectEntry::new(frame));
        for &child in scene.node_children(id) {
            Self::collect_nodes(scene, child, out);
        }
    }

    fn refresh_dirty(
        entries: &mut HashMap<NodeId, RenderObjectEntry>,
        dirty_nodes: &HashSet<NodeId>,
        full_paint: Option<bool>,
        scene: &impl ScenePaint,
    ) {
        for (id, entry) in entries {
            if scene.node_visible(*id) {
                let frame = scene.node_frame(*id);
                if entry.frame != frame {
                    entry.frame = frame;
                    entry.is_dirty = true;
                    entry.display_list = None;
                }
                let node_dirty = match full_paint {
                    Some(true) => true,
                    Some(false) => dirty_nodes.contains(id),
                    None => scene.node_dirty(*id),
                };
                if node_dirty {
                    entry.is_dirty = true;
                }
            } else {
                entry.is_dirty = true;
                entry.display_list = None;
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/draw/scene/render_object__tests.rs"]
mod tests;
