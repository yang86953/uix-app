use std::collections::{HashMap, HashSet};

use crate::core::Rect;
use crate::draw::RenderTarget;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::painting::DisplayList;
use crate::draw::scene::NodeId;
use crate::draw::scene::{PicturePolicy, ScenePaint};

use super::{
    LayerNode, LayerTree, PICTURE_CACHE_BUDGET_BYTES, PictureCandidate, PictureSubtreeStats,
};

impl LayerTree {
    /// 创建不包含根图层或浮层的空图层树。
    pub fn new() -> Self {
        Self {
            root: None,
            overlays: Vec::new(),
            orphaned_handles: Vec::new(),
        }
    }

    /// 从 ScenePaint 构建图层树。
    /// 生成所有可见 widget 的 LayerNode（无遗漏）。
    /// 自动复用已缓存离屏缓冲——按 node_id 匹配旧 Picture 节点。
    /// 子节点按 z_index 预排序，渲染时无需再排序（#97）。
    ///
    /// `supports_offscreen`：引擎无离屏能力时不提升为 Picture（避免每帧 create 失败刷 WARN）。
    ///
    /// 注意：build 后未复用的旧离屏缓冲句柄暂存在 `orphaned_handles` 中，
    /// 调用者需在合适的时机调用 `sweep_orphaned_offscreens` 释放。
    pub fn build(&mut self, scene: &impl ScenePaint, supports_offscreen: bool) {
        // Take ownership of old Picture nodes' display lists and offscreen handles
        // to avoid cloning in the hot path.
        let mut old_cache: HashMap<NodeId, (Rect, Option<ImageHandle>, Option<DisplayList>)> =
            HashMap::new();
        if let Some(ref mut root) = self.root {
            Self::collect_picture_handles(root, &mut old_cache);
        }
        for overlay in &mut self.overlays {
            Self::collect_picture_handles(overlay, &mut old_cache);
        }

        let mut overlay_ids = Vec::new();
        if let Some(root_id) = scene.root_id() {
            Self::collect_overlay_node_ids(scene, root_id, &mut overlay_ids);
        }

        let selected_pictures = Self::select_picture_candidates(
            scene,
            scene.root_id().filter(|id| !scene.node_is_overlay(*id)),
            &overlay_ids,
            supports_offscreen,
            &old_cache,
        );

        self.root = scene
            .root_id()
            .filter(|id| !scene.node_is_overlay(*id))
            .and_then(|root_id| {
                Self::build_node_cached(
                    scene,
                    root_id,
                    0,
                    &mut old_cache,
                    supports_offscreen,
                    &selected_pictures,
                )
            });
        self.overlays = overlay_ids
            .into_iter()
            .filter_map(|id| {
                Self::build_overlay_node_cached(
                    scene,
                    id,
                    0,
                    &mut old_cache,
                    supports_offscreen,
                    &selected_pictures,
                )
            })
            .collect();
        self.overlays
            .sort_by_key(|overlay| scene.node_z_index(overlay.node_id()));

        // 收集未复用的旧句柄（需要在引擎上下文中释放）
        self.orphaned_handles.clear();
        for (_, (_, handle_opt, _)) in old_cache {
            if let Some(h) = handle_opt {
                self.orphaned_handles.push(h);
            }
        }
    }

    /// 释放 build 后未复用的旧离屏缓冲。
    /// 必须在 build 之后、下一帧渲染之前调用。
    pub fn sweep_orphaned_offscreens(
        &mut self,
        engine: &mut dyn RenderTarget,
    ) -> Result<(), crate::core::Error> {
        let handles = std::mem::take(&mut self.orphaned_handles);
        for (index, handle) in handles.iter().copied().enumerate() {
            if let Err(error) = engine.try_destroy_offscreen(handle) {
                self.orphaned_handles.extend_from_slice(&handles[index..]);
                return Err(error);
            }
        }
        Ok(())
    }

    /// 增量更新脏状态。
    pub fn update_dirty(&mut self, scene: &impl ScenePaint) {
        if let Some(ref mut root) = self.root {
            Self::update_dirty_node(root, scene);
        }
        for overlay in &mut self.overlays {
            Self::update_dirty_node(overlay, scene);
        }
    }

