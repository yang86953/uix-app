//! Pagination widget — 分页器，Ant Design 风格。
//!
//! 支持页码切换、上一页/下一页、快速跳转（省略号）、pageSize 切换。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入 current 与 pageSize 的声明式状态句柄。
use crate::ui::State;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, View, ViewNode,
    WidgetId, WidgetTree,
};
use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

mod presentation;
use presentation::*;

// Pagination — 分页器。
widget! {
    /// 拥有页码、每页条数与可选受控状态的分页导航组件。
    pub struct Pagination {
        total: usize,
        page_size: usize,
        // 保存声明端 pageSize 的唯一受控状态来源。
        page_size_binding: Option<State<usize>>,
        current: Cell<usize>,
        // 保存声明端 current 的唯一受控状态来源。
        current_binding: Option<State<usize>>,
        show_size_changer: bool,
        show_total: bool,
        simple: bool,
        show_jumper: bool,
        total_template: Option<Rc<dyn Fn(usize, Range<usize>) -> String>>,
        size: f32, // item size
        page_size_options: Vec<usize>,
        focused: bool,
        jumper_active: bool,
        jumper_buffer: String,
        jumper_cursor_rect: Cell<Rect>,
        pending_change: Cell<Option<PaginationChange>>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static PaginationVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { self.show_jumper && self.jumper_active }

    text_input_cursor_rect => (&self) -> Rect { self.jumper_cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        // 每次处理输入前吸收声明端可能发生的双状态更新。
        self.sync_bound_values();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if pos.y < 0.0 || pos.y > self.size + self.visual.layout.hit_vertical_extra {
                    return EventResult::NotHandled;
                }
                self.handle_pointer_down(pos.x)
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.commit_jumper();
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if self.jumper_active => match key {
                KeyCode::Enter => {
                    self.commit_jumper();
                    EventResult::Handled
                }
                KeyCode::Escape => {
                    self.cancel_jumper();
                    EventResult::Handled
                }
                KeyCode::Backspace => {
                    self.jumper_buffer.pop();
                    EventResult::Handled
                }
                KeyCode::Left
                | KeyCode::Right
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::PageUp
                | KeyCode::PageDown => EventResult::Handled,
                _ => EventResult::NotHandled,
            },
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Left | KeyCode::PageUp => {
                    self.change_page_by(-1);
                    EventResult::Handled
                }
                KeyCode::Right | KeyCode::PageDown => {
                    self.change_page_by(1);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.select_page(1);
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.select_page(self.total_pages().max(1));
                    EventResult::Handled
                }
                KeyCode::Up if self.show_size_changer => {
                    self.cycle_page_size(false);
                    EventResult::Handled
                }
                KeyCode::Down if self.show_size_changer => {
                    self.cycle_page_size(true);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            SystemEvent::TextInput { text } | SystemEvent::Paste { text }
                if self.jumper_active =>
            {
                self.append_jumper_text(text)
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change.take().map(|change| match change {
            PaginationChange::Page(page) => SemanticEvent::change(id, page.to_string()),
            PaginationChange::PageSize(page_size) => {
                SemanticEvent::change(id, format!("page_size={page_size}"))
            }
        })
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 捕获双状态依赖，使外部更新触发声明树重建。
        self.capture_bound_value_dependencies();
        let total_pages = self.total.div_ceil(self.page_size);
        if total_pages == 0 && !self.show_total && !self.show_size_changer {
            return;
        }
        let cur = self.current.get();
        let item_w = self.size;
        let item_h = self.size;
        // 全部控件、辅助文字与输入光标同帧共享一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let layout = &self.visual.layout;
        let typography = &self.visual.typography;
        let radius = Radius::uniform(visual.radius);
        let primary = visual.primary;
        let border = visual.border;
        let text = visual.text;
        let text_sec = visual.text_secondary;
        // 当前页文字反白色：白色 token。
        let white = visual.white;
        let bg = visual.container_background;

        let mut x = frame.x;
        let y = frame.y + (frame.h - item_h) * 0.5;
        let range = self.visible_range(total_pages, cur);
        let prev_disabled = cur <= 1 || total_pages == 0;
        let prev_c = if prev_disabled { text_sec } else { text };
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            self.visual.icons.previous,
            Rect::new(x, y, item_w, item_h),
            prev_c,
            typography.icon,
        );
        x += item_w + layout.gap;

        if self.simple {
            let simple_rect = Rect::new(x, y, item_w * layout.simple_width_factor, item_h);
            ctx.text_center(
                &format!("{cur} / {total_pages}"),
                simple_rect,
                text,
                typography.page,
            );
            x += simple_rect.w + layout.gap;
        } else {
            for &p in &range {
            let label = if p == 0 { "...".to_string() } else { p.to_string() };
            let active = p == cur;
            let btn_rect = Rect::new(x, y, item_w, item_h);
            if active {
                ctx.fill_rect(btn_rect, primary, Some(radius));
                ctx.text_center(&label, btn_rect, white, typography.page);
            } else {
                ctx.fill_rect(btn_rect, bg, Some(radius));
                ctx.stroke_rect(
                    btn_rect,
                    border,
                    self.visual.chrome.border_width,
                    Some(radius),
                );
                ctx.text_center(&label, btn_rect, text, typography.page);
            }
            x += item_w + layout.gap;
            }
        }

        let next_disabled = cur >= total_pages || total_pages == 0;
        let next_c = if next_disabled { text_sec } else { text };
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            self.visual.icons.next,
            Rect::new(x, y, item_w, item_h),
            next_c,
            typography.icon,
        );
        x += item_w;

        if self.show_total {
            let total_rect = Rect::new(
                x + layout.extra_gap,
                y,
                layout.total_width,
                item_h,
            );
            // 分页辅助文本字号：统一使用主题 font_size_sm token。
            let total_y = ctx.visual_center_y(total_rect, visual.small_font_size);
            let total_label = self
                .total_template
                .as_ref()
                .map(|template| template(self.total, self.current_record_range()))
                .unwrap_or_else(|| format!("共 {} 条", self.total));
            ctx.draw_text(
                &total_label,
                Point::new(total_rect.x, total_y),
                text_sec,
                visual.small_font_size,
            );
            x = total_rect.x + total_rect.w;
        }

        if self.show_jumper {
            let jumper_rect = Rect::new(x + layout.extra_gap, y, layout.jumper_width, item_h);
            let label_rect = Rect::new(
                jumper_rect.x,
                jumper_rect.y,
                layout.jumper_label_width,
                jumper_rect.h,
            );
            let input_rect = Rect::new(
                label_rect.x + label_rect.w,
                jumper_rect.y,
                layout.jumper_input_width,
                jumper_rect.h,
            );
            let suffix_rect = Rect::new(
                input_rect.x + input_rect.w + layout.jumper_suffix_gap,
                jumper_rect.y,
                layout.jumper_suffix_width,
                jumper_rect.h,
            );
            let border_color = if self.jumper_active { primary } else { border };
            let label_y = ctx.visual_center_y(label_rect, visual.small_font_size);
            ctx.draw_text(
                "跳至",
                Point::new(label_rect.x, label_y),
                text_sec,
                visual.small_font_size,
            );
            ctx.fill_rect(input_rect, bg, Some(radius));
            ctx.stroke_rect(
                input_rect,
                border_color,
                self.visual.chrome.border_width,
                Some(radius),
            );
            let inactive_text = (!self.jumper_active).then(|| cur.to_string());
            let jumper_text = inactive_text
                .as_deref()
                .unwrap_or(self.jumper_buffer.as_str());
            ctx.text_center(jumper_text, input_rect, text, visual.small_font_size);
            let suffix_y = ctx.visual_center_y(suffix_rect, visual.small_font_size);
            ctx.draw_text(
                "页",
                Point::new(suffix_rect.x, suffix_y),
                text_sec,
                visual.small_font_size,
            );
            let text_width = ctx
                .measure_text(jumper_text, visual.small_font_size)
                .w
                .min(input_rect.w - layout.cursor_horizontal_padding);
            self.jumper_cursor_rect.set(Rect::new(
                input_rect.x + (input_rect.w + text_width) * 0.5,
                input_rect.y + layout.cursor_vertical_inset,
                layout.cursor_width,
                (input_rect.h - layout.cursor_vertical_inset * 2.0).max(0.0),
            ));
            x = jumper_rect.x + jumper_rect.w;
        }

        if self.show_size_changer && !self.page_size_options.is_empty() {
            let changer_text = format!("{} 条/页", self.page_size);
            let changer_rect = Rect::new(
                x + layout.extra_gap,
                y,
                layout.size_changer_width,
                item_h,
            );
            ctx.stroke_rect(
                changer_rect,
                border,
                self.visual.chrome.border_width,
                Some(radius),
            );
            let cy = ctx.visual_center_y(changer_rect, visual.small_font_size);
            ctx.draw_text(
                &changer_text,
                Point::new(changer_rect.x + layout.changer_text_start, cy),
                text,
                visual.small_font_size,
            );
        }

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                frame,
                primary,
                self.visual.chrome.focus_width,
                Some(radius),
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaginationChange {
    Page(usize),
    PageSize(usize),
}

impl Pagination {
    /// 创建总记录数与每页条数固定、当前页为第一页的非受控分页器。
    pub fn new(total: usize, page_size: usize) -> Self {
        let visual = PAGINATION_VISUAL_REF;
        Self {
            total,
            page_size: page_size.max(1),
            // 默认构造保持 pageSize 非受控。
            page_size_binding: None,
            current: Cell::new(1),
            // 默认构造保持 current 非受控。
            current_binding: None,
            show_size_changer: visual.defaults.show_size_changer,
            show_total: visual.defaults.show_total,
            simple: visual.defaults.simple,
            show_jumper: visual.defaults.show_jumper,
            total_template: None,
            size: visual.layout.default_item_size,
            page_size_options: vec![10, 20, 50, 100],
            focused: false,
            jumper_active: false,
            jumper_buffer: String::new(),
            jumper_cursor_rect: Cell::new(Rect::zero()),
            pending_change: Cell::new(None),
            visual,
        }
    }
    /// 设置非受控当前页初始值，并按当前总页数归一化。
    pub fn current(mut self, v: usize) -> Self {
        // 显式数值配置切回非受控模式。
        self.current_binding = None;
        // 按当前 total 与 pageSize 归一化页码。
        self.current.set(self.clamp_current(v));
        // 返回完成配置的分页器。
        self
    }
    /// 将当前页双向绑定到声明端状态，并把越界值归一化写回。
    pub fn current_state(mut self, state: &State<usize>) -> Self {
        // 克隆轻量句柄，不复制或夺取应用状态。
        self.current_binding = Some(state.clone());
        // 立即同步并归一化声明端双状态。
        self.sync_bound_values();
        // 返回受控分页器。
        self
    }
    /// 返回已按总页数归一化的当前页码。
    pub fn get_current(&self) -> usize {
        self.current.get()
    }
    /// 返回已归一化为至少一条的当前每页条数。
    pub fn get_page_size(&self) -> usize {
        self.page_size
    }
    /// 设置非受控每页条数，并重新限制当前页。
    pub fn page_size(mut self, v: usize) -> Self {
        // 显式数值配置切回非受控模式。
        self.page_size_binding = None;
        // pageSize 的最小合法值为一。
        self.page_size = v.max(1);
        // pageSize 改变后重新限制 current。
        self.current.set(self.clamp_current(self.current.get()));
        // 若 current 仍受控，则同步写回可能发生的页码收敛。
        self.write_current_bound(self.current.get());
        // 返回完成配置的分页器。
        self
    }
    /// 将每页条数双向绑定到声明端状态，并把零值归一化写回。
    pub fn page_size_state(mut self, state: &State<usize>) -> Self {
        // 克隆轻量句柄，不复制或夺取应用状态。
        self.page_size_binding = Some(state.clone());
        // 立即同步 pageSize，并在必要时收敛 current。
        self.sync_bound_values();
        // 返回受控分页器。
        self
    }
    /// 设置是否显示总记录数与当前记录范围。
    pub fn show_total(mut self, v: bool) -> Self {
        self.show_total = v;
        self
    }
    /// 设置单个页码、上一页与下一页控件的方形边长。
    pub fn item_size(mut self, v: f32) -> Self {
        self.size = v;
        self
    }
    /// 命令式设置当前页并同步受控状态，但不产生用户变更事件。
    pub fn set_current(&self, v: usize) {
        // 归一化命令式页码。
        let current = self.clamp_current(v);
        // 更新组件本地镜像。
        self.current.set(current);
        // 受控模式同步写回应用状态，但不伪造用户事件。
        self.write_current_bound(current);
    }
    /// 返回按当前每页条数向上取整的总页数。
    pub fn total_pages(&self) -> usize {
        self.total.div_ceil(self.page_size)
    }
    /// 设置是否显示每页条数切换入口。
    pub fn show_size_changer(mut self, v: bool) -> Self {
        self.show_size_changer = v;
        self
    }
    /// 设置每页条数候选项，按输入顺序归一化零值并去重。
    pub fn page_size_options(mut self, opts: Vec<usize>) -> Self {
        self.page_size_options.clear();
        for option in opts.into_iter().map(|option| option.max(1)) {
            if !self.page_size_options.contains(&option) {
                self.page_size_options.push(option);
            }
        }
        self
    }

    /// 设置是否仅显示上一页、当前页摘要与下一页。
    pub fn simple(mut self, simple: bool) -> Self {
        self.simple = simple;
        self
    }

    /// 设置是否显示可输入目标页码的快速跳转框。
    pub fn show_jumper(mut self, show: bool) -> Self {
        self.show_jumper = show;
        self
    }

    /// 设置总数区域的格式化函数，参数为总记录数和当前记录范围。
    pub fn total_template<F>(mut self, template: F) -> Self
    where
        F: Fn(usize, std::ops::Range<usize>) -> String + 'static,
    {
        self.total_template = Some(Rc::new(template));
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // 保存非受控模式下应继续沿用的交互页码。
        let previous_current = self.current.get();
        // 受控声明携带从最新 State 同步出的 current 镜像。
        let controlled_current = next.current_binding.as_ref().map(|_| next.current.get());
        // 采用新声明的总条数。
        self.total = next.total;
        // pageSize 的声明配置或受控镜像都由 next 提供。
        self.page_size = next.page_size.max(1);
        // 接管新声明的 pageSize 状态句柄。
        self.page_size_binding = next.page_size_binding;
        // 接管新声明的 current 状态句柄。
        self.current_binding = next.current_binding;
        // 受控模式服从 State；非受控模式保留旧交互页码。
        let current = controlled_current.unwrap_or(previous_current);
        // 在新 total 与 pageSize 下归一化页码。
        self.current.set(self.clamp_current(current));
        // 若受控 State 在声明生成后变化，确保最终镜像仍写回一致值。
        self.write_current_bound(self.current.get());
        self.show_size_changer = next.show_size_changer;
        self.show_total = next.show_total;
        self.simple = next.simple;
        self.show_jumper = next.show_jumper;
        if !self.show_jumper {
            self.cancel_jumper();
        }
        self.total_template = next.total_template;
        self.size = next.size;
        self.page_size_options = next.page_size_options;
        // 同步 UIX 生成的视觉表引用，不保留 Rust 视觉副本。
        self.visual = next.visual;
    }

    fn intrinsic_size(&self) -> Size {
        let range_count = self
            .visible_range(self.total_pages(), self.current.get())
            .len() as f32;
        let control_count = if self.simple { 3.0 } else { range_count + 2.0 };
        let layout = &self.visual.layout;
        let controls = control_count * self.size + (control_count - 1.0) * layout.gap;
        let total = if self.show_total {
            layout.extra_gap + layout.total_width
        } else {
            0.0
        };
        let changer = if self.show_size_changer && !self.page_size_options.is_empty() {
            layout.extra_gap + layout.size_changer_width
        } else {
            0.0
        };
        let jumper = if self.show_jumper {
            layout.extra_gap + layout.jumper_width
        } else {
            0.0
        };
        Size::new(
            controls + total + changer + jumper,
            self.size + layout.hit_vertical_extra,
        )
    }

    pub(crate) fn current_record_range(&self) -> Range<usize> {
        if self.total == 0 {
            return 0..0;
        }
        let start = (self.current.get().saturating_sub(1))
            .saturating_mul(self.page_size)
            .saturating_add(1)
            .min(self.total);
        let end = self
            .current
            .get()
            .saturating_mul(self.page_size)
            .min(self.total);
        start..end
    }

    /// 计算可见页码范围（含省略号逻辑，最多 7 个按钮）。
    pub(crate) fn visible_range(&self, total_pages: usize, cur: usize) -> Vec<usize> {
        if total_pages == 0 {
            return Vec::new();
        }
        if total_pages <= 7 {
            return (1..=total_pages).collect();
        }
        let mut pages = Vec::new();
        pages.push(1);
        if cur > 3 {
            pages.push(0);
        } // 省略号用 0 表示
        let start = (cur.saturating_sub(1)).max(2);
        let end = cur.saturating_add(1).min(total_pages - 1);
        for p in start..=end {
            pages.push(p);
        }
        if cur < total_pages - 2 {
            pages.push(0);
        }
        pages.push(total_pages);
        pages
    }

    fn clamp_current(&self, current: usize) -> usize {
        let total_pages = self.total_pages();
        if total_pages == 0 {
            1
        } else {
            current.clamp(1, total_pages)
        }
    }

    fn handle_pointer_down(&mut self, x: f32) -> EventResult {
        if self.jumper_active
            && self
                .jumper_x_range()
                .is_some_and(|range| !range.contains(&x))
        {
            self.commit_jumper();
        }
        let total_pages = self.total_pages();
        let cur = self.current.get();
        let mut btn_x = 0.0;
        if x >= btn_x && x < btn_x + self.size {
            self.change_page_by(-1);
            return EventResult::Handled;
        }
        let layout = &self.visual.layout;
        btn_x += self.size + layout.gap;

        if self.simple {
            if x >= btn_x && x < btn_x + self.size * layout.simple_width_factor {
                return EventResult::Handled;
            }
            btn_x += self.size * layout.simple_width_factor + layout.gap;
            if x >= btn_x && x < btn_x + self.size {
                self.change_page_by(1);
                return EventResult::Handled;
            }
            return EventResult::NotHandled;
        }

        for page in self.visible_range(total_pages, cur) {
            if page > 0 && x >= btn_x && x < btn_x + self.size {
                self.select_page(page);
                return EventResult::Handled;
            }
            btn_x += self.size + layout.gap;
        }

        if x >= btn_x && x < btn_x + self.size {
            self.change_page_by(1);
            return EventResult::Handled;
        }
        btn_x += self.size;

        if self.show_total {
            btn_x += layout.extra_gap + layout.total_width;
        }
        if self.show_jumper {
            let jumper_x = btn_x + layout.extra_gap;
            if x >= jumper_x && x < jumper_x + layout.jumper_width {
                self.begin_jumper_edit();
                return EventResult::Handled;
            }
            if self.jumper_active {
                self.commit_jumper();
            }
            btn_x = jumper_x + layout.jumper_width;
        }
        if self.show_size_changer && !self.page_size_options.is_empty() {
            let changer_x = btn_x + layout.extra_gap;
            if x >= changer_x && x < changer_x + layout.size_changer_width {
                self.cycle_page_size(true);
                return EventResult::Handled;
            }
        }
        EventResult::NotHandled
    }

    pub(crate) fn jumper_x_range(&self) -> Option<std::ops::Range<f32>> {
        if !self.show_jumper {
            return None;
        }
        let range_count = self
            .visible_range(self.total_pages(), self.current.get())
            .len() as f32;
        let control_count = if self.simple { 3.0 } else { range_count + 2.0 };
        let layout = &self.visual.layout;
        let controls = control_count * self.size + (control_count - 1.0) * layout.gap;
        let total = if self.show_total {
            layout.extra_gap + layout.total_width
        } else {
            0.0
        };
        let start = controls + total + layout.extra_gap;
        Some(start..start + layout.jumper_width)
    }

    fn begin_jumper_edit(&mut self) {
        self.jumper_active = true;
        self.jumper_buffer.clear();
    }

    fn append_jumper_text(&mut self, text: &str) -> EventResult {
        let before = self.jumper_buffer.len();
        let max_digits = self.total_pages().max(1).to_string().len().max(1);
        self.jumper_buffer.extend(
            text.chars()
                .filter(char::is_ascii_digit)
                .take(max_digits.saturating_sub(self.jumper_buffer.len())),
        );
        if self.jumper_buffer.len() != before {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    fn commit_jumper(&mut self) {
        if !self.jumper_active {
            return;
        }
        if let Ok(page) = self.jumper_buffer.parse::<usize>() {
            self.select_page(page);
        }
        self.jumper_active = false;
        self.jumper_buffer.clear();
    }

    fn cancel_jumper(&mut self) {
        self.jumper_active = false;
        self.jumper_buffer.clear();
    }

    fn change_page_by(&self, delta: isize) {
        let current = self.current.get();
        let next = if delta < 0 {
            current.saturating_sub(delta.unsigned_abs()).max(1)
        } else {
            current
                .saturating_add(delta as usize)
                .min(self.total_pages().max(1))
        };
        self.select_page(next);
    }

    fn select_page(&self, page: usize) {
        // 把用户请求限制到当前有效页码范围。
        let page = self.clamp_current(page);
        // 只有真实变化才写状态并发布 Change。
        if page != self.current.get() {
            // 先更新组件镜像。
            self.current.set(page);
            // 再写回声明端唯一状态源。
            self.write_current_bound(page);
            // 保存现有页码 Change 载荷。
            self.pending_change.set(Some(PaginationChange::Page(page)));
        }
    }

    fn cycle_page_size(&mut self, forward: bool) {
        if self.page_size_options.is_empty() {
            return;
        }
        let current = self
            .page_size_options
            .iter()
            .position(|option| *option == self.page_size);
        let next = match (current, forward) {
            (Some(index), true) => (index + 1) % self.page_size_options.len(),
            (Some(index), false) => {
                (index + self.page_size_options.len() - 1) % self.page_size_options.len()
            }
            (None, true) => 0,
            (None, false) => self.page_size_options.len() - 1,
        };
        let page_size = self.page_size_options[next];
        if page_size != self.page_size {
            // 更新组件 pageSize 镜像。
            self.page_size = page_size;
            // 用户切换先写回声明端 pageSize 状态。
            self.write_page_size_bound(page_size);
            // 新 pageSize 可能缩短总页数，需要收敛 current。
            self.current.set(self.clamp_current(self.current.get()));
            // 同步写回可能发生变化的 current。
            self.write_current_bound(self.current.get());
            // 保留既有 page_size=<值> Change 载荷。
            self.pending_change
                .set(Some(PaginationChange::PageSize(page_size)));
        }
    }

    // 从声明端 State 吸收 pageSize/current，并把非法值归一化回同一状态源。
    fn sync_bound_values(&mut self) {
        // 克隆 pageSize 句柄，避免读取期间扩大 self 的借用范围。
        if let Some(state) = self.page_size_binding.clone() {
            // 读取声明端最新 pageSize。
            let requested = state.get();
            // pageSize 至少为一。
            let page_size = requested.max(1);
            // 更新运行时镜像。
            self.page_size = page_size;
            // 零值归一化必须写回唯一状态源。
            if requested != page_size {
                // 提交合法 pageSize。
                state.set(page_size);
            }
        }
        // pageSize 先同步后才能正确限制 current。
        if let Some(state) = self.current_binding.clone() {
            // 读取声明端最新页码。
            let requested = state.get();
            // 按当前总页数归一化。
            let current = self.clamp_current(requested);
            // 更新运行时镜像。
            self.current.set(current);
            // 越界值必须同步写回唯一状态源。
            if requested != current {
                // 提交合法 current。
                state.set(current);
            }
        } else {
            // 即使 current 非受控，pageSize 外部变化也必须限制旧页码。
            self.current.set(self.clamp_current(self.current.get()));
        }
    }

    // 捕获声明端双状态的响应式依赖。
    fn capture_bound_value_dependencies(&self) {
        // pageSize 受控时登记其 State 依赖。
        if let Some(state) = self.page_size_binding.as_ref() {
            // 读取值以接入当前追踪上下文。
            let _ = state.get();
        }
        // current 受控时登记其 State 依赖。
        if let Some(state) = self.current_binding.as_ref() {
            // 读取值以接入当前追踪上下文。
            let _ = state.get();
        }
    }

    // 把 current 写回受控 State。
    fn write_current_bound(&self, current: usize) {
        // 非受控模式没有外部写回目标。
        let Some(state) = self.current_binding.as_ref() else {
            // 直接返回，保留组件自身状态。
            return;
        };
        // 相同值不产生多余 generation 与 reconcile。
        if state.get() != current {
            // 提交新页码。
            state.set(current);
        }
    }

    // 把 pageSize 写回受控 State。
    fn write_page_size_bound(&self, page_size: usize) {
        // 非受控模式没有外部写回目标。
        let Some(state) = self.page_size_binding.as_ref() else {
            // 直接返回，保留组件自身状态。
            return;
        };
        // 相同值不产生多余 generation 与 reconcile。
        if state.get() != page_size {
            // 提交新的每页条数。
            state.set(page_size);
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Pagination {
            total: self.total,
            page_size: self.page_size,
            current: self.current.get(),
            show_size_changer: self.show_size_changer,
            show_total: self.show_total,
            size: self.size,
            page_size_options: self.page_size_options.clone(),
        }
    }
}

// UIX 只注入静态视觉表，Rust 内核继续拥有双状态、页码算法和文本输入。
fn build_pagination_view(mut kernel: Pagination, visual: &'static PaginationVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Pagination {
    fn build(self) -> ViewNode {
        build_pagination_view(self, PAGINATION_VISUAL_REF)
    }
}

// 只在组件边界验证 Pagination 双状态与 reconcile 所有权。
