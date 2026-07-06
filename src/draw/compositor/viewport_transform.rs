//! Viewport 坐标变换 — content 与 viewport 空间统一（Phase 5）。
//!
//! 子节点 frame 处于 content 坐标（ScrollView layout 自然坐标），
//! dirty_region 处于 viewport/screen 坐标；绘制时 canvas translate(-scroll) 对齐。
//! 脏剪枝须先将 content frame 映射到 viewport 再与 dirty_region 求交。

use crate::core::Rect;

use crate::core::DirtyRegion;
use crate::draw::compositor::ScenePaint;
use crate::draw::pipeline::NodeId;

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

/// 节点是否需绘制：自身脏，或其 viewport 投影与 dirty_region 相交。
pub fn needs_paint(scene: &impl ScenePaint, node_id: NodeId, dirty_region: &DirtyRegion) -> bool {
    if scene.node_dirty(node_id) {
        return true;
    }
    let frame = scene.node_frame(node_id);
    if frame.w <= 0.0 || frame.h <= 0.0 {
        return false;
    }
    let (sx, sy) = cumulative_scroll(scene, node_id);
    let vp = content_to_viewport(frame, sx, sy);
    dirty_region.intersects(vp)
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
    let (sx, sy) = cumulative_scroll(scene, node_id);
    let vp = content_to_viewport(area, sx, sy);
    dirty_region.intersects(vp)
}

fn is_viewport(scene: &impl ScenePaint, id: NodeId) -> bool {
    scene.children_clip(id, scene.node_frame(id)).is_some()
}

#[cfg(test)]
#[path = "../../tests/draw/compositor/viewport_transform.rs"]
mod tests;
