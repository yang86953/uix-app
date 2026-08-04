//! 跨节点文字拖选 — 同父级下连续 Typography / selectable Label / RichText。
//!
//! PointerDown 落在某一文字节点后，`pressed_component` 会把后续 PointerMove
//! 全部派发给该节点；节点内选区无法越过自身边界。本模块在拖选进行中按
//! 文档序协调兄弟节点的选区，使向上/向下拖动能覆盖相邻文字行。
//!
//! 复制时焦点仍在起点节点，单节点 `selected_text` 只含本行；须按同父级
//! 文档序聚合各参与者选区（行间 `\n`）再写入剪贴板。

use crate::core::Point;
use crate::core::Rect;
use crate::ui::component::clipboard;
use crate::ui::component::widget::tree_core::WidgetTree;
use crate::ui::component::widget::{BoxedWidget, WidgetCore, WidgetId};
use crate::ui::widgets::general::label::Label;
use crate::ui::widgets::general::typography::Typography;
// 富文本 capability 关闭时不引用已裁剪的组件模块。
#[cfg(feature = "rich-text")]
use crate::ui::widgets::other::rich_text::RichText;

pub(crate) fn participates(node: &BoxedWidget) -> bool {
    let c = node.component();
    if c.as_any().is::<Typography>() {
        return true;
    }
    // 启用富文本后才把 RichText 纳入跨节点选择参与者。
    #[cfg(feature = "rich-text")]
    if c.as_any().is::<RichText>() {
        return true;
    }
    if let Some(label) = c.as_any().downcast_ref::<Label>() {
        return label.participates_in_cross_text_selection();
    }
    false
}

pub(crate) fn is_dragging(node: &BoxedWidget) -> bool {
    let c = node.component();
    if let Some(t) = c.as_any().downcast_ref::<Typography>() {
        return t.is_cross_text_dragging();
    }
    // 启用富文本后才读取 RichText 的拖选状态。
    #[cfg(feature = "rich-text")]
    if let Some(r) = c.as_any().downcast_ref::<RichText>() {
        return r.is_cross_text_dragging();
    }
    if let Some(l) = c.as_any().downcast_ref::<Label>() {
        return l.is_cross_text_dragging();
    }
    false
}

fn text_len(node: &BoxedWidget) -> usize {
    let c = node.component();
    if let Some(t) = c.as_any().downcast_ref::<Typography>() {
        return t.cross_text_len();
    }
    // 启用富文本后才读取 RichText 的字符长度。
    #[cfg(feature = "rich-text")]
    if let Some(r) = c.as_any().downcast_ref::<RichText>() {
        return r.cross_text_len();
    }
    if let Some(l) = c.as_any().downcast_ref::<Label>() {
        return l.cross_text_len();
    }
    0
}

fn text_anchor(node: &BoxedWidget) -> usize {
    let c = node.component();
    if let Some(t) = c.as_any().downcast_ref::<Typography>() {
        return t.cross_text_anchor();
    }
    // 启用富文本后才读取 RichText 的选择锚点。
    #[cfg(feature = "rich-text")]
    if let Some(r) = c.as_any().downcast_ref::<RichText>() {
        return r.cross_text_anchor();
    }
    if let Some(l) = c.as_any().downcast_ref::<Label>() {
        return l.cross_text_anchor();
    }
    0
}

fn set_range(node: &BoxedWidget, range: Option<(usize, usize)>) {
    let c = node.component();
    if let Some(t) = c.as_any().downcast_ref::<Typography>() {
        t.set_cross_text_range(range);
        return;
    }
    // 启用富文本后才同步 RichText 的跨节点选区。
    #[cfg(feature = "rich-text")]
    if let Some(r) = c.as_any().downcast_ref::<RichText>() {
        r.set_cross_text_range(range);
        return;
    }
    if let Some(l) = c.as_any().downcast_ref::<Label>() {
        l.set_cross_text_range(range);
    }
}

fn char_at(node: &BoxedWidget, frame_local: Point) -> usize {
    let c = node.component();
    if let Some(t) = c.as_any().downcast_ref::<Typography>() {
        return t.cross_text_char_at(frame_local);
    }
    // 启用富文本后才执行 RichText 的字符命中测试。
    #[cfg(feature = "rich-text")]
    if let Some(r) = c.as_any().downcast_ref::<RichText>() {
        return r.cross_text_char_at(frame_local);
    }
    if let Some(l) = c.as_any().downcast_ref::<Label>() {
        return l.cross_text_char_at(frame_local);
    }
    0
}

