//! Viewport 坐标变换 — content 与 viewport 空间统一（Phase 5）。
//!
//! 子节点 frame 处于 content 坐标（ScrollView layout 自然坐标），
//! dirty_region 处于 viewport/screen 坐标；绘制时 canvas translate(-scroll) 对齐。
//! 脏剪枝须先将 content frame 映射到 viewport 再与 dirty_region 求交。

use crate::core::Rect;

use crate::core::DirtyRegion;
use crate::draw::renderer::NodeId;
use crate::draw::scene::ScenePaint;
use crate::draw::Transform;

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
        transform = transform.concat(scene.node_transform(id));
        if index + 1 < path.len() {
            if let Some((sx, sy)) = scene.scroll_offset(id) {
                transform = transform.concat(Transform::translate(-sx, -sy));
            }
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

/// 节点在 viewport/screen 空间的可见矩形（累计祖先 scroll 与 children_clip）。
/// 完全滚出可视区或不可见时返回 `None`。与 `WidgetTree::visible_rect_for` 同语义。
pub fn visible_viewport_rect(scene: &impl ScenePaint, node_id: NodeId) -> Option<Rect> {
    if !scene.node_visible(node_id) {
        return None;
    }
    let frame = scene.node_frame(node_id);
    let mut rect = node_visual_rect(scene, node_id, frame);
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return None;
    }

    let path = visual_path(scene, node_id);
    let mut transform = Transform::identity();
    for (index, id) in path.iter().copied().enumerate() {
        if !scene.node_visible(id) {
            return None;
        }
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

/// 节点是否需绘制：自身脏，或其 viewport 投影与 dirty_region 相交。
pub fn needs_paint(scene: &impl ScenePaint, node_id: NodeId, dirty_region: &DirtyRegion) -> bool {
    if scene.node_dirty(node_id) {
        return true;
    }
    let frame = node_viewport_frame(scene, node_id);
    if frame.w <= 0.0 || frame.h <= 0.0 {
        return false;
    }
    dirty_region.intersects(frame)
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
