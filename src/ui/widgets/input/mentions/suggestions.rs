// 拆分自 mentions.rs：候选建议刷新、停止与过滤。
// 引入被扩展的提及组件与字符索引换算。
use super::{Mentions, byte_index_for_char};
// 引入受控文本绑定状态句柄、矩形类型与共享候选引用。
use crate::core::Rect;
use crate::ui::reactive::state::State;
use std::sync::Arc;

// 为 Mentions 提供候选建议生命周期方法。
impl Mentions {
    // 从外部受控状态同步当前完整文本。
    pub(super) fn sync_bound_value(&mut self) {
        // 非受控模式继续保留组件内部文本。
        let Some(value) = self.value_binding.as_ref().map(State::get) else {
            // 没有绑定时无需同步。
            return;
        };
        // 仅在外部文本实际变化时重建派生状态。
        if value != self.value {
            // 统一更新文本、光标与活动查询。
            self.set_value(&value);
        }
    }

    pub(super) fn refresh_suggestion_from_value(&mut self) {
        let cursor_byte = byte_index_for_char(&self.value, self.cursor_char);
        let prefix = &self.value[..cursor_byte];
        let Some(trigger_start) = prefix.rfind(&self.trigger) else {
            self.stop_suggesting();
            return;
        };
        let query_start = trigger_start + self.trigger.len();
        let query = &prefix[query_start..];
        if query.chars().any(char::is_whitespace) {
            self.stop_suggesting();
            return;
        }

        let query = query.to_owned();
        // 记录有效活动查询是否开始新的呈现周期。
        let starts_presentation = !self.suggesting;
        // 只有新呈现周期才丢弃上一周期的弹层历史。
        if starts_presentation {
            // 新呈现周期重新收集弹层脏区。
            self.popup_damage_rect.set(Rect::zero());
            // 新呈现周期等待当前帧重新解析弹层。
            self.popup_row_count.set(0);
            // 丢弃上一呈现周期的相对弹层缓存。
            self.popup_rect.set(Rect::zero());
            // 等待当前帧取得最新逻辑表面。
            self.surface_rect.set(None);
            // 等待当前帧取得最新绝对锚点。
            self.popup_anchor_frame.set(None);
        }
        // 新呈现或光标查询变化都需要刷新候选集合。
        let query_changed = starts_presentation || self.search_text != query;
        self.search_text = query;
        self.suggesting = true;
        if query_changed {
            self.update_filtered();
        }
    }

    pub(super) fn stop_suggesting(&mut self) {
        self.suggesting = false;
        self.search_text.clear();
        // 整体替换为空列表，避免清空与候选共享的底层数据。
        self.filtered = Arc::new(Vec::new());
        self.selected_index = 0;
        self.hovered_option = None;
        self.dropdown_scroll.set_scroll_offset(0.0);
        self.scroll_delta_strip.set((0.0, 0.0));
    }

    pub(super) fn update_filtered(&mut self) {
        if self.search_text.is_empty() {
            // 空查询直接共享候选列表底层数据，避免每键击全量深拷贝。
            self.filtered = Arc::clone(&self.options);
        } else {
            // 查询仅小写化一次，候选匹配复用预存小写副本，逐键击零额外分配。
            let query = self.search_text.to_lowercase();
            self.filtered = Arc::new(
                self.options
                    .iter()
                    .zip(self.lowercase_options.iter())
                    .filter(|(_, lower)| lower.contains(&query))
                    .map(|(option, _)| option.clone())
                    .collect(),
            );
        }
        self.selected_index = 0;
        self.hovered_option = None;
        self.dropdown_scroll.set_scroll_offset(0.0);
        self.scroll_delta_strip.set((0.0, 0.0));
    }

}
