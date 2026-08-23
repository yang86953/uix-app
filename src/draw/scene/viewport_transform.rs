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

fn visual_path(scene: &impl ScenePaint, node_id: NodeId) -> Vec<NodeId> {
    let mut path = Vec::new();
    let mut current = Some(node_id);
    while let Some(id) = current {
        path.push(id);
        if scene.node_is_overlay(id) {
            break;
        }
        current = scene.parent(id);
    }
    path.reverse();
    path
}

/// Layout coordinates to viewport/screen coordinates for a node.
///
/// Each node transform applies before its descendants. A viewport's scroll
/// translation applies between the viewport transform and the child transform.
pub fn node_visual_transform(scene: &impl ScenePaint, node_id: NodeId) -> Transform {
    let path = visual_path(scene, node_id);
    let mut transform = Transform::identity();
    for (index, id) in path.iter().copied().enumerate() {
        let node_transform = if index == 0 && scene.node_is_overlay(id) {
            // 浮层路径在提升边界处截断，首节点必须使用场景声明的根画布变换。
            scene.node_overlay_transform(id)
        } else {
            scene.node_transform(id)
        };
        transform = transform.concat(node_transform);
        if index + 1 < path.len() {
            if let Some((sx, sy)) = scene.scroll_offset(id) {
                transform = transform.concat(Transform::translate(-sx, -sy));
            }
        }
    }
    transform
}

/// 计算被提升为根浮层的节点在原组件树中的完整视觉变换。
///
/// 浮层绘制会脱离普通父子遍历，因此必须在提升前补回全部祖先变换与滚动位移；
/// 这里不能在当前浮层节点处截断，否则滚动容器中的下拉会回到内容原始坐标。
pub(crate) fn overlay_root_visual_transform(scene: &impl ScenePaint, node_id: NodeId) -> Transform {
    let mut path = Vec::new();
    let mut current = Some(node_id);
    while let Some(id) = current {
        path.push(id);
        current = scene.parent(id);
    }
    path.reverse();

    let mut transform = Transform::identity();
    for (index, id) in path.iter().copied().enumerate() {
        transform = transform.concat(scene.node_transform(id));
        if index + 1 < path.len()
            && let Some((sx, sy)) = scene.scroll_offset(id)
        {
            transform = transform.concat(Transform::translate(-sx, -sy));
        }
    }
    transform
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
    if !scene.node_visible(node_id) {
        return None;
    }
    let mut rect = node_visual_rect(scene, node_id, source_rect);
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return None;
    }

    let path = visual_path(scene, node_id);
    let mut transform = Transform::identity();
    for (index, id) in path.iter().copied().enumerate() {
        if !scene.node_visible(id) {
            return None;
        }
        // 父级片段位于当前节点自身变换之前的父内容坐标系。
        if index > 0 {
            // 只在父布局显式声明不连续片段时收缩可见区域。
            if let Some(regions) = scene.node_clip_regions(id) {
                // 空片段集合表示当前节点子树完全不可见。
                let mut regions = regions.into_iter();
                // 读取首个片段作为保守联合边界初值。
                let mut region_bounds = regions.next()?;
                // 合并其余片段，仅用于可见性与脏区的保守矩形投影。
                for region in regions {
                    // 使用矩形联合保留全部实际片段。
                    region_bounds = region_bounds.union(&region);
                }
                // 先用祖先与滚动变换投影片段，再同节点可见矩形求交。
                rect = rect.intersect(&transform.transform_rect(region_bounds))?;
            }
        }
        // 片段裁剪之后才应用当前节点自身视觉变换。
        transform = transform.concat(scene.node_transform(id));
        if index + 1 < path.len() {
            let node_frame = scene.node_frame(id);
            if let Some(clip) = scene.children_clip(id, node_frame) {
                rect = rect.intersect(&transform.transform_rect(clip))?;
            }
            if let Some((sx, sy)) = scene.scroll_offset(id) {
                transform = transform.concat(Transform::translate(-sx, -sy));
            }
        }
    }

    Some(rect)
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
