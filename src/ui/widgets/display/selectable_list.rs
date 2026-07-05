//! SelectableList — 可选中的列表组件
//!
//! 支持列表项选中高亮、悬停高亮、图标前缀、Header/Footer、
//! 点击回调、滚动。适合侧边栏导航、对话列表等场景。

use std::cell::Cell;

use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::{traits::GraphicsEngine, Color, Radius};
use crate::native::{Point, Rect, Size};
use crate::ui::{EventResult, SystemEvent, WidgetTree};

/// 列表项数据
#[derive(Debug, Clone)]
pub struct SelectableItem {
    /// 唯一标识
    pub id: String,
    /// 显示文本
    pub text: String,
    /// 可选前缀图标（如 "💬"）
    pub icon: Option<String>,
}

impl SelectableItem {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            icon: None,
        }
    }
    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }
}

define_widget! {
    /// 可选中的列表组件
    ///
    /// 垂直排列列表项，支持：
    /// - 选中态高亮（primary 色 accent 线）
    /// - 悬停态背景变化
    /// - 图标前缀
    /// - Header 按钮区
    /// - Footer 状态栏
    /// - 鼠标滚轮滚动
    pub struct SelectableList {
        /// 列表项
        pub items: Vec<SelectableItem>,

        /// 当前选中索引
        pub active_index: usize,

        /// Header 按钮文本（如 "+ 新建对话"），空则不显示
        pub header_button_text: String,

        /// Footer 文本（如 "3 个对话"），空则不显示
        pub footer_text: String,

        /// 每项高度
        pub item_height: f32,

        /// 选中回调
        pub on_select: Option<Box<dyn FnMut(usize)>>,

        /// Header 按钮点击回调
        pub on_header_click: Option<Box<dyn FnMut()>>,

        /// 列表项删除回调（点击每项右侧的 ✕ 按钮触发）
        pub on_item_remove: Option<Box<dyn FnMut(usize)>>,

        // ── 内部状态 ──
        hovered_index: Cell<Option<usize>>,
        hovered_header: Cell<bool>,
        scroll_y: Cell<f32>,
    }

    @new -> Self {
        Self {
            items: Vec::new(),
            active_index: 0,
            header_button_text: String::new(),
            footer_text: String::new(),
            item_height: 36.0,
            on_select: None,
            on_header_click: None,
            on_item_remove: None,
            hovered_index: Cell::new(None),
            hovered_header: Cell::new(false),
            scroll_y: Cell::new(0.0),
        }
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        let mut h = 0.0;
        // Header 按钮
        if !self.header_button_text.is_empty() {
            h += 48.0;
        }
        // 列表项
        h += self.items.len() as f32 * (self.item_height + 2.0);
        // Footer
        if !self.footer_text.is_empty() {
            h += 28.0;
        }
        Size::new(220.0, h.max(100.0))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                let mut y = 0.0;

                // Header 按钮点击
                if !self.header_button_text.is_empty() {
                    let btn_rect = Rect::new(8.0, y, 204.0, 40.0);
                    if btn_rect.contains(*pos) {
                        if let Some(ref mut cb) = self.on_header_click {
                            cb();
                        }
                        return EventResult::Handled;
                    }
                    y += 48.0;
                }

                // 列表项点击
                for i in 0..self.items.len() {
                    let iy = y + i as f32 * (self.item_height + 2.0) + self.scroll_y.get();
                    let item_rect = Rect::new(0.0, iy, 220.0, self.item_height);
                    if item_rect.contains(*pos) {
                        // 检查是否点击了 ✕ 删除按钮
                        // 与渲染对齐：del_btn = (item_frame.x + item_frame.w - 24.0, item_frame.y + 8.0)
                        // item_frame = (frame.x + 8.0, iy, frame.w - 16.0, item_height)
                        // frame.x = 0, frame.w = 220 → del_btn.x = 8 + 204 - 24 = 188
                        // 从 item_rect 角度：220 - 32 = 188
                        if self.on_item_remove.is_some() {
                            let del_rect = Rect::new(
                                item_rect.x + 220.0 - 32.0,
                                iy + 8.0,
                                20.0, 20.0,
                            );
                            if del_rect.contains(*pos) {
                                if let Some(ref mut cb) = self.on_item_remove {
                                    cb(i);
                                }
                                return EventResult::Handled;
                            }
                        }

                        self.active_index = i;
                        if let Some(ref mut cb) = self.on_select {
                            cb(i);
                        }
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }

            SystemEvent::PointerMove { pos, .. } => {
                let old_hover = self.hovered_index.get();
                let old_btn = self.hovered_header.get();
                let mut new_hover: Option<usize> = None;
                let mut new_btn = false;

                let mut y = 0.0;
                if !self.header_button_text.is_empty() {
                    let btn_rect = Rect::new(8.0, y, 204.0, 40.0);
                    if btn_rect.contains(*pos) { new_btn = true; }
                    y += 48.0;
                }

                for i in 0..self.items.len() {
                    let iy = y + i as f32 * (self.item_height + 2.0) + self.scroll_y.get();
                    let item_rect = Rect::new(0.0, iy, 220.0, self.item_height);
                    if item_rect.contains(*pos) { new_hover = Some(i); break; }
                }

                self.hovered_index.set(new_hover);
                self.hovered_header.set(new_btn);
                if old_hover != new_hover || old_btn != new_btn { EventResult::Handled }
                else { EventResult::NotHandled }
            }

            SystemEvent::Wheel { delta, .. } => {
                let sy = self.scroll_y.get();
                let max_scroll = -(self.items.len() as f32 * (self.item_height + 2.0) - 500.0).min(0.0);
                self.scroll_y.set((sy + delta.y * 0.5).max(max_scroll).min(0.0));
                EventResult::Handled
            }

            SystemEvent::PointerLeave => {
                self.hovered_index.set(None);
                self.hovered_header.set(false);
                EventResult::Handled
            }

            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_layout();
        let border = Color::from_rgb(40, 40, 45);
        let t_sec = ctx.tokens().color_text_secondary();
        let t_ter = ctx.tokens().color_text_tertiary();
        let t_pri = ctx.tokens().color_primary();

        // 背景
        ctx.fill_rect(frame, bg, None);
        // 右侧分割线
        ctx.fill_rect(Rect::new(frame.x + frame.w - 1.0, frame.y, 1.0, frame.h), border, None);

        let mut y = frame.y;

        // ── Header 按钮 ──
        if !self.header_button_text.is_empty() {
            let btn_frame = Rect::new(frame.x + 8.0, y + 8.0, frame.w - 16.0, 32.0);
            let btn_bg = if self.hovered_header.get() {
                Color::from_rgb(45, 45, 52)
            } else {
                Color::from_rgb(35, 35, 42)
            };
            ctx.fill_rect(btn_frame, btn_bg, Some(Radius::uniform(6.0)));
            ctx.draw_text("+", Point::new(btn_frame.x + 10.0, btn_frame.y + 7.0), t_sec, 15.0);
            ctx.draw_text(&self.header_button_text, Point::new(btn_frame.x + 28.0, btn_frame.y + 8.0), t_sec, 13.0);
            // 分割线
            ctx.fill_rect(Rect::new(frame.x + 8.0, btn_frame.y + btn_frame.h + 8.0, frame.w - 16.0, 1.0), border, None);
            y = btn_frame.y + btn_frame.h + 16.0;
        }

        // ── 列表项 ──
        let sy = self.scroll_y.get();
        let list_clip = Rect::new(frame.x, y, frame.w, frame.h - y - 28.0);
        ctx.canvas_2d().push_clip(list_clip);

        for i in 0..self.items.len() {
            let iy = y + i as f32 * (self.item_height + 2.0) + sy;
            if iy + self.item_height < y || iy > y + list_clip.h { continue; } // 裁剪不可见项

            let is_active = i == self.active_index;
            let is_hover = self.hovered_index.get() == Some(i);
            let item_frame = Rect::new(frame.x + 8.0, iy, frame.w - 16.0, self.item_height);

            // 背景
            if is_active {
                ctx.fill_rect(item_frame, Color::from_rgba(55, 110, 255, 25), Some(Radius::uniform(6.0)));
                ctx.fill_rect(Rect::new(item_frame.x, item_frame.y + 6.0, 3.0, self.item_height - 12.0),
                    t_pri, Some(Radius::uniform(1.5)));
            } else if is_hover {
                ctx.fill_rect(item_frame, Color::from_rgb(42, 42, 48), Some(Radius::uniform(6.0)));
            }

            // 图标
            let icon = self.items[i].icon.as_deref().unwrap_or("");
            let text_x = if icon.is_empty() { item_frame.x + 14.0 } else { item_frame.x + 32.0 };
            if !icon.is_empty() {
                ctx.draw_text(icon, Point::new(item_frame.x + 10.0, item_frame.y + 8.0), t_sec, 14.0);
            }

            // 文本
            let color = if is_active { t_pri } else { t_sec };
            ctx.draw_text(&self.items[i].text, Point::new(text_x, item_frame.y + 9.0), color, 13.0);

            // ✕ 删除按钮（仅悬停时显示）
            if is_hover && self.on_item_remove.is_some() {
                let del_btn = Rect::new(
                    item_frame.x + item_frame.w - 24.0,
                    item_frame.y + 8.0,
                    20.0, 20.0,
                );
                ctx.fill_rect(del_btn, Color::from_rgba(200, 60, 60, 180), Some(Radius::uniform(10.0)));
                ctx.draw_text("✕", Point::new(del_btn.x + 5.0, del_btn.y + 3.0),
                    Color::white(), 12.0);
            }
        }

        ctx.canvas_2d().pop_clip();

        // ── Footer ──
        if !self.footer_text.is_empty() {
            let fy = frame.y + frame.h - 24.0;
            ctx.draw_text(&self.footer_text, Point::new(frame.x + 12.0, fy), t_ter, 11.0);
        }
    }
}
