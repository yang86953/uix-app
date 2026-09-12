//! 跨节点文字拖选 — 同父级下连续 Typography / selectable Label / RichText。
//!
//! PointerDown 落在某一文字节点后，`pressed_widget` 会把后续 PointerMove
//! 全部派发给该节点；节点内选区无法越过自身边界。本模块在拖选进行中按
//! 文档序协调兄弟节点的选区，使向上/向下拖动能覆盖相邻文字行。
//!
//! 复制时焦点仍在起点节点，单节点 `selected_text` 只含本行；须按同父级
//! 文档序聚合各参与者选区（行间 `\n`）再写入剪贴板。
//!
//! 单节点（Label / Typography 共享）的布局缓存、命中与选区读写
//! 实现在 [`per_node`] 子模块，本层只负责跨节点的分组与协调。

// 引入单节点共享的文字选区状态实现。
pub(crate) mod per_node;

use crate::core::Point;
use crate::core::Rect;
use crate::ui::widget_runtime::clipboard;
use crate::ui::widget_runtime::widget::tree_core::WidgetTree;
use crate::ui::widget_runtime::widget::{BoxedWidget, WidgetCore, WidgetId};
use crate::ui::UserSelect;

pub(crate) fn participates(node: &BoxedWidget) -> bool {
    node.widget().as_text_selection().is_some_and(|text| text.selection_enabled())
}
pub(crate) fn is_dragging(node: &BoxedWidget) -> bool {
    node.widget().as_text_selection().is_some_and(|text| text.selection_dragging())
}
fn text_len(node: &BoxedWidget) -> usize {
    node.widget().as_text_selection().map(|text| text.selection_len()).unwrap_or(0)
}
fn text_anchor(node: &BoxedWidget) -> usize {
    node.widget().as_text_selection().map(|text| text.selection_anchor()).unwrap_or(0)
}
fn set_range(node: &BoxedWidget, range: Option<(usize, usize)>) {
    if let Some(text) = node.widget().as_text_selection() { text.set_selection_range(range); }
}
fn char_at(node: &BoxedWidget, point: Point) -> usize {
    node.widget().as_text_selection().map(|text| text.selection_char_at(point)).unwrap_or(0)
}
fn node_selected_text(node: &BoxedWidget) -> Option<String> {
    node.widget().as_text_selection()?.selection_text()
}
fn apply_widget_policy(node: &mut BoxedWidget, policy: UserSelect) {
    if let Some(text) = node.widget_mut().as_text_selection_mut() { text.set_selection_policy(policy); }
}

impl WidgetTree {
    // 安装一个节点的声明并重算其既有子树最终选择策略。
    pub(crate) fn set_node_user_select(&mut self, root: WidgetId, declared: UserSelect) {
        // 先保存当前声明，避免重算仍读取旧值。
        let Some((parent, previous_effective)) = self.get_mut(root).map(|root_node| {
            // 声明身份与 used-value 分开保存。
            root_node.set_declared_user_select(declared);
            // 在释放可变借用前保存继承入口与既有最终值。
            (root_node.parent(), root_node.effective_user_select())
        }) else {
            // 已移除节点没有可更新的策略状态。
            return;
        };
        // 读取真实父节点已经解析出的最终策略。
        let inherited = parent
            // 从树中读取父节点。
            .and_then(|parent| self.get(parent))
            // 复制父节点最终策略。
            .map(BoxedWidget::effective_user_select)
            // 根节点使用无约束 auto 起点。
            .unwrap_or(UserSelect::Auto);
        // 当前节点最终策略没有变化时，后代继承结果也保持不变。
        let effective = declared.resolve_with_parent(inherited);
        if previous_effective == effective {
            if let Some(root_node) = self.get_mut(root) {
                // 新协调进来的具体组件仍需接收既有最终策略。
                apply_widget_policy(root_node, effective);
            }
            return;
        }
        // 使用显式栈按父到子顺序重算任意深度子树。
        let mut stack = vec![(root, inherited)];
        // 直到全部既有后代完成协调。
        while let Some((id, parent_policy)) = stack.pop() {
            // 在单次可变借用中更新元数据与具体组件。
            let Some((effective, children, changed)) = self.get_mut(id).map(|node| {
                // 结合父级最终值解析当前声明。
                let effective = node
                    // 读取当前节点自己的声明。
                    .declared_user_select()
                    // 应用闭合父子 used-value 规则。
                    .resolve_with_parent(parent_policy);
                // 记录是否需要使现有选区绘制失效。
                let changed = node.effective_user_select() != effective;
                // 保存最终策略供事件查询和后代解析。
                node.set_effective_user_select(effective);
                // 同步具体文本组件的私有选择状态。
                apply_widget_policy(node, effective);
                // 复制子身份以在释放节点借用后继续遍历。
                let children = node.children().to_vec();
                // 返回本轮后续所需的小型值。
                (effective, children, changed)
            }) else {
                // 协调中失效的节点直接跳过。
                continue;
            };
            // 策略变化可能清除选区或改变选择高亮参与资格。
            if changed {
                // 只使当前实际节点的绘制输出失效。
                self.invalidate_paint(id);
            }
            // 逆序压栈以保持实际处理顺序与声明文档序一致。
            for child in children.into_iter().rev() {
                // 后代使用当前节点刚解析出的最终策略。
                stack.push((child, effective));
            }
        }
    }