    /// 单 Pass 渲染：按 z-order 合成 widget 绘制与调试覆盖（Phase 7）。
    ///
    /// 不绘制全局焦点环（#177）；交互反馈由控件自身（如 Button Material ripple）负责。
    ///
    /// `paint_region` 为实际绘制剪枝区域（可与 `scene.dirty_region()` 不同，
    /// 例如首帧强制全帧重绘时由 ScenePipeline 传入 `DirtyRegion::full()`）。
    pub fn invalidate(&mut self) {
        if let Some(ref mut root) = self.root {
            root.mark_cache_dirty();
        }
        for overlay in &mut self.overlays {
            overlay.mark_cache_dirty();
        }
    }

    /// 返回图层树是否包含可供渲染的根图层或浮层。
    pub fn is_ready(&self) -> bool {
        self.root.is_some() || !self.overlays.is_empty()
    }

    // ── 内部 ──

    /// 为需要独立持有生命周期的 LayerNode 创建一次片段快照。
    fn clip_regions_snapshot(scene: &impl ScenePaint, id: NodeId) -> Option<Vec<Rect>> {
        let mut snapshot = None;
        scene.visit_node_clip_regions(id, &mut |regions| {
            snapshot = Some(regions.to_vec());
        });
        snapshot
    }

    /// 判断节点是否声明片段元数据，保留 Some(empty) 的“完全隐藏”语义。
    fn node_has_clip_regions(scene: &impl ScenePaint, id: NodeId) -> bool {
        scene.visit_node_clip_regions(id, &mut |_| {})
    }

    /// 同步 LayerNode 已有片段快照；内容不变时不复制也不重新申请容量。
    fn sync_clip_regions_snapshot(
        current: &mut Option<Vec<Rect>>,
        scene: &impl ScenePaint,
        id: NodeId,
    ) -> bool {
        let mut present = false;
        let mut changed = false;
        scene.visit_node_clip_regions(id, &mut |regions| {
            present = true;
            if current.as_deref() != Some(regions) {
                let snapshot = current.get_or_insert_with(Vec::new);
                snapshot.clear();
                snapshot.extend_from_slice(regions);
                changed = true;
            }
        });
        if present {
            changed
        } else {
            current.take().is_some()
        }
    }

    /// Take ownership of old Picture nodes' display lists and offscreen handles
    /// (avoiding clone). The old tree is discarded after build, so moving out is
    /// safe.
    fn collect_picture_handles(
        node: &mut LayerNode,
        cache: &mut HashMap<NodeId, (Rect, Option<ImageHandle>, Option<DisplayList>)>,
    ) {
        match node {
            LayerNode::Picture {
                node_id,
                bounds,
                offscreen_handle,
                display_list,
                ..
            } => {
                cache.insert(*node_id, (*bounds, *offscreen_handle, display_list.take()));
            }
            LayerNode::ClipRect { children, .. } | LayerNode::Direct { children, .. } => {
                for child in children {
                    Self::collect_picture_handles(child, cache);
                }
            }
        }
    }

    fn collect_overlay_node_ids(scene: &impl ScenePaint, id: NodeId, ids: &mut Vec<NodeId>) {
        if !scene.node_visible(id) {
            return;
        }
        if scene.node_is_overlay(id) {
            ids.push(id);
        }
        for child in scene.node_children(id) {
            Self::collect_overlay_node_ids(scene, *child, ids);
        }
    }

