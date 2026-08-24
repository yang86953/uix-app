//! Viewport 坐标变换 — content 与 viewport 空间统一（Phase 5）。
//!
//! 子节点 frame 处于 content 坐标（ScrollView layout 自然坐标），
//! dirty_region 处于 viewport/screen 坐标；绘制时 canvas translate(-scroll) 对齐。
//! 脏剪枝须先将 content frame 映射到 viewport 再与 dirty_region 求交。

use crate::core::Rect;

use crate::core::DirtyRegion;
use crate::draw::Transform;
use crate::draw::scene::NodeId;
use crate::draw::scene::ScenePaint;

/// 沿视觉父链递归累计到当前节点的完整变换，不构造临时路径容器。
fn accumulate_visual_transform(scene: &impl ScenePaint, id: NodeId) -> Transform {
    // 浮层根已经携带提升前的最终根画布变换，不再继承普通父链。
    if scene.node_is_overlay(id) {
        return scene.node_overlay_transform(id);
    }
    let Some(parent) = scene.parent(id) else {
        return scene.node_transform(id);
    };
    // 先完成祖先变换，再在父子边界应用父视口滚动，最后应用当前节点变换。
    let mut transform = accumulate_visual_transform(scene, parent);
    if let Some((sx, sy)) = scene.scroll_offset(parent) {
        transform = transform.concat(Transform::translate(-sx, -sy));
    }
    transform.concat(scene.node_transform(id))
}

/// 沿完整组件父链累计浮层提升前的变换，不在中间浮层节点处截断。
fn accumulate_overlay_root_transform(scene: &impl ScenePaint, id: NodeId) -> Transform {
    let Some(parent) = scene.parent(id) else {
        return scene.node_transform(id);
    };
    let mut transform = accumulate_overlay_root_transform(scene, parent);
    if let Some((sx, sy)) = scene.scroll_offset(parent) {
        transform = transform.concat(Transform::translate(-sx, -sy));
    }
    transform.concat(scene.node_transform(id))
}

/// 单次父链递归期间累计的屏幕变换与祖先裁剪边界。
#[derive(Clone, Copy)]
struct VisiblePathState {
    transform: Transform,
    clip_bounds: Option<Rect>,
    has_ancestor: bool,
}

impl VisiblePathState {
    fn root() -> Self {
        Self {
            transform: Transform::identity(),
            clip_bounds: None,
            has_ancestor: false,
        }
    }

    /// 合并一个已投影到屏幕坐标的裁剪矩形。
    fn intersect_clip(&mut self, clip: Rect) -> Option<()> {
        self.clip_bounds = Some(match self.clip_bounds {
            Some(bounds) => bounds.intersect(&clip)?,
            None => clip,
        });
        Some(())
    }
}

/// 从视觉根递归回卷到目标节点，同时累计变换、可见性与裁剪。
fn accumulate_visible_path(
    scene: &impl ScenePaint,
    id: NodeId,
    target: NodeId,
) -> Option<VisiblePathState> {
    if !scene.node_visible(id) {
        return None;
    }
    let is_overlay_root = scene.node_is_overlay(id);
    let mut state = if is_overlay_root {
        VisiblePathState::root()
    } else if let Some(parent) = scene.parent(id) {
        accumulate_visible_path(scene, parent, target)?
    } else {
        VisiblePathState::root()
    };

    // 父级片段位于当前节点自身变换之前的父内容坐标系。
    if state.has_ancestor
        && let Some(regions) = scene.node_clip_regions(id)
    {
        let mut regions = regions.into_iter();
        let mut region_bounds = regions.next()?;
        for region in regions {
            region_bounds = region_bounds.union(&region);
        }
        state.intersect_clip(state.transform.transform_rect(region_bounds))?;
    }

    // 视觉根浮层消费完整根变换，其余节点消费常规节点变换。
    state.transform = state
        .transform
        .concat(if is_overlay_root && !state.has_ancestor {
            scene.node_overlay_transform(id)
        } else {
            scene.node_transform(id)
        });
    state.has_ancestor = true;

    if id != target {
        let node_frame = scene.node_frame(id);
        if let Some(clip) = scene.children_clip(id, node_frame) {
            state.intersect_clip(state.transform.transform_rect(clip))?;
        }
        if let Some((sx, sy)) = scene.scroll_offset(id) {
            state.transform = state.transform.concat(Transform::translate(-sx, -sy));
        }
    }
    Some(state)
}

/// Layout coordinates to viewport/screen coordinates for a node.
///
/// Each node transform applies before its descendants. A viewport's scroll
/// translation applies between the viewport transform and the child transform.
pub fn node_visual_transform(scene: &impl ScenePaint, node_id: NodeId) -> Transform {
    accumulate_visual_transform(scene, node_id)
}

/// 计算被提升为根浮层的节点在原组件树中的完整视觉变换。
///
/// 浮层绘制会脱离普通父子遍历，因此必须在提升前补回全部祖先变换与滚动位移；
/// 这里不能在当前浮层节点处截断，否则滚动容器中的下拉会回到内容原始坐标。
pub(crate) fn overlay_root_visual_transform(scene: &impl ScenePaint, node_id: NodeId) -> Transform {
    accumulate_overlay_root_transform(scene, node_id)
}