    // 查找目标所属的最近显式 all 选择边界。
    fn nearest_all_boundary(&self, target: WidgetId) -> Option<WidgetId> {
        // 非 all 最终策略不存在整体选择边界。
        if self
            // 读取目标实际节点。
            .get(target)
            // 查询已解析策略。
            .is_none_or(|node| node.effective_user_select() != UserSelect::All)
        {
            // 直接返回无整体边界。
            return None;
        }
        // 从目标向根查找最近的显式 all 声明。
        let mut current = Some(target);
        // 父链有限，直到根节点结束。
        while let Some(id) = current {
            // 失效节点终止边界解析。
            let node = self.get(id)?;
            // 最近显式 all 声明拥有整体选择范围。
            if node.declared_user_select() == UserSelect::All {
                // 返回稳定实际节点身份。
                return Some(id);
            }
            // 继续检查直接父节点。
            current = node.parent();
        }
        // 防御性处理不一致元数据。
        None
    }

    // 按文档序收集一个实际子树中的全部文字选择参与者。
    fn selection_nodes_in_subtree(&self, root: WidgetId) -> Vec<WidgetId> {
        // 保存稳定的先序文档顺序结果。
        let mut result = Vec::new();
        // 显式栈避免深声明树递归溢出。
        let mut stack = vec![root];
        // 遍历全部实际后代。
        while let Some(id) = stack.pop() {
            // 已移除节点没有可选择内容。
            let Some(node) = self.get(id) else {
                // 继续处理其余稳定身份。
                continue;
            };
            // 支持的文字组件按最终策略决定参与资格。
            if participates(node) {
                // 保存当前文字节点身份。
                result.push(id);
            }
            // 逆序压栈使弹出顺序保持声明顺序。
            for child in node.children().iter().rev() {
                // 复制小型节点身份。
                stack.push(*child);
            }
        }
        // 返回稳定文档序参与者集合。
        result
    }

    // 若目标位于 all 边界内，选择边界中的全部文字并返回成功事实。
    pub(crate) fn select_all_user_select_subtree(&mut self, target: WidgetId) -> bool {
        // 只有实际文字参与者上的交互才能启动整体选择。
        if !self.get(target).is_some_and(participates) {
            // 非文字按钮或容器交互不应意外建立文字选区。
            return false;
        }
        // 找到最近 all 声明拥有的实际子树。
        let Some(boundary) = self.nearest_all_boundary(target) else {
            // 普通 auto/text 继续沿用既有选择行为。
            return false;
        };
        // 按文档序收集边界中的全部支持文字节点。
        let nodes = self.selection_nodes_in_subtree(boundary);
        // 空文本子树不声称已经选择。
        if nodes.is_empty() {
            // 返回未建立选择。
            return false;
        }
        // 为每个参与者建立完整逻辑文本范围。
        for id in nodes {
            // 读取当前节点完整逻辑字符数量。
            let length = self.get(id).map(text_len).unwrap_or(0);
            // 使用组件自身的字素簇和原子对象归一规则保存范围。
            if let Some(node) = self.get(id) {
                // 零长度组件自然清除空范围。
                set_range(node, Some((0, length)));
            }
            // 选区背景变化只需要重新绘制当前文字节点。
            self.invalidate_paint(id);
        }
        // 已完成整个边界的同步选择。
        true
    }

    /// 按同父级文档序聚合跨节点选区文本；无任何选区时返回 `None`。
    /// 多节点选区以 `\n` 连接，与视觉分行一致。
    pub(crate) fn aggregate_cross_text_selection(&self, anchor: WidgetId) -> Option<String> {
        if !self.get(anchor).is_some_and(participates) {
            return None;
        }
        // all 使用最近显式边界的完整子树，普通拖选继续使用同父级参与者。
        let group: Vec<WidgetId> = if let Some(boundary) = self.nearest_all_boundary(anchor) {
            // 整体选择按边界内完整文档序聚合。
            self.selection_nodes_in_subtree(boundary)
        } else {
            // 普通跨节点拖选保持既有同父级范围。
            match self.get(anchor).and_then(|n| n.parent()) {
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
            }
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
                if len == 0 { None } else { Some((0, len)) }
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
            if going_up { 0 } else { len }
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