    /// 带缓存复用的 build_node。
    /// 如果旧缓存中有相同 node_id 且 bounds 未变的 Picture，复用其离屏句柄。
    ///
    /// 注意：`frame` 是绝对坐标（由 layout 阶段设置），以下所有位置计算直接使用 `frame` 的坐标，
    /// 不再累加父级偏移。
    fn build_node_cached(
        scene: &impl ScenePaint,
        id: NodeId,
        depth: usize,
        cache: &mut HashMap<NodeId, (Rect, Option<ImageHandle>, Option<DisplayList>)>,
        supports_offscreen: bool,
        selected_pictures: &HashSet<NodeId>,
    ) -> Option<LayerNode> {
        if !scene.node_visible(id) {
            return None;
        }
        let frame = scene.node_frame(id);
        let transform = scene.node_transform(id);
        let opacity = scene.node_opacity(id).clamp(0.0, 1.0);
        // 捕获父布局为当前节点声明的不连续裁剪片段。
        let clip_regions = Self::clip_regions_snapshot(scene, id);
        let descendants_support_offscreen = supports_offscreen && transform.is_identity();

        if supports_offscreen && selected_pictures.contains(&id) {
            // frame 是绝对坐标，直接用作 bounds
            let bounds = frame;
            // Take ownership from cache (remove instead of get+clone)
            let (offscreen_handle, display_list) = cache
                .remove(&id)
                .map(|(old_bounds, old_handle, old_list)| {
                    if old_bounds == bounds {
                        (old_handle, old_list)
                    } else {
                        (None, None)
                    }
                })
                .unwrap_or((None, None));
            let children = Self::build_children_cached(
                scene,
                id,
                depth,
                cache,
                descendants_support_offscreen,
                selected_pictures,
            );
            Some(LayerNode::Picture {
                node_id: id,
                bounds,
                is_dirty: offscreen_handle.is_none()
                    || display_list.is_none()
                    || scene.node_dirty(id),
                offscreen_handle,
                display_list,
                // 图片节点沿用统一的父级片段元数据。
                clip_regions,
                children,
                retry_count: 0,
            })
        } else if let Some(clip) = scene.children_clip(id, frame) {
            // clip 由 children_clip 基于 frame（绝对坐标）计算，直接使用
            let adj = Rect::new(clip.x, clip.y, clip.w, clip.h);
            let children = Self::build_children_cached(
                scene,
                id,
                depth,
                cache,
                descendants_support_offscreen,
                selected_pictures,
            );
            Some(LayerNode::ClipRect {
                node_id: id,
                rect: adj,
                transform,
                opacity,
                // 在节点自身变换之前应用父级片段。
                clip_regions,
                children,
            })
        } else {
            let children = Self::build_children_cached(
                scene,
                id,
                depth,
                cache,
                descendants_support_offscreen,
                selected_pictures,
            );
            Some(LayerNode::Direct {
                node_id: id,
                transform,
                opacity,
                // 在节点自身变换之前应用父级片段。
                clip_regions,
                children,
            })
        }
    }

    fn build_overlay_node_cached(
        scene: &impl ScenePaint,
        id: NodeId,
        depth: usize,
        cache: &mut HashMap<NodeId, (Rect, Option<ImageHandle>, Option<DisplayList>)>,
        supports_offscreen: bool,
        selected_pictures: &HashSet<NodeId>,
    ) -> Option<LayerNode> {
        if !scene.node_visible(id) {
            return None;
        }
        // 根浮层脱离普通父子遍历，使用场景声明的最终根画布变换。
        let transform = scene.node_overlay_transform(id);
        let opacity = scene.node_opacity(id).clamp(0.0, 1.0);
        // overlay 通常没有父级片段，但仍保持节点结构一致。
        let clip_regions = Self::clip_regions_snapshot(scene, id);
        Some(LayerNode::Direct {
            node_id: id,
            transform,
            opacity,
            // 保存可选的父级片段快照。
            clip_regions,
            children: Self::build_children_cached(
                scene,
                id,
                depth,
                cache,
                supports_offscreen && transform.is_identity(),
                selected_pictures,
            ),
        })
    }

    fn select_picture_candidates(
        scene: &impl ScenePaint,
        root_id: Option<NodeId>,
        overlay_ids: &[NodeId],
        supports_offscreen: bool,
        cache: &HashMap<NodeId, (Rect, Option<ImageHandle>, Option<DisplayList>)>,
    ) -> HashSet<NodeId> {
        if !supports_offscreen {
            return HashSet::new();
        }

        let mut candidates = Vec::new();
        if let Some(root_id) = root_id {
            Self::collect_picture_candidates(scene, root_id, true, true, cache, &mut candidates);
        }
        // Overlay roots are always direct, but their normal descendants retain
        // the same Picture eligibility as before.
        for overlay_id in overlay_ids {
            Self::collect_picture_candidates(
                scene,
                *overlay_id,
                true,
                false,
                cache,
                &mut candidates,
            );
        }

        candidates.sort_by(|left, right| {
            let left_weighted =
                (left.estimated_repaint_work as u128).saturating_mul(right.retained_bytes as u128);
            let right_weighted =
                (right.estimated_repaint_work as u128).saturating_mul(left.retained_bytes as u128);
            right_weighted
                .cmp(&left_weighted)
                .then_with(|| {
                    right
                        .estimated_repaint_work
                        .cmp(&left.estimated_repaint_work)
                })
                .then_with(|| right.reusable.cmp(&left.reusable))
                .then_with(|| left.node_id.cmp(&right.node_id))
        });

        let mut retained_bytes = 0usize;
        let mut selected = HashSet::new();
        for candidate in candidates {
            let Some(next_retained_bytes) = retained_bytes.checked_add(candidate.retained_bytes)
            else {
                continue;
            };
            if next_retained_bytes > PICTURE_CACHE_BUDGET_BYTES {
                continue;
            }
            retained_bytes = next_retained_bytes;
            selected.insert(candidate.node_id);
        }
        selected
    }