fn node_selected_text(node: &BoxedWidget) -> Option<String> {
    let c = node.component();
    if let Some(t) = c.as_any().downcast_ref::<Typography>() {
        return t.selected_text();
    }
    // 启用富文本后才聚合 RichText 的已选文字。
    #[cfg(feature = "rich-text")]
    if let Some(r) = c.as_any().downcast_ref::<RichText>() {
        return r.selected_text();
    }
    if let Some(l) = c.as_any().downcast_ref::<Label>() {
        return l.selected_text();
    }
    None
}

impl WidgetTree {
    /// 按同父级文档序聚合跨节点选区文本；无任何选区时返回 `None`。
    /// 多节点选区以 `\n` 连接，与视觉分行一致。
    pub(crate) fn aggregate_cross_text_selection(&self, anchor: WidgetId) -> Option<String> {
        if !self.get(anchor).is_some_and(participates) {
            return None;
        }
        let group: Vec<WidgetId> = match self.get(anchor).and_then(|n| n.parent()) {
            Some(parent) => self
                .get(parent)
                .map(|p| {
                    p.children()
                        .iter()
                        .copied()
                        .filter(|&id| self.get(id).is_some_and(participates))
                        .collect()
                })
                .unwrap_or_default(),
            None => vec![anchor],
        };
        let mut parts: Vec<String> = Vec::new();
        for id in group {
            if let Some(text) = self.get(id).and_then(node_selected_text) {
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        }
    }

    /// 若焦点文字节点存在选区（含跨兄弟），写入剪贴板并返回 true。
    /// 无选区时返回 false，交给组件自身处理（如 Typography 复制全文）。
    pub(crate) fn try_copy_cross_text_selection(&self, anchor: WidgetId) -> bool {
        let Some(text) = self.aggregate_cross_text_selection(anchor) else {
            return false;
        };
        clipboard::copy_to_clipboard(&text);
        true
    }

    /// PointerDown 开始文字拖选时，清除同父级其他参与者的选区。
    pub(crate) fn clear_sibling_cross_text_selections(&mut self, anchor: WidgetId) {
        let Some(parent) = self.get(anchor).and_then(|n| n.parent()) else {
            return;
        };
        let siblings: Vec<WidgetId> = self
            .get(parent)
            .map(|n| n.children().to_vec())
            .unwrap_or_default();
        for id in siblings {
            if id == anchor {
                continue;
            }
            if self.get(id).is_some_and(participates) {
                if let Some(node) = self.get(id) {
                    set_range(node, None);
                }
                self.invalidate_paint(id);
            }
        }
    }

    /// 拖选进行中：按文档序把选区从锚点节点扩展到当前指针下的兄弟文字节点。
    pub(crate) fn apply_cross_text_selection_drag(
        &mut self,
        anchor: WidgetId,
        screen_pos: Point,
    ) -> bool {
        let Some(parent) = self.get(anchor).and_then(|n| n.parent()) else {
            // 无兄弟可协调时退回单节点 PointerMove。
            return false;
        };
        let group: Vec<WidgetId> = {
            let Some(p) = self.get(parent) else {
                return false;
            };
            p.children()
                .iter()
                .copied()
                .filter(|&id| self.get(id).is_some_and(participates))
                .collect()
        };
        if group.len() < 2 {
            return false;
        }
        let Some(anchor_idx) = group.iter().position(|&id| id == anchor) else {
            return false;
        };

        let (focus_id, focus_char) = match self.resolve_cross_text_focus(&group, anchor, screen_pos)
        {
            Some(v) => v,
            None => return false,
        };
        let Some(focus_idx) = group.iter().position(|&id| id == focus_id) else {
            return false;
        };

        let anchor_char = self.get(anchor).map(text_anchor).unwrap_or(0);
        let lo = anchor_idx.min(focus_idx);
        let hi = anchor_idx.max(focus_idx);
        // 文档序：靠前节点为选区起点侧，靠后为终点侧。
        let forward = focus_idx >= anchor_idx;

        for (i, &id) in group.iter().enumerate() {
            let range = if i < lo || i > hi {
                None
            } else if i == anchor_idx && i == focus_idx {
                let a = anchor_char;
                let b = focus_char;
                if a == b {
                    None
                } else {
                    Some((a.min(b), a.max(b)))
                }
            } else if i == anchor_idx {
                let len = self.get(id).map(text_len).unwrap_or(0);
                if forward {
                    // 向下拖：锚点行从 anchor → 行尾
                    if anchor_char >= len {
                        None
                    } else {
                        Some((anchor_char, len))
                    }
                } else {
                    // 向上拖：锚点行从行首 → anchor
                    if anchor_char == 0 {
                        None
                    } else {
                        Some((0, anchor_char))
                    }
                }
            } else if i == focus_idx {
                let len = self.get(id).map(text_len).unwrap_or(0);
                if forward {
                    // 焦点在下方：行首 → focus
                    if focus_char == 0 {
                        None
                    } else {
                        Some((0, focus_char.min(len)))
                    }
                } else {
                    // 焦点在上方：focus → 行尾
                    if focus_char >= len {
                        None
                    } else {
                        Some((focus_char, len))
                    }
                }
            } else {
                // 完全夹在中间的行：整行选中
                let len = self.get(id).map(text_len).unwrap_or(0);
                if len == 0 {
                    None
                } else {
                    Some((0, len))
                }
            };

            if let Some(node) = self.get(id) {
                set_range(node, range);
            }
            self.invalidate_paint(id);
        }
        true
    }

    fn resolve_cross_text_focus(
        &self,
        group: &[WidgetId],
        anchor: WidgetId,
        screen_pos: Point,
    ) -> Option<(WidgetId, usize)> {
        // 优先 hit_test；命中参与者且属于本组时直接使用。
        if let Some(hit) = self.hit_test(screen_pos) {
            if let Some(id) = self.find_group_member(hit, group) {
                let local = self.frame_local_pointer(id, screen_pos);
                let ci = self.get(id).map(|n| char_at(n, local)).unwrap_or(0);
                return Some((id, ci));
            }
        }

        // 落在行间空隙 / 非文字节点：按 Y 落到最近的参与者。
        let mut best: Option<(WidgetId, f32)> = None;
        for &id in group {
            let Some(node) = self.get(id) else {
                continue;
            };
            let frame = self.viewport_frame_for_hit(id, node.frame());
            let cy = frame.y + frame.h * 0.5;
            let dist = if screen_pos.y < frame.y {
                frame.y - screen_pos.y
            } else if screen_pos.y > frame.y + frame.h {
                screen_pos.y - (frame.y + frame.h)
            } else {
                0.0
            };
            let score = dist * 1000.0 + (cy - screen_pos.y).abs();
            if best.is_none_or(|(_, b)| score < b) {
                best = Some((id, score));
            }
        }
        let (id, _) = best?;
        let local = self.frame_local_pointer(id, screen_pos);
        // 空隙中按相对锚点方向钳到行首/行尾，避免停在中间字符。
        let len = self.get(id).map(text_len).unwrap_or(0);
        let ci = if id == anchor {
            self.get(id).map(|n| char_at(n, local)).unwrap_or(0)
        } else {
            let anchor_frame = self
                .get(anchor)
                .map(|n| self.viewport_frame_for_hit(anchor, n.frame()));
            let focus_frame = self.viewport_frame_for_hit(id, self.get(id)?.frame());
            let going_up = anchor_frame.is_some_and(|af| focus_frame.y + focus_frame.h <= af.y);
            if going_up {
                0
            } else {
                len
            }
        };
        Some((id, ci))
    }

    fn find_group_member(&self, hit: WidgetId, group: &[WidgetId]) -> Option<WidgetId> {
        let mut current = Some(hit);
        while let Some(id) = current {
            if group.contains(&id) {
                return Some(id);
            }
            current = self.get(id).and_then(|n| n.parent());
        }
        None
    }

    fn frame_local_pointer(&self, id: WidgetId, screen_pos: Point) -> Point {
        let frame = self.get(id).map(|n| n.frame()).unwrap_or_else(Rect::zero);
        self.point_to_node_layout(id, screen_pos)
            .map(|point| Point::new(point.x - frame.x, point.y - frame.y))
            .unwrap_or_default()
    }

    /// hit-test / 指针比较用的 visual viewport 空间 frame。
    fn viewport_frame_for_hit(&self, id: WidgetId, content_frame: Rect) -> Rect {
        self.node_visual_rect(id, content_frame)
            .unwrap_or(content_frame)
    }
}
