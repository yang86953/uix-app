// 拆分自 mentions.rs：文本编辑、光标定位与候选替换。
// 引入被扩展的提及组件。
use super::Mentions;
// 复用 input 层共享的字符索引换算辅助。
use crate::ui::widgets::input::byte_index_for_char;

// 为 Mentions 提供文本编辑与候选替换方法。
impl Mentions {
    pub(super) fn set_cursor_from_x(&mut self, x: f32) {
        let glyph_xs = self.glyph_xs.borrow();
        if glyph_xs.len() != self.value.chars().count() + 1 {
            self.cursor_char = self.value.chars().count();
            return;
        }
        let scale =
            (self.interaction_frame().h / self.visual.layout.control_height).clamp(0.0, 1.0);
        let target =
            (x - self.visual.layout.horizontal_padding * scale + self.text_scroll_x.get()).max(0.0);
        let mut index = glyph_xs.len().saturating_sub(1);
        for candidate in 0..glyph_xs.len().saturating_sub(1) {
            let midpoint = (glyph_xs[candidate] + glyph_xs[candidate + 1]) * 0.5;
            if target < midpoint {
                index = candidate;
                break;
            }
        }
        self.cursor_char = index;
    }

    pub(super) fn insert_text(&mut self, text: &str) -> bool {
        if text.is_empty() || text.chars().any(char::is_control) {
            return false;
        }
        let byte_index = byte_index_for_char(&self.value, self.cursor_char);
        self.value.insert_str(byte_index, text);
        self.cursor_char += text.chars().count();
        self.refresh_suggestion_from_value();
        self.publish_change();
        true
    }

    pub(super) fn delete_previous_char(&mut self) -> bool {
        if self.cursor_char == 0 || self.value.is_empty() {
            return false;
        }
        let end = byte_index_for_char(&self.value, self.cursor_char);
        let start = byte_index_for_char(&self.value, self.cursor_char - 1);
        self.value.replace_range(start..end, "");
        self.cursor_char -= 1;
        self.refresh_suggestion_from_value();
        self.publish_change();
        true
    }

    pub(super) fn delete_next_char(&mut self) -> bool {
        let char_count = self.value.chars().count();
        if self.cursor_char >= char_count {
            return false;
        }
        let start = byte_index_for_char(&self.value, self.cursor_char);
        let end = byte_index_for_char(&self.value, self.cursor_char + 1);
        self.value.replace_range(start..end, "");
        self.refresh_suggestion_from_value();
        self.publish_change();
        true
    }

    pub(super) fn active_replacement_range(&self) -> Option<(usize, usize)> {
        let cursor_byte = byte_index_for_char(&self.value, self.cursor_char);
        let prefix = &self.value[..cursor_byte];
        let start = prefix.rfind(&self.trigger)?;
        let query_start = start + self.trigger.len();
        if prefix[query_start..].chars().any(char::is_whitespace) {
            return None;
        }

        let mut end = cursor_byte;
        for (offset, ch) in self.value[cursor_byte..].char_indices() {
            end = cursor_byte + offset + ch.len_utf8();
            if ch.is_whitespace() {
                break;
            }
        }
        Some((start, end))
    }

    pub(super) fn select_index(&mut self, index: usize) -> bool {
        let Some(selected) = self.filtered.get(index).cloned() else {
            return false;
        };
        let Some((start, end)) = self.active_replacement_range() else {
            self.stop_suggesting();
            return false;
        };
        let prefix_chars = self.value[..start].chars().count();
        let replacement = format!("{}{} ", self.trigger, selected);
        let replacement_chars = replacement.chars().count();
        self.value.replace_range(start..end, &replacement);
        self.cursor_char = prefix_chars + replacement_chars;
        self.publish_change();
        self.stop_suggesting();
        true
    }
}