    fn collect_picture_candidates(
        scene: &impl ScenePaint,
        id: NodeId,
        supports_offscreen: bool,
        allow_self: bool,
        cache: &HashMap<NodeId, (Rect, Option<ImageHandle>, Option<DisplayList>)>,
        candidates: &mut Vec<PictureCandidate>,
    ) {
        if !scene.node_visible(id) {
            return;
        }

        let frame = scene.node_frame(id);
        let transform = scene.node_transform(id);
        if allow_self && supports_offscreen {
            let stats = Self::picture_subtree_stats(scene, id);
            if stats.eligible() {
                if let Some(retained_bytes) = Self::picture_retained_bytes(frame) {
                    let estimated_pixels = if stats.estimated_pixels.is_finite() {
                        stats.estimated_pixels.max(0.0).ceil() as u64
                    } else {
                        u64::MAX
                    };
                    let reusable = cache
                        .get(&id)
                        .is_some_and(|(bounds, handle, _)| *bounds == frame && handle.is_some());
                    candidates.push(PictureCandidate {
                        node_id: id,
                        retained_bytes,
                        estimated_repaint_work: estimated_pixels,
                        reusable,
                    });
                }
            }
        }

        let descendants_support_offscreen = supports_offscreen && transform.is_identity();
        for child in scene.node_children(id) {
            if scene.node_is_overlay(*child) {
                continue;
            }
            Self::collect_picture_candidates(
                scene,
                *child,
                descendants_support_offscreen,
                true,
                cache,
                candidates,
            );
        }
    }

    fn picture_retained_bytes(frame: Rect) -> Option<usize> {
        if !frame.w.is_finite() || !frame.h.is_finite() || frame.w <= 0.0 || frame.h <= 0.0 {
            return None;
        }
        let width = frame.w.ceil() as u64;
        let height = frame.h.ceil() as u64;
        let bytes = width
            .checked_mul(height)?
            .checked_mul(std::mem::size_of::<u32>() as u64)?;
        usize::try_from(bytes).ok()
    }

    fn picture_subtree_stats(scene: &impl ScenePaint, id: NodeId) -> PictureSubtreeStats {
        let frame = scene.node_frame(id);
        let mut stats = PictureSubtreeStats {
            node_count: 1,
            estimated_pixels: (frame.w.max(0.0) * frame.h.max(0.0)).max(0.0),
            cacheable: Self::node_allows_picture(scene, id, frame),
        };

        for child in scene.node_children(id) {
            if scene.node_is_overlay(*child) {
                continue;
            }
            let child_stats = Self::picture_subtree_stats(scene, *child);
            stats.node_count += child_stats.node_count;
            stats.estimated_pixels += child_stats.estimated_pixels;
            stats.cacheable &= child_stats.cacheable;
        }

        stats
    }

    fn node_allows_picture(scene: &impl ScenePaint, id: NodeId, frame: Rect) -> bool {
        scene.node_picture_policy(id) == PicturePolicy::Eligible
            // 二阶段覆盖节点不能被只缓存 Content 的 Picture 吞掉。
            && !scene.node_paints_after_children(id)
            && !scene.node_has_semantic_handlers(id)
            && !scene.node_has_dynamic_content(id)
            && !scene.node_has_interactive_state(id)
            && !scene.node_wants_continuous_pointer_move(id)
            && !scene.node_is_overlay(id)
            && !scene.node_focusable(id)
            // 带父级片段的节点必须在主合成路径逐片重放，不能提升为单张 Picture。
            && !Self::node_has_clip_regions(scene, id)
            && scene.children_clip(id, frame).is_none()
            && scene.scroll_offset(id).is_none()
            && scene.node_transform(id).is_identity()
            && scene.node_opacity(id) == 1.0
    }

    /// 带缓存复用的子节点构建，按 z_index 预排序（#97：排序缓存）。
    fn build_children_cached(
        scene: &impl ScenePaint,
        id: NodeId,
        depth: usize,
        cache: &mut HashMap<NodeId, (Rect, Option<ImageHandle>, Option<DisplayList>)>,
        supports_offscreen: bool,
        selected_pictures: &HashSet<NodeId>,
    ) -> Vec<LayerNode> {
        let mut children: Vec<LayerNode> = scene
            .node_children(id)
            .iter()
            .copied()
            .filter(|cid| !scene.node_is_overlay(*cid))
            .filter_map(|cid| {
                Self::build_node_cached(
                    scene,
                    cid,
                    depth + 1,
                    cache,
                    supports_offscreen,
                    selected_pictures,
                )
            })
            .collect();
        // 预排序：render 时无需再排序
        children.sort_by_key(|child| scene.node_z_index(child.node_id()));
        children
    }

