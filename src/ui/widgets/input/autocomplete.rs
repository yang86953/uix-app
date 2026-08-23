//! AutoComplete widget — 自动完成输入框，Ant Design 风格。
//!
//! 输入时弹出匹配选项列表，支持键盘导航选择。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::animation::{TransitionPlayer, presets};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入自动完成文本的受控状态句柄。
use crate::ui::reactive::state::State;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, WidgetId,
    WidgetTree,
};
use std::cell::{Cell, RefCell};

// 将表面约束与坐标转换隔离到私有几何模块。
mod geometry;
// 将弹层缓存与实际视口方法隔离到私有实现模块。
mod methods;

// 复用所有 AutoComplete 消费端共享的最终几何函数。
use geometry::{
    // 将相对弹层转换为窗口绝对坐标。
    absolute_autocomplete_popup_rect,
    // 合并触发器、弹层与当前表面。
    autocomplete_surface_rect,
};

const CONTROL_HEIGHT: f32 = 32.0;
const ROW_HEIGHT: f32 = 28.0;
const MAX_POPUP_HEIGHT: f32 = 280.0;
const MIN_POPUP_WIDTH: f32 = 200.0;
const FONT_SIZE: f32 = 13.0;

// AutoComplete — 自动完成输入框。
widget! {
    /// 拥有输入文本、过滤候选与弹层交互状态的自动完成组件。
    pub struct AutoComplete {
        placeholder: String,
        value: String,
        // 保存外部输入文本的受控绑定。
        value_binding: Option<State<String>>,
        options: Vec<String>,
        filtered: Vec<String>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        focus: bool,
        hovered: bool,
        hovered_option: Option<usize>,
        selected_idx: usize,
        cursor_char: usize,
        cursor_rect: Cell<Rect>,
        glyph_xs: RefCell<Vec<f32>>,
        text_scroll_x: Cell<f32>,
        pending_change: RefCell<Option<String>>,
        dropdown_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
        last_frame: Cell<Option<Rect>>,
        // 缓存相对触发器原点的最终弹层矩形。
        popup_rect: Cell<Rect>,
        // 记录弹层缓存对应的显示行数。
        popup_row_count: Cell<usize>,
        // 缓存最近登记或绘制使用的逻辑表面。
        surface_rect: Cell<Option<Rect>>,
        // 缓存最近登记时的绝对触发器锚点。
        popup_anchor_frame: Cell<Option<Rect>>,
        // 累计当前呈现周期内新旧绝对弹层脏区。
        popup_damage_rect: Cell<Rect>,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { true }

    text_input_cursor_rect => (&self) -> Rect { self.cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        // 交互前接收外部状态的最新文本。
        self.sync_bound_value();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if point_in_half_open_rect(self.interaction_frame(), *pos) {
                    self.focus = true;
                    self.set_cursor_from_x(pos.x);
                    self.open();
                    return EventResult::Handled;
                }
                if self.open && point_in_half_open_rect(self.popup_local_rect(), *pos) {
                    if let Some(index) = self.dropdown_row_at(*pos) {
                        self.hovered_option = Some(index);
                        if self.commit_index(index) {
                            return EventResult::Handled;
                        }
                    } else {
                        return EventResult::Handled;
                    }
                }
                self.close();
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let next_hovered = point_in_half_open_rect(self.interaction_frame(), *pos);
                let next_option = if self.open {
                    self.dropdown_row_at(*pos)
                } else {
                    None
                };
                if self.hovered != next_hovered || self.hovered_option != next_option {
                    self.hovered = next_hovered;
                    self.hovered_option = next_option;
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerEnter => {
                if self.hovered {
                    EventResult::NotHandled
                } else {
                    self.hovered = true;
                    EventResult::Handled
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered || self.hovered_option.is_some();
                self.hovered = false;
                self.hovered_option = None;
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => {
                self.focus = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focus = false;
                self.close();
                EventResult::Handled
            }
            SystemEvent::Wheel { delta, pos } => {
                if self.open && point_in_half_open_rect(self.popup_local_rect(), *pos) {
                    let row_count = self.filtered.len();
                    let dy = self.dropdown_scroll.scroll_by_wheel(
                        delta.y,
                        row_count,
                        ROW_HEIGHT,
                        // 滚轮范围使用表面约束后的实际视口。
                        self.effective_popup_viewport_height(row_count),
                    );
                    if dy.abs() > 0.01 {
                        self.push_scroll_delta(0.0, dy);
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Down => {
                        if !self.open {
                            self.open();
                        } else if !self.filtered.is_empty() {
                            self.selected_idx = (self.selected_idx + 1) % self.filtered.len();
                            self.reveal_selected();
                        }
                    }
                    KeyCode::Up if self.open => {
                        if !self.filtered.is_empty() {
                            self.selected_idx =
                                (self.selected_idx + self.filtered.len() - 1) % self.filtered.len();
                            self.reveal_selected();
                        }
                    }
                    KeyCode::Enter if self.open => {
                        if !self.commit_index(self.selected_idx) {
                            self.close();
                        }
                    }
                    KeyCode::Escape => { self.close(); }
                    KeyCode::Backspace => {
                        if !self.delete_previous_char() {
                            return EventResult::NotHandled;
                        }
                    }
                    KeyCode::Delete => {
                        if !self.delete_next_char() {
                            return EventResult::NotHandled;
                        }
                    }
                    KeyCode::Left => {
                        self.cursor_char = self.cursor_char.saturating_sub(1);
                    }
                    KeyCode::Right => {
                        self.cursor_char =
                            (self.cursor_char + 1).min(self.value.chars().count());
                    }
                    KeyCode::Home => { self.cursor_char = 0; }
                    KeyCode::End => { self.cursor_char = self.value.chars().count(); }
                    _ => return EventResult::NotHandled,
                }
                EventResult::Handled
            }
            SystemEvent::TextInput { text } | SystemEvent::Paste { text } => {
                if self.insert_text(text) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            // 读取最近登记或绘制记录的当前逻辑表面。
            let surface = self.surface_or_fallback(frame, self.filtered.len());
            // 命中只使用当前实际候选行数解析弹层。
            let popup = self.remember_popup_rect(frame, surface, self.filtered.len());
            // 将相对弹层转换为窗口绝对坐标。
            let popup = absolute_autocomplete_popup_rect(frame, popup);
            // 触发器与实际弹层命中框共同收敛到当前表面。
            autocomplete_surface_rect(frame, popup, surface)
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w.max(0.0), frame.h.max(0.0))));
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_quaternary();
        let scale = (frame.h / CONTROL_HEIGHT).clamp(0.0, 1.0);
        let font_size = FONT_SIZE * scale;
        let padding = 10.0 * scale;
        let input_rect = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        let radius = Some(Radius::uniform(
            (ctx.tokens().border_radius_sm() * scale).min(input_rect.h * 0.5),
        ));
        let border_color = if self.focus {
            primary
        } else if self.hovered {
            ctx.tokens().color_primary_hover()
        } else {
            border
        };
        ctx.push_clip(input_rect);
        ctx.fill_rect(input_rect, bg, radius);
        ctx.stroke_rect(
            input_rect,
            border_color,
            if self.focus { 2.0 } else { 1.0 },
            radius,
        );

        let text_area = Rect::new(
            input_rect.x + padding,
            input_rect.y,
            (input_rect.w - padding * 2.0).max(0.0),
            input_rect.h,
        );
        if font_size > 0.0 && text_area.w > 0.0 {
            let mut glyph_xs = Vec::with_capacity(self.value.chars().count() + 1);
            let mut prefix = String::new();
            glyph_xs.push(0.0);
            for ch in self.value.chars() {
                prefix.push(ch);
                glyph_xs.push(ctx.measure_text(&prefix, font_size).w);
            }
            let cursor_index = self.cursor_char.min(glyph_xs.len().saturating_sub(1));
            let total_width = glyph_xs.last().copied().unwrap_or(0.0);
            let cursor_offset = glyph_xs.get(cursor_index).copied().unwrap_or(0.0);
            let max_scroll = (total_width - text_area.w).max(0.0);
            let mut scroll = self.text_scroll_x.get().clamp(0.0, max_scroll);
            if cursor_offset < scroll {
                scroll = cursor_offset;
            } else if cursor_offset > scroll + text_area.w {
                scroll = cursor_offset - text_area.w;
            }
            scroll = scroll.clamp(0.0, max_scroll);
            self.text_scroll_x.set(scroll);
            *self.glyph_xs.borrow_mut() = glyph_xs;

            let display = if self.value.is_empty() {
                &self.placeholder
            } else {
                &self.value
            };
            let display_color = if self.value.is_empty() {
                text_tertiary
            } else {
                text
            };
            let draw_x = if self.value.is_empty() {
                text_area.x
            } else {
                text_area.x - scroll
            };
            let draw_y = ctx.visual_center_y(text_area, font_size);
            ctx.push_clip(text_area);
            ctx.draw_text(display, Point::new(draw_x, draw_y), display_color, font_size);

            let caret_h = (18.0 * scale).min(text_area.h);
            let caret_x = (text_area.x + cursor_offset - scroll)
                .clamp(text_area.x, text_area.x + text_area.w);
            let caret_y = text_area.y + (text_area.h - caret_h) * 0.5;
            let cursor_rect = Rect::new(caret_x, caret_y, 1.0, caret_h);
            self.cursor_rect.set(cursor_rect);
            if self.focus {
                ctx.fill_rect(cursor_rect, primary, None);
            }
            ctx.pop_clip();
        } else {
            self.glyph_xs.replace(vec![0.0]);
            self.text_scroll_x.set(0.0);
            self.cursor_rect
                .set(Rect::new(text_area.x, text_area.y, 0.0, text_area.h));
        }
        ctx.pop_clip();

        if self.is_present() {
            // 将逻辑表面映射到当前组件坐标，兼容被提升的滚动浮层。
            let surface = ctx.logical_surface_rect();
            // 在绘制弹层前解析并缓存同帧最终几何。
            let popup = self.remember_popup_rect(frame, surface, self.filtered.len());
            // 将相对触发器缓存转换为窗口绝对弹层矩形。
            let menu_rect = absolute_autocomplete_popup_rect(frame, popup);
            // 空表面不生成可见自动完成弹层。
            if menu_rect.w <= 0.0 || menu_rect.h <= 0.0 {
                // 保留输入框绘制结果并跳过弹层。
                return;
            }
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let fill = fade_color(ctx.tokens().color_fill_tertiary(), opacity);
            let bg_elev = fade_color(ctx.tokens().color_bg_elevated(), opacity);
            let border = fade_color(border, opacity);
            let text = fade_color(text, opacity);
            let text_secondary = fade_color(text_secondary, opacity);
            let panel_radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
            // 将整个自动完成弹层裁剪到当前逻辑表面。
            ctx.push_clip(surface);
            // 再按最终弹层矩形裁剪候选行与边框。
            ctx.push_clip(menu_rect);
            ctx.fill_rect(menu_rect, bg_elev, panel_radius);
            ctx.stroke_rect(menu_rect, border, 1.0, panel_radius);

            if self.filtered.is_empty() {
                let no_data_area = Rect::new(
                    menu_rect.x + 10.0,
                    menu_rect.y,
                    (menu_rect.w - 20.0).max(0.0),
                    menu_rect.h,
                );
                let y = ctx.visual_center_y(no_data_area, FONT_SIZE);
                ctx.push_clip(no_data_area);
                ctx.draw_text(
                    crate::ui::widget_runtime::locale::use_locale().no_data,
                    Point::new(no_data_area.x, y),
                    text_secondary,
                    FONT_SIZE,
                );
                ctx.pop_clip();
                ctx.pop_clip();
                // 恢复弹层外层的逻辑表面裁剪。
                ctx.pop_clip();
                return;
            }

            let scroll_offset = self.dropdown_scroll.scroll_offset();
            let (start, end) = self.dropdown_scroll.scroll_range(
                self.filtered.len(),
                ROW_HEIGHT,
                menu_rect.h,
            );
            for (index, option) in self
                .filtered
                .iter()
                .enumerate()
                .take(end)
                .skip(start)
            {
                let option_y = menu_rect.y + index as f32 * ROW_HEIGHT - scroll_offset;
                if option_y + ROW_HEIGHT <= menu_rect.y
                    || option_y >= menu_rect.y + menu_rect.h
                {
                    continue;
                }
                let item_rect = Rect::new(menu_rect.x, option_y, menu_rect.w, ROW_HEIGHT);
                if index == self.selected_idx || self.hovered_option == Some(index) {
                    ctx.fill_rect(item_rect, fill, None);
                }
                let text_area = Rect::new(
                    item_rect.x + 10.0,
                    item_rect.y,
                    (item_rect.w - 20.0).max(0.0),
                    item_rect.h,
                );
                let y = ctx.visual_center_y(text_area, FONT_SIZE);
                ctx.push_clip(text_area);
                ctx.draw_text(option, Point::new(text_area.x, y), text, FONT_SIZE);
                ctx.pop_clip();
            }
            ctx.pop_clip();
            // 恢复弹层外层的逻辑表面裁剪。
            ctx.pop_clip();
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 读取当前候选行数供表面回退与脏区解析共用。
        let row_count = self.filtered.len();
        // 读取最近登记或绘制记录的当前逻辑表面。
        let surface = self.surface_or_fallback(frame, row_count);
        // 合并当前与本次呈现周期历史弹层脏区。
        let popup = self.damage_popup_rect(frame, surface, row_count);
        // 将输入框和弹层脏区限制在当前表面内。
        autocomplete_surface_rect(frame, popup, surface)
    }

    overlay_entry => (&self, id: WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.is_present().then(|| {
            // 读取当前候选行数供表面回退与弹层解析共用。
            let row_count = self.filtered.len();
            // 读取最近记录的表面或首次有限回退。
            let surface = self.surface_or_fallback(frame, row_count);
            // 解析并缓存当前实际弹层。
            let popup = self.remember_popup_rect(frame, surface, row_count);
            // 将相对弹层转换为窗口绝对坐标。
            let bounds = absolute_autocomplete_popup_rect(frame, popup);
            // 创建只覆盖实际弹层的浮层登记。
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                // 登记边界与绘制、命中共用同一矩形。
                .bounds(bounds)
                .z_index(900)
        })
    }

    // 使用组件树提供的同帧表面创建自动完成弹层登记。
    overlay_entry_for_surface => (&self, id: WidgetId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 在旧登记入口执行前刷新表面与实际弹层缓存。
        self.remember_popup_rect(frame, surface, self.filtered.len());
        // 复用统一的自动完成弹层登记逻辑。
        self.overlay_entry(id, frame)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() {
            self.transition_dirty = false;
            return false;
        }

        if self.transition.finished {
            if self.closing {
                self.closing = false;
                self.hovered_option = None;
                self.dropdown_scroll.set_scroll_offset(0.0);
            }
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.closing = false;
            self.hovered_option = None;
            self.dropdown_scroll.set_scroll_offset(0.0);
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            // 读取当前候选行数供表面回退与脏区解析共用。
            let row_count = self.filtered.len();
            // 动画脏区使用最近记录的当前逻辑表面。
            let surface = self.surface_or_fallback(frame, row_count);
            // 动画期间同时覆盖当前与历史最终弹层。
            let popup = self.damage_popup_rect(frame, surface, row_count);
            // 将动画脏区限制在当前表面。
            autocomplete_surface_rect(frame, popup, surface)
        } else {
            Rect::zero()
        }
    }
}

impl AutoComplete {
    fn intrinsic_size(&self) -> Size {
        Size::new(200.0, 32.0)
    }

    /// 创建空文本、空候选且弹层关闭的自动完成输入框。
    pub fn new() -> Self {
        Self {
            placeholder: String::new(),
            value: String::new(),
            // 默认保持非受控输入模式。
            value_binding: None,
            options: Vec::new(),
            filtered: Vec::new(),
            open: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            focus: false,
            hovered: false,
            hovered_option: None,
            selected_idx: 0,
            cursor_char: 0,
            cursor_rect: Cell::new(Rect::zero()),
            glyph_xs: RefCell::new(vec![0.0]),
            text_scroll_x: Cell::new(0.0),
            pending_change: RefCell::new(None),
            dropdown_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            last_frame: Cell::new(None),
            // 首次表面解析前弹层缓存为空。
            popup_rect: Cell::new(Rect::zero()),
            // 零行标记尚未生成有效弹层缓存。
            popup_row_count: Cell::new(0),
            // 首次登记或绘制前尚未取得当前逻辑表面。
            surface_rect: Cell::new(None),
            // 首次登记前尚未取得绝对触发器锚点。
            popup_anchor_frame: Cell::new(None),
            // 首次呈现前没有历史弹层脏区。
            popup_damage_rect: Cell::new(Rect::zero()),
        }
    }
    /// 设置输入框为空时显示的占位文本。
    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = p.to_string();
        self
    }

    /// 设置参与本地包含匹配的候选文本。
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect();
        self
    }

    /// 将输入文本与选中结果绑定到外部字符串状态。
    pub fn bind_value(mut self, state: &State<String>) -> Self {
        // 克隆轻量状态句柄供后续交互提交使用。
        self.value_binding = Some(state.clone());
        // 构造时立即同步文本并登记响应式依赖。
        self.sync_bound_value();
        self
    }
    /// 返回当前输入文本。
    pub fn value(&self) -> &str {
        &self.value
    }

    /// 替换当前输入文本，并在交互期间重新过滤候选。
    pub fn set_value(&mut self, v: &str) {
        self.value = v.to_string();
        self.cursor_char = self.value.chars().count();
        self.text_scroll_x.set(0.0);
        if self.is_present() || self.focus {
            self.filter();
        }
    }
    // 从外部受控状态同步当前输入文本。
    fn sync_bound_value(&mut self) {
        // 非受控模式保留组件内部文本。
        let Some(value) = self.value_binding.as_ref().map(State::get) else {
            return;
        };
        // 仅在外部值变化时重置光标与过滤结果。
        if value != self.value {
            self.set_value(&value);
        }
    }
    fn filter(&mut self) {
        if self.value.is_empty() {
            self.filtered = self.options.clone();
        } else {
            let query = self.value.to_lowercase();
            self.filtered = self
                .options
                .iter()
                .filter(|option| option.to_lowercase().contains(&query))
                .cloned()
                .collect();
        }
        self.selected_idx = 0;
        self.hovered_option = None;
        self.dropdown_scroll.set_scroll_offset(0.0);
        self.scroll_delta_strip.set((0.0, 0.0));
    }

    // 测试目标保留过滤选项观测入口，供自动完成匹配测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn filtered_options(&self) -> &[String] {
        &self.filtered
    }

    /// 判断弹层是否处于接受交互的打开状态。
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 判断弹层是否打开或仍在播放关闭动画。
    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    /// 打开弹层、过滤候选并启动进入动画。
    pub fn open(&mut self) {
        // 记录本次调用是否开始新的呈现周期。
        let starts_presentation = !self.is_present();
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
        self.filter();
        self.open = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    /// 关闭弹层并在需要时启动退出动画。
    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }

        self.open = false;
        self.closing = true;
        self.hovered_option = None;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::AutoComplete {
            placeholder: self.placeholder.clone(),
            options: self.options.clone(),
            value: self.value.clone(),
            open: self.open,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // 受控实例以新声明中的状态值为权威。
        let controlled_value = next.value_binding.as_ref().map(|_| next.value.clone());
        let options_changed = self.options != next.options;
        self.placeholder = next.placeholder;
        self.options = next.options;
        // 同步声明式重建携带的状态句柄。
        self.value_binding = next.value_binding;
        // 外部文本变化时更新光标与当前过滤结果。
        if let Some(value) = controlled_value {
            if value != self.value {
                self.set_value(&value);
            }
        }
        self.cursor_char = self.cursor_char.min(self.value.chars().count());
        if (self.is_present() || self.focus) && options_changed {
            self.filter();
        }
    }

    fn interaction_frame(&self) -> Rect {
        self.last_frame.get().unwrap_or_else(|| {
            let size = self.intrinsic_size();
            Rect::new(0.0, 0.0, size.w, size.h)
        })
    }

    fn popup_local_rect(&self) -> Rect {
        // 读取事件路径使用的本地触发器 frame。
        let frame = self.interaction_frame();
        // 返回当前表面解析后的最终本地弹层。
        self.interaction_popup_rect(frame, self.filtered.len())
    }

    fn dropdown_row_at(&self, pos: Point) -> Option<usize> {
        let popup = self.popup_local_rect();
        if !point_in_half_open_rect(popup, pos) {
            return None;
        }
        let local_y = pos.y - popup.y + self.dropdown_scroll.scroll_offset();
        if local_y < 0.0 {
            return None;
        }
        let index = (local_y / ROW_HEIGHT).floor() as usize;
        (index < self.filtered.len()).then_some(index)
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    fn reveal_selected(&mut self) {
        if self.selected_idx >= self.filtered.len() {
            return;
        }
        // 键盘显露使用表面约束后的实际视口高度。
        let viewport_height = self.effective_popup_viewport_height(self.filtered.len());
        let old_offset = self.dropdown_scroll.scroll_offset();
        let row_top = self.selected_idx as f32 * ROW_HEIGHT;
        let row_bottom = row_top + ROW_HEIGHT;
        let new_offset = if row_top < old_offset {
            row_top
        } else if row_bottom > old_offset + viewport_height {
            row_bottom - viewport_height
        } else {
            old_offset
        };
        self.dropdown_scroll.set_scroll_offset(new_offset);
        self.dropdown_scroll
            .clamp_to_content(self.filtered.len(), ROW_HEIGHT, viewport_height);
        let applied = self.dropdown_scroll.scroll_offset() - old_offset;
        if applied.abs() > 0.01 {
            self.push_scroll_delta(0.0, applied);
        }
    }

    fn set_cursor_from_x(&mut self, x: f32) {
        let glyph_xs = self.glyph_xs.borrow();
        if glyph_xs.len() != self.value.chars().count() + 1 {
            self.cursor_char = self.value.chars().count();
            return;
        }
        let scale = (self.interaction_frame().h / CONTROL_HEIGHT).clamp(0.0, 1.0);
        let target = (x - 10.0 * scale + self.text_scroll_x.get()).max(0.0);
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

    fn insert_text(&mut self, text: &str) -> bool {
        if text.is_empty() || text.chars().any(char::is_control) {
            return false;
        }
        let byte_index = byte_index_for_char(&self.value, self.cursor_char);
        self.value.insert_str(byte_index, text);
        self.cursor_char += text.chars().count();
        self.open();
        self.publish_change();
        true
    }

    fn delete_previous_char(&mut self) -> bool {
        if self.cursor_char == 0 || self.value.is_empty() {
            return false;
        }
        let end = byte_index_for_char(&self.value, self.cursor_char);
        let start = byte_index_for_char(&self.value, self.cursor_char - 1);
        self.value.replace_range(start..end, "");
        self.cursor_char -= 1;
        self.open();
        self.publish_change();
        true
    }

    fn delete_next_char(&mut self) -> bool {
        let char_count = self.value.chars().count();
        if self.cursor_char >= char_count {
            return false;
        }
        let start = byte_index_for_char(&self.value, self.cursor_char);
        let end = byte_index_for_char(&self.value, self.cursor_char + 1);
        self.value.replace_range(start..end, "");
        self.open();
        self.publish_change();
        true
    }

    fn commit_index(&mut self, index: usize) -> bool {
        let Some(value) = self.filtered.get(index).cloned() else {
            return false;
        };
        self.value = value;
        self.cursor_char = self.value.chars().count();
        self.text_scroll_x.set(0.0);
        self.publish_change();
        self.close();
        true
    }

    fn publish_change(&self) {
        // 受控模式先把本地编辑结果写回外部状态。
        if let Some(state) = self.value_binding.as_ref() {
            // 避免重复发布相同文本。
            if state.get() != self.value {
                state.set(self.value.clone());
            }
        }
        // 再保留既有语义 Change 事件负载。
        self.pending_change.replace(Some(self.value.clone()));
    }
}

impl Default for AutoComplete {
    fn default() -> Self {
        Self::new()
    }
}

fn byte_index_for_char(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

fn point_in_half_open_rect(rect: Rect, point: Point) -> bool {
    point.x >= rect.x && point.x < rect.x + rect.w && point.y >= rect.y && point.y < rect.y + rect.h
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

// 将 AutoComplete 表面约束契约放在独立测试文件中。
