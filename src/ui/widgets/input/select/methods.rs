use crate::core::{Rect, Size};
// 读取组件树根 frame 时引入核心几何能力。
use crate::ui::widget_runtime::widget::WidgetCore;
// 布局阶段通过组件树取得当前逻辑表面。
use crate::ui::WidgetTree;

use super::search::VisibleRow;
// 复用选择弹层的共享表面解析原语。
use super::{Select, normalize_select_rect, resolve_select_popup_rect, select_fallback_surface};

impl Select {
    pub(crate) fn control_height(&self) -> f32 {
        self.visual.layout.control_height(self.select_size)
    }

    pub(crate) fn intrinsic_size(&self) -> Size {
        let mut max_text_width = crate::draw::resources::font::text_backend::estimate_text_metrics(
            &self.placeholder,
            f32::INFINITY,
            self.visual.intrinsic_text_size(),
        )
        .max_line_width;
        for option in &self.options {
            max_text_width = max_text_width.max(
                crate::draw::resources::font::text_backend::estimate_text_metrics(
                    // 固有宽度由实际显示文案决定，而不是内部稳定值。
                    &option.label,
                    f32::INFINITY,
                    self.visual.intrinsic_text_size(),
                )
                .max_line_width,
            );
        }
        for group in &self.optgroups {
            max_text_width = max_text_width.max(
                crate::draw::resources::font::text_backend::estimate_text_metrics(
                    &group.label,
                    f32::INFINITY,
                    self.visual.intrinsic_group_text_size(),
                )
                .max_line_width,
            );
            for option in &group.options {
                max_text_width = max_text_width.max(
                    crate::draw::resources::font::text_backend::estimate_text_metrics(
                        option,
                        f32::INFINITY,
                        self.visual.intrinsic_text_size(),
                    )
                    .max_line_width,
                );
            }
        }
        Size::new(
            (max_text_width + self.visual.layout.intrinsic_extra_width)
                .max(self.visual.layout.natural_min_width),
            self.control_height(),
        )
    }

    pub(crate) fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    pub(crate) fn refresh_search_results(&mut self) {
        self.hovered_option = None;
        self.dropdown_scroll.set_scroll_offset(0.0);
        self.highlighted_option = self.visible_option_indices().first().copied();
        if !self.open {
            self.open();
        } else {
            self.transition_dirty = true;
        }
    }