    /// 递归更新脏状态。返回 true 表示该节点或其子树有脏节点。
    ///
    /// 关键语义：子节点脏 → 父 Picture 必须重新栅格化。
    /// 这是离屏缓存一致性的核心保证——没有这个传播，
    /// Picture 子树的动画变化会被缓存的旧内容覆盖，产生视觉残留。
    fn update_dirty_node(node: &mut LayerNode, scene: &impl ScenePaint) -> bool {
        match node {
            LayerNode::Picture {
                node_id,
                bounds,
                is_dirty,
                clip_regions,
                children,
                ..
            } => {
                // 同步父布局可能因滚动或重排改变的片段集合。
                let clip_regions_changed =
                    Self::sync_clip_regions_snapshot(clip_regions, scene, *node_id);
                // 窗口放大后若未 rebuild，仍须刷新 bounds，否则离屏/blit 卡在旧几何。
                let frame = scene.node_frame(*node_id);
                if *bounds != frame {
                    *bounds = frame;
                    *is_dirty = true;
                }
                // Picture 自身脏标记 + 子节点传播的脏标记（完整遍历，不短路）
                let self_dirty = scene.node_dirty(*node_id);
                let mut child_dirty = false;
                for child in children.iter_mut() {
                    if Self::update_dirty_node(child, scene) {
                        child_dirty = true;
                    }
                }
                *is_dirty = *is_dirty || clip_regions_changed || self_dirty || child_dirty;
                *is_dirty
            }
            LayerNode::ClipRect {
                node_id,
                rect,
                transform,
                opacity,
                clip_regions,
                children,
            } => {
                let next_transform = scene.node_transform(*node_id);
                let transform_changed = *transform != next_transform;
                *transform = next_transform;
                let next_opacity = scene.node_opacity(*node_id).clamp(0.0, 1.0);
                let opacity_changed = *opacity != next_opacity;
                *opacity = next_opacity;
                // 同步父布局可能因滚动或重排改变的片段集合。
                let clip_regions_changed =
                    Self::sync_clip_regions_snapshot(clip_regions, scene, *node_id);
                let frame = scene.node_frame(*node_id);
                if let Some(clip) = scene.children_clip(*node_id, frame) {
                    if *rect != clip {
                        *rect = clip;
                    }
                }
                let self_dirty = scene.node_dirty(*node_id);
                let mut child_dirty = false;
                for child in children.iter_mut() {
                    if Self::update_dirty_node(child, scene) {
                        child_dirty = true;
                    }
                }
                transform_changed
                    || opacity_changed
                    // 片段变化同样属于可见几何变化。
                    || clip_regions_changed
                    || self_dirty
                    || child_dirty
            }
            LayerNode::Direct {
                node_id,
                transform,
                opacity,
                clip_regions,
                children,
            } => {
                let next_transform = if scene.node_is_overlay(*node_id) {
                    // 浮层根在增量帧中继续使用场景声明的最终变换。
                    scene.node_overlay_transform(*node_id)
                } else {
                    scene.node_transform(*node_id)
                };
                let transform_changed = *transform != next_transform;
                *transform = next_transform;
                let next_opacity = scene.node_opacity(*node_id).clamp(0.0, 1.0);
                let opacity_changed = *opacity != next_opacity;
                *opacity = next_opacity;
                // 同步父布局可能因滚动或重排改变的片段集合。
                let clip_regions_changed =
                    Self::sync_clip_regions_snapshot(clip_regions, scene, *node_id);
                let self_dirty = scene.node_dirty(*node_id);
                let mut child_dirty = false;
                for child in children.iter_mut() {
                    if Self::update_dirty_node(child, scene) {
                        child_dirty = true;
                    }
                }
                transform_changed
                    || opacity_changed
                    // 片段变化同样属于可见几何变化。
                    || clip_regions_changed
                    || self_dirty
                    || child_dirty
            }
        }
    }
}

// 将浮层根变换回归测试统一存放到根 tests 目录。
#[cfg(test)]
#[path = "../../../../tests/unit/draw/scene/layer_tree_layout__tests.rs"]
mod tests;
