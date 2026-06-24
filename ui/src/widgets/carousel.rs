//! Carousel 轮播图 — 图片/内容轮播切换。
//!
//! 支持自动播放、指示器、切换动画（滑动/淡入淡出）。

use std::cell::Cell;
use std::time::Instant;

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, GraphicsEngine};
use crate::children::WidgetChildren;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, Widget, WidgetEvent, WidgetId, WidgetTree};

define_widget! {
    /// Carousel — 轮播图组件。
    pub struct Carousel {
        children: WidgetChildren,
        /// 当前索引
        current: Cell<usize>,
        /// 自动播放间隔（秒），0 表示不自动
        autoplay_interval: f32,
        /// 上次切换时间（实际时间戳，不依赖帧dt）
        last_switch: Cell<Option<Instant>>,
        /// 是否显示指示器圆点
        show_dots: bool,
        /// 是否显示箭头
        show_arrows: bool,
        /// 动画进度 (0~1)
        anim_progress: Cell<f32>,
        /// 是否正在切换
        animating: Cell<bool>,
        /// 当前 frame（hit-test 用）
        last_frame: Cell<Option<Rect>>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(300.0, 200.0)
    }

    flex_grow => (&self) -> f32 { 1.0 }

    build => (&self) -> Vec<Box<dyn Widget>> {
        self.children.take()
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                if let Some(frame) = self.last_frame.get() {
                    let dot_area_y = frame.y + frame.h - 20.0;
                    if pos.y >= dot_area_y && pos.y <= dot_area_y + 12.0 {
                        // 点击指示器
                        let count = self.children.len();
                        if count > 0 {
                            let dot_w = frame.w / count as f32;
                            let idx = ((pos.x - frame.x) / dot_w) as usize;
                            if idx < count && idx != self.current.get() {
                                self.current.set(idx);
                            }
                        }
                        return EventResult::Handled;
                    }
                    if self.show_arrows {
                        let arrow_area = 30.0;
                        if pos.x - frame.x < arrow_area {
                            // 上一张
                            let count = self.children.len();
                            if count > 0 {
                                let new_idx = (self.current.get() + count - 1) % count;
                                self.current.set(new_idx);
                            }
                            return EventResult::Handled;
                        }
                        if frame.x + frame.w - pos.x < arrow_area {
                            // 下一张
                            let count = self.children.len();
                            if count > 0 {
                                self.current.set((self.current.get() + 1) % count);
                            }
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
    }

    on_update => (&mut self, _dt: f32) {
        if self.autoplay_interval > 0.0 && self.children.len() > 1 {
            let now = Instant::now();
            let should_switch = match self.last_switch.get() {
                Some(last) => now.duration_since(last).as_secs_f32() >= self.autoplay_interval,
                None => true, // 首次立即记录时间，不切换
            };
            if should_switch {
                self.last_switch.set(Some(now));
                self.current.set((self.current.get() + 1) % self.children.len());
                self.animating.set(true);
                self.anim_progress.set(0.0);
            }
        }
        // 切换动画推进
        if self.animating.get() {
            self.anim_progress.set((self.anim_progress.get() + _dt * 2.0).min(1.0));
            if self.anim_progress.get() >= 1.0 {
                self.animating.set(false);
            }
        }
    }

    needs_continuous_update => (&self) -> bool {
        // 只在自动轮播且正处过渡动画中才持续更新
        self.autoplay_interval > 0.0 && self.animating.get()
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        self.last_frame.set(Some(frame));
        let count = self.children.len();
        if count == 0 { return; }

        let idx = self.current.get();
        let bg = ctx.tokens().color_bg_container();
        let primary = ctx.tokens().color_primary();
        let border_color = ctx.tokens().color_border();
        let dot_color = ctx.tokens().color_text_quaternary();

        // 背景
        ctx.fill_rect(frame, bg, None);

        // 当前 slide 编号文字（代替实际子节点渲染）
        let text = format!("Slide {}", idx + 1);
        ctx.text_center(&text, frame, ctx.tokens().color_text(), 14.0);

        // 箭头
        if self.show_arrows && count > 1 {
            let arrow_y = ctx.visual_center_y(frame, 14.0);
            ctx.draw_text("◀", Point::new(frame.x + 10.0, arrow_y),
                ctx.tokens().color_text(), 14.0);
            ctx.draw_text("▶", Point::new(frame.x + frame.w - 22.0, arrow_y),
                ctx.tokens().color_text(), 14.0);
        }

        // 指示器圆点
        if self.show_dots && count > 1 {
            let dot_y = frame.y + frame.h - 14.0;
            let total_dot_w = count as f32 * 12.0;
            let start_x = frame.x + (frame.w - total_dot_w) * 0.5;

            for i in 0..count {
                let is_active = i == idx;
                let dot_rect = Rect::new(
                    start_x + i as f32 * 12.0, dot_y,
                    if is_active { 16.0 } else { 8.0 }, 6.0,
                );
                ctx.fill_rect(dot_rect,
                    if is_active { primary } else { dot_color },
                    Some(uix_graphics::Radius::uniform(3.0)));
            }
        }
    }

    layout_children => (&self, frame: Rect, children: &[WidgetId], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        // 所有子节点都布局为 frame 大小（叠放），通过 current 控制显示哪个
        children.iter().map(|&cid| (cid, frame)).collect()
    }
}

impl Carousel {
    pub fn new() -> Self {
        Self {
            children: WidgetChildren::new(),
            current: Cell::new(0),
            autoplay_interval: 3.0,
            last_switch: Cell::new(None),
            show_dots: true,
            show_arrows: true,
            anim_progress: Cell::new(0.0),
            animating: Cell::new(false),
            last_frame: Cell::new(None),
        }
    }

    pub fn autoplay(mut self, interval: f32) -> Self { self.autoplay_interval = interval; self }
    pub fn show_dots(mut self, v: bool) -> Self { self.show_dots = v; self }
    pub fn show_arrows(mut self, v: bool) -> Self { self.show_arrows = v; self }

    /// 当前展示的子节点索引（供外部渲染使用）
    pub fn current_index(&self) -> usize { self.current.get() }
}

// 用于 hit-test 的 frame 存储
// 已经通过 last_frame 字段处理