    pub(crate) fn selected_multi_payload(&self) -> String {
        self.selected_multi
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub(crate) fn sync_bound_selection(&mut self) {
        let Some(binding) = self.value_binding.as_ref() else {
            return;
        };
        let multiple = binding.multiple;
        let selected = {
            let options = self.all_options();
            (binding.read)(&options)
        };
        if multiple {
            self.selected_multi = selected;
        } else {
            self.selected = selected.first().copied().unwrap_or(usize::MAX);
        }
    }

    pub(crate) fn capture_bound_value_dependency(&self) {
        if let Some(binding) = self.value_binding.as_ref() {
            (binding.capture)();
        }
    }

    pub(crate) fn write_bound_selection(&self) {
        let Some(binding) = self.value_binding.as_ref() else {
            return;
        };
        let options = self.all_options();
        if binding.multiple {
            (binding.write)(&options, &self.selected_multi);
        } else {
            (binding.write)(&options, &[self.selected]);
        }
    }

    pub(crate) fn select_single(&mut self, index: usize) {
        self.highlighted_option = Some(index);
        if self.selected == index {
            return;
        }
        self.selected = index;
        self.write_bound_selection();
        self.pending_change.replace(Some(index.to_string()));
    }

    pub(crate) fn publish_multi_change(&self) {
        self.write_bound_selection();
        self.pending_change
            .replace(Some(self.selected_multi_payload()));
    }

    pub(crate) fn move_highlight(&mut self, forward: bool) {
        let visible = self.visible_option_indices();
        if visible.is_empty() {
            self.highlighted_option = None;
            return;
        }
        let current = self
            .highlighted_option
            .and_then(|index| visible.iter().position(|visible| *visible == index));
        let position = match (current, forward) {
            (Some(position), true) => (position + 1).min(visible.len() - 1),
            (Some(position), false) => position.saturating_sub(1),
            (None, true) => 0,
            (None, false) => visible.len() - 1,
        };
        let option_index = visible[position];
        self.highlighted_option = Some(option_index);
        self.reveal_option(option_index);
    }

    pub(crate) fn toggle_highlighted_multi(&mut self) {
        let Some(option_index) = self
            .highlighted_option
            .or_else(|| self.visible_option_indices().first().copied())
        else {
            return;
        };
        if let Some(position) = self
            .selected_multi
            .iter()
            .position(|selected| *selected == option_index)
        {
            self.selected_multi.remove(position);
        } else {
            self.selected_multi.push(option_index);
        }
        self.highlighted_option = Some(option_index);
        self.publish_multi_change();
    }

    pub(crate) fn reveal_option(&mut self, option_index: usize) {
        let visible_rows = self.visible_rows();
        let Some(row_index) = visible_rows
            .iter()
            .position(|row| matches!(row, VisibleRow::Option(index) if *index == option_index))
        else {
            return;
        };
        let row_count = self.dropdown_row_count();
        // 键盘滚动使用受当前表面缩高后的实际视口。
        let viewport_height = self.effective_dropdown_viewport_height(row_count);
        let old_offset = self.dropdown_scroll.scroll_offset();
        let row_top = row_index as f32 * self.visual.layout.row_height;
        let row_bottom = row_top + self.visual.layout.row_height;
        let new_offset = if row_top < old_offset {
            row_top
        } else if row_bottom > old_offset + viewport_height {
            row_bottom - viewport_height
        } else {
            old_offset
        };
        self.dropdown_scroll.set_scroll_offset(new_offset);
        self.dropdown_scroll.clamp_to_content(
            row_count,
            self.visual.layout.row_height,
            viewport_height,
        );
        let applied = self.dropdown_scroll.scroll_offset() - old_offset;
        if applied.abs() > 0.01 {
            self.push_scroll_delta(0.0, applied);
        }
    }

    // 解析当前状态可能覆盖的保守弹层脏区。
    pub(crate) fn dropdown_damage_rect(&self, frame: Rect, surface: Rect) -> Rect {
        // 先解析并缓存过滤后当前实际可见弹层。
        let current = self.remember_dropdown_rect(frame, surface, self.dropdown_row_count());
        // 计算包含过滤前后行数的保守自然高度。
        let height = self.dropdown_viewport_height(self.dropdown_damage_row_count());
        // 使用同一表面解析器生成受约束矩形。
        let damage = resolve_select_popup_rect(frame, self.control_height(), height, surface);
        // 当实际弹层与保守弹层翻转方向不同时同时覆盖两侧。
        current.union(&damage)
    }

    // 解析并缓存当前实际选择弹层矩形。
    pub(crate) fn remember_dropdown_rect(
        // 借用组件状态。
        &self,
        // 接收控件绝对 frame。
        frame: Rect,
        // 接收当前逻辑表面。
        surface: Rect,
        // 接收当前实际行数。
        row_count: usize,
        // 返回相对控件原点的最终弹层矩形。
    ) -> Rect {
        // 归一化控件 frame 供控制区缓存复用。
        let frame = normalize_select_rect(frame);
        // 归一化当前逻辑表面。
        let surface = normalize_select_rect(surface);
        // 将实际控件高度限制在 frame 内。
        let control_height = self.control_height().min(frame.h).max(0.0);
        // 更新本地控制区，保证事件路径不依赖随后绘制。
        self.control_rect
            // 控制区事件坐标以组件原点为基准。
            .set(Rect::new(0.0, 0.0, frame.w, control_height));
        // 计算当前行集合的自然弹层高度。
        let popup_height = self.dropdown_viewport_height(row_count);
        // 通过共享解析器得到最终相对矩形。
        let popup = resolve_select_popup_rect(frame, control_height, popup_height, surface);
        // 缓存当前表面供 dirty、命中和旧登记入口复用。
        self.surface_rect.set(Some(surface));
        // 缓存最终弹层供事件和自定义子树复用。
        self.dropdown_rect.set(popup);
        // 返回同一最终矩形。
        popup
    }

    // 返回最近记录的表面，首次登记前使用有限回退。
    pub(crate) fn surface_or_fallback(&self, frame: Rect) -> Rect {
        // 优先读取布局、绘制或显式登记记录的真实表面。
        self.surface_rect
            // 读取可复制的可选表面。
            .get()
            // 首次使用时按最大可能弹层高度构造有限表面。
            .unwrap_or_else(|| {
                // 计算过滤前后最大行数的自然高度。
                let popup_height = self.dropdown_viewport_height(self.dropdown_damage_row_count());
                // 构造上下均可容纳弹层的有限回退。
                select_fallback_surface(frame, popup_height)
            })
    }

    // 从组件树根布局 frame 读取同帧逻辑表面。
    pub(crate) fn surface_from_tree(&self, frame: Rect, tree: &WidgetTree) -> Rect {
        // 优先使用当前组件树根节点的最新布局结果。
        tree.root()
            // 将根节点尺寸转换为窗口原点表面。
            .map(|root| {
                // 读取根布局 frame。
                let root_frame = root.frame();
                // 构造并归一化逻辑表面。
                normalize_select_rect(Rect::new(0.0, 0.0, root_frame.w, root_frame.h))
            })
            // 无根节点时使用有限回退。
            .unwrap_or_else(|| self.surface_or_fallback(frame))
    }

    // 返回滚动、键盘与物化范围应使用的实际弹层高度。
    pub(crate) fn effective_dropdown_viewport_height(&self, row_count: usize) -> f32 {
        // 读取最近解析的受表面约束高度。
        let resolved = self.dropdown_rect.get().h;
        // 已有正高度时直接复用实际视口。
        if resolved > 0.0 {
            // 防止旧缓存超过当前行集合的自然高度。
            resolved.min(self.dropdown_viewport_height(row_count))
        } else {
            // 首次解析前使用自然视口高度。
            self.dropdown_viewport_height(row_count)
        }
    }

    pub(crate) fn custom_option_indices(&self) -> Vec<usize> {
        if !self.custom_option_views || !self.is_present() || self.loading {
            return Vec::new();
        }
        let rows = self.visible_rows();
        let row_count = self.dropdown_row_count();
        // 自定义选项物化使用受当前表面缩高后的实际视口。
        let viewport_height = self.effective_dropdown_viewport_height(row_count);
        let (start, end) = self.dropdown_scroll.scroll_range(
            row_count,
            self.visual.layout.row_height,
            viewport_height,
        );
        rows[start.min(rows.len())..end.min(rows.len())]
            .iter()
            .filter_map(|row| match row {
                VisibleRow::Option(index) => Some(*index),
                VisibleRow::Group(_) => None,
            })
            .collect()
    }

    pub(crate) fn custom_option_labels(&self, indices: &[usize]) -> Vec<String> {
        indices
            .iter()
            .filter_map(|index| self.option_label(*index).map(str::to_owned))
            .collect()
    }

    pub(crate) fn needs_custom_option_refresh(
        &self,
        indices: &[usize],
        child_count: usize,
    ) -> bool {
        self.materialized_custom_options.borrow().as_slice() != indices
            || child_count != indices.len()
    }

    pub(crate) fn mark_custom_options_materialized(&self, indices: Vec<usize>) {
        *self.materialized_custom_options.borrow_mut() = indices;
    }

    pub(crate) fn invalidate_custom_option_materialization(&self) {
        self.materialized_custom_options.borrow_mut().clear();
    }
}