/// Maps layout geometry owned by `node_id` into viewport/screen coordinates.
pub fn node_visual_rect(scene: &impl ScenePaint, node_id: NodeId, rect: Rect) -> Rect {
    node_visual_transform(scene, node_id).transform_rect(rect)
}

/// 累计从根到 node 路径上所有 viewport 祖先的 scroll 偏移。
pub fn cumulative_scroll(scene: &impl ScenePaint, node_id: NodeId) -> (f32, f32) {
    let mut sx = 0.0f32;
    let mut sy = 0.0f32;
    let mut current = scene.parent(node_id);
    while let Some(pid) = current {
        if is_viewport(scene, pid) {
            if let Some((ox, oy)) = scene.scroll_offset(pid) {
                sx += ox;
                sy += oy;
            }
        }
        if scene.node_is_overlay(pid) {
            break;
        }
        current = scene.parent(pid);
    }
    (sx, sy)
}

/// content 坐标矩形 → viewport 可见坐标（与 dirty_region 同空间）。
pub fn content_to_viewport(content: Rect, scroll_x: f32, scroll_y: f32) -> Rect {
    Rect::new(
        content.x - scroll_x,
        content.y - scroll_y,
        content.w,
        content.h,
    )
}

/// 节点 frame 映射到 viewport/screen 坐标（累计祖先 scroll，不含 clip）。
pub fn node_viewport_frame(scene: &impl ScenePaint, node_id: NodeId) -> Rect {
    node_visual_rect(scene, node_id, scene.node_frame(node_id))
}

/// 将节点拥有的任意布局矩形投影到 viewport/screen 可见区域。
fn visible_viewport_rect_for(
    scene: &impl ScenePaint,
    node_id: NodeId,
    source_rect: Rect,
) -> Option<Rect> {
    // 单次父链递归同时生成完整变换和全部祖先裁剪，不创建 Vec<NodeId>。
    let state = accumulate_visible_path(scene, node_id, node_id)?;
    let rect = state.transform.transform_rect(source_rect);
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return None;
    }
    match state.clip_bounds {
        Some(clip) => rect.intersect(&clip),
        None => Some(rect),
    }
}

/// 节点在 viewport/screen 空间的可见矩形（累计祖先 scroll 与 children_clip）。
/// 完全滚出可视区或不可见时返回 `None`。与 `WidgetTree::visible_rect_for` 同语义。
pub fn visible_viewport_rect(scene: &impl ScenePaint, node_id: NodeId) -> Option<Rect> {
    visible_viewport_rect_for(scene, node_id, scene.node_frame(node_id))
}

/// 节点是否需绘制：自身脏，或其 viewport 投影与 dirty_region 相交。
pub fn needs_paint(scene: &impl ScenePaint, node_id: NodeId, dirty_region: &DirtyRegion) -> bool {
    if scene.node_dirty(node_id) {
        return true;
    }
    // 使用同时累计祖先视口与父级片段的真实可见包围盒。
    let Some(frame) = visible_viewport_rect(scene, node_id) else {
        // 完全落在片段外或视口外的节点无需绘制。
        return false;
    };
    // 只在真实可见包围盒与脏区域相交时提交节点。
    dirty_region.intersects(frame)
}

/// 主表面节点是否需绘制：脏节点也必须至少有实际绘制范围落在可见区域内。
///
/// Picture 离屏目标在重栅格化前会被完整清空，不能使用该裁剪入口。
pub(crate) fn needs_paint_in_viewport(
    scene: &impl ScenePaint,
    node_id: NodeId,
    dirty_region: &DirtyRegion,
) -> bool {
    let node_dirty = scene.node_dirty(node_id);
    let frame = scene.node_frame(node_id);
    // 脏节点可能通过阴影、徽标等视觉超出布局 frame，必须使用完整 dirty_rect。
    let paint_rect = if node_dirty {
        scene.dirty_rect(node_id, frame)
    } else {
        frame
    };
    let Some(visible_rect) = visible_viewport_rect_for(scene, node_id, paint_rect) else {
        return false;
    };
    node_dirty || dirty_region.intersects(visible_rect)
}

/// overlay / dirty_rect 区域是否需重绘。
pub fn needs_paint_rect(
    scene: &impl ScenePaint,
    node_id: NodeId,
    area: Rect,
    dirty_region: &DirtyRegion,
) -> bool {
    if area.w <= 0.0 || area.h <= 0.0 {
        return false;
    }
    dirty_region.intersects(node_visual_rect(scene, node_id, area))
}

fn is_viewport(scene: &impl ScenePaint, id: NodeId) -> bool {
    scene.children_clip(id, scene.node_frame(id)).is_some()
}

// 将主表面裁剪契约测试放在根 tests 目录，保留私有辅助函数访问能力。
#[cfg(test)]
#[path = "../../../tests/unit/draw/scene/viewport_transform__tests.rs"]
mod tests;
