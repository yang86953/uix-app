//! Cascader 级联选择器 — 多级联动下拉选择。
//!
//! 支持多级选项、搜索过滤、选中回显。

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};
use uix_graphics::{Color, GraphicsEngine};
use uix_platform::{Point, Rect, Size};

/// 级联选项
#[derive(Debug, Clone)]
pub struct CascaderOption {
    pub label: String,
    pub value: String,
    pub children: Vec<CascaderOption>,
    pub disabled: bool,
}

impl CascaderOption {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            children: vec![],
            disabled: false,
        }
    }
    pub fn children(mut self, children: Vec<CascaderOption>) -> Self {
        self.children = children;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
}

/// 选中的级联路径项
#[derive(Debug, Clone, PartialEq)]
pub struct CascaderValue {
    pub labels: Vec<String>,
    pub values: Vec<String>,
}

define_widget! {
    /// Cascader — 级联选择器。
    pub struct Cascader {
        /// 选项树
        options: Vec<CascaderOption>,
        /// 当前选中值（各级路径）
        selected: CascaderValue,
        /// 每层展开的选项列表
        current_levels: Vec<Vec<CascaderOption>>,
        /// 每层选中索引
        level_indices: Vec<usize>,
        /// 弹出层是否展开
        open: bool,
        /// 占位文本
        placeholder: String,
        /// 焦点
        focused: bool,
    }


    tab_index => (&self) -> i32 { 1 }
    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(120.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos: _, .. } => {
                // 点击输入框切换弹出
                self.focused = true;
                if !self.open {
                    self.open = true;
                    self.init_levels();
                }
                EventResult::Handled
            }
            WidgetEvent::FocusOut => { self.focused = false; self.open = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                if !self.open { return EventResult::NotHandled; }
                match key {
                    KeyCode::Escape => { self.open = false; EventResult::Handled }
                    KeyCode::Enter => {
                        // 确认当前选择
                        self.confirm_selection();
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let loc = crate::locale::use_locale();
        let primary = ctx.tokens().color_primary();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let border_radius_sm = ctx.tokens().border_radius_sm();
        let radius = Some(uix_graphics::Radius::uniform(border_radius_sm));

        // 输入框
        ctx.fill_rect(frame, Color::white(), radius);
        ctx.stroke_rect(frame, if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 }, radius);

        let draw_y = ctx.visual_center_y(frame, 14.0);
        if self.selected.labels.is_empty() {
            ctx.draw_text(&self.placeholder, Point::new(frame.x + 12.0, draw_y),
                text_tertiary, 14.0);
        } else {
            let display_text = self.selected.labels.join(loc.cascader_separator);
            ctx.draw_text(&display_text, Point::new(frame.x + 12.0, draw_y),
                text_color, 14.0);
        }

        // 下拉箭头
        let arrow_y = ctx.visual_center_y(frame, 8.0);
        ctx.draw_text(if self.open { "▲" } else { "▼" },
            Point::new(frame.x + frame.w - 20.0, arrow_y), text_secondary, 8.0);

        // 弹出层
        if self.open {
            let popup_w = frame.w.max(200.0);
            let popup = Rect::new(frame.x, frame.y + frame.h + 2.0, popup_w, 200.0);
            ctx.fill_rect(popup, bg_elevated, radius);
            ctx.stroke_rect(popup, border_color, 1.0, radius);

            // 当前级别选项
            let current_opts = if let Some(level) = self.current_levels.last() {
                level
            } else { return; };

            let item_h = 32.0;
            let visible = (popup.h / item_h) as usize;
            let start = 0;

            for i in start..(start + visible).min(current_opts.len()) {
                let y = popup.y + (i - start) as f32 * item_h;
                if i < current_opts.len() {
                    let opt = &current_opts[i];
                    let has_children = !opt.children.is_empty();

                    // 高亮
                    if let Some(&sel_idx) = self.level_indices.last() {
                        if i == sel_idx {
                            ctx.fill_rect(Rect::new(popup.x, y, popup.w, item_h),
                                ctx.tokens().color_primary_bg(), None);
                        }
                    }

                    ctx.draw_text(&opt.label,
                        Point::new(popup.x + 12.0, y + (item_h - 14.0) * 0.5),
                        if opt.disabled { text_tertiary } else { text_color }, 14.0);

                    if has_children {
                        ctx.draw_text(loc.cascader_arrow,
                            Point::new(popup.x + popup.w - 16.0, y + (item_h - 14.0) * 0.5),
                            text_secondary, 14.0);
                    }
                }
            }
        }
    }
}

impl Cascader {
    pub fn new(options: Vec<CascaderOption>, placeholder: impl Into<String>) -> Self {
        Self {
            options: options.clone(),
            selected: CascaderValue {
                labels: vec![],
                values: vec![],
            },
            current_levels: vec![options],
            level_indices: vec![0],
            open: false,
            placeholder: placeholder.into(),
            focused: false,
        }
    }

    fn init_levels(&mut self) {
        self.current_levels = vec![self.options.clone()];
        self.level_indices = vec![0];
    }

    /// 点击某选项时触发（由外部或 MouseDown 处理）
    pub fn select_option(&mut self, level: usize, index: usize) {
        if level >= self.current_levels.len() {
            return;
        }
        let opt = match self.current_levels[level].get(index) {
            Some(o) => o.clone(),
            None => return,
        };
        if opt.disabled {
            return;
        }

        // 截断到当前层级
        self.level_indices.truncate(level);
        self.level_indices.push(index);
        self.selected.labels.truncate(level);
        self.selected.values.truncate(level);
        self.selected.labels.push(opt.label.clone());
        self.selected.values.push(opt.value.clone());

        // 更新子级选项列表
        self.current_levels.truncate(level + 1);
        if !opt.children.is_empty() {
            self.current_levels.push(opt.children);
        } else {
            // 叶子节点，关闭弹出
            self.open = false;
        }
    }

    fn confirm_selection(&mut self) {
        // 如果有选中值就关闭
        if !self.selected.values.is_empty() {
            self.open = false;
        }
    }

    pub fn selected(&self) -> &CascaderValue {
        &self.selected
    }
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }
}
