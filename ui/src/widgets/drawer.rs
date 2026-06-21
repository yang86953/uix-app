//! Drawer widget — 抽屉面板，从屏幕侧边滑入。
//!
//! 支持从 right/left/top/bottom 四个方向滑出，半透明遮罩层，
//! 可配置标题、尺寸、可关闭。

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

/// 抽屉滑出方向。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawerPlacement {
    Right,
    Left,
    Top,
    Bottom,
}

/// Drawer — 抽屉面板。
define_widget! {
    pub struct Drawer {
        title: String,
        visible: bool,
        width: f32,
        height: f32,
        placement: DrawerPlacement,
        closable: bool,
        mask_closable: bool,
        mask: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        if self.visible {
            match self.placement {
                DrawerPlacement::Right | DrawerPlacement::Left => Size::new(self.width, 600.0),
                DrawerPlacement::Top | DrawerPlacement::Bottom => Size::new(400.0, self.height),
            }
        } else {
            Size::zero()
        }
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if !self.visible { return EventResult::NotHandled; }
        if let WidgetEvent::MouseDown { pos, .. } = event {
            if self.mask_closable {
                // 点击遮罩关闭：根据放置方向判断点击是否在抽屉外
                let outside = match self.placement {
                    DrawerPlacement::Right => pos.x < 0.0,
                    DrawerPlacement::Left  => pos.x >= self.width,
                    DrawerPlacement::Top   => pos.y >= self.height,
                    DrawerPlacement::Bottom => pos.y < 0.0,
                };
                if outside { self.visible = false; return EventResult::Handled; }
            }
            // 关闭按钮
            if self.closable {
                let cx = match self.placement {
                    DrawerPlacement::Right => self.width - 36.0,
                    DrawerPlacement::Left => self.width - 36.0,
                    DrawerPlacement::Top => pos.x, // 简化：顶部/底部暂不支持关闭按钮点击
                    DrawerPlacement::Bottom => pos.x,
                };
                if pos.x >= cx - 12.0 && pos.x <= cx + 12.0 && pos.y >= 8.0 && pos.y <= 32.0 {
                    self.visible = false; return EventResult::Handled;
                }
            }
        }
        if let WidgetEvent::KeyDown { key, .. } = event {
            if *key == crate::widget::KeyCode::Escape && self.closable {
                self.visible = false; return EventResult::Handled;
            }
        }
        EventResult::Handled
    }

    render => (&self, _frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if !self.visible { return; }
        // 遮罩层（全屏）
        if self.mask {
            ctx.fill_rect(Rect::new(-2000.0, -2000.0, 4000.0, 4000.0),
                Color::from_rgba(0, 0, 0, 96), None);
        }
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let r = Radius::uniform(ctx.tokens().border_radius_lg());
        // 绘制抽屉面板
        let (drawer_x, drawer_y, drawer_w, drawer_h) = match self.placement {
            DrawerPlacement::Right => (_frame.x, _frame.y, self.width, _frame.h),
            DrawerPlacement::Left  => (_frame.x, _frame.y, self.width, _frame.h),
            DrawerPlacement::Top   => (_frame.x, _frame.y, _frame.w, self.height),
            DrawerPlacement::Bottom => (_frame.x, _frame.y, _frame.w, self.height),
        };
        let drawer_rect = Rect::new(drawer_x, drawer_y, drawer_w, drawer_h);
        // 圆角仅在单侧
        let corner = match self.placement {
            DrawerPlacement::Right => Some(Radius { tl: r.tl, tr: 0.0, br: 0.0, bl: r.br }),
            DrawerPlacement::Left  => Some(Radius { tl: 0.0, tr: r.tr, br: r.bl, bl: 0.0 }),
            DrawerPlacement::Top   => Some(Radius { tl: 0.0, tr: 0.0, br: r.bl, bl: r.br }),
            DrawerPlacement::Bottom => Some(Radius { tl: r.tl, tr: r.tr, br: 0.0, bl: 0.0 }),
        };
        ctx.fill_rect(drawer_rect, bg, corner);
        ctx.stroke_rect(drawer_rect, border, 1.0, corner);
        // 标题
        ctx.draw_text(&self.title, Point::new(drawer_x + 24.0, drawer_y + 14.0), text, 16.0);
        // 关闭按钮
        if self.closable {
            let close_x = drawer_x + drawer_w - 36.0;
            ctx.draw_text("✕", Point::new(close_x, drawer_y + 14.0), text_sec, 16.0);
        }
        // 标题分割线
        ctx.fill_rect(Rect::new(drawer_x, drawer_y + 48.0, drawer_w, 1.0), border, None);
    }

    layout_children => (&self, frame: Rect, children: &[crate::widget::WidgetId], _tree: &WidgetTree)
        -> Vec<(crate::widget::WidgetId, Rect)>
    {
        if !self.visible || children.is_empty() { return Vec::new(); }
        let (drawer_x, drawer_y, drawer_w, drawer_h) = match self.placement {
            DrawerPlacement::Right => (frame.x, frame.y, self.width, frame.h),
            DrawerPlacement::Left  => (frame.x, frame.y, self.width, frame.h),
            DrawerPlacement::Top   => (frame.x, frame.y, frame.w, self.height),
            DrawerPlacement::Bottom => (frame.x, frame.y, frame.w, self.height),
        };
        let body_y = drawer_y + 56.0;
        let body_h = drawer_h - 56.0;
        let pad = 24.0;
        children.iter().map(|&cid| (cid, Rect::new(drawer_x + pad, body_y + pad, drawer_w - pad * 2.0, body_h - pad * 2.0))).collect()
    }
}

impl Drawer {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            visible: false,
            width: 378.0,
            height: 300.0,
            placement: DrawerPlacement::Right,
            closable: true,
            mask_closable: true,
            mask: true,
        }
    }
    pub fn visible(mut self, v: bool) -> Self { self.visible = v; self }
    pub fn show(mut self) -> Self { self.visible = true; self }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.width = w; self.height = h; self }
    pub fn placement(mut self, p: DrawerPlacement) -> Self { self.placement = p; self }
    pub fn closable(mut self, v: bool) -> Self { self.closable = v; self }
    pub fn mask_closable(mut self, v: bool) -> Self { self.mask_closable = v; self }
    pub fn mask(mut self, v: bool) -> Self { self.mask = v; self }
    pub fn is_visible(&self) -> bool { self.visible }
    pub fn open(&mut self) { self.visible = true; }
    pub fn close(&mut self) { self.visible = false; }
    pub fn set_visible(&mut self, v: bool) { self.visible = v; }
}
