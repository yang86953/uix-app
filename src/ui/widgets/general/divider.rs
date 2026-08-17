//! Ant Design 风格的水平或垂直分隔线，支持可选文本。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::SnapshotFields;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;

/// 水平分隔线的文本位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerOrientation {
    /// 文本靠左放置。
    Left,
    /// 文本居中放置。
    Center,
    /// 文本靠右放置。
    Right,
}

/// 分隔线方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerDirection {
    /// 水平分隔线。
    Horizontal,
    /// 垂直分隔线。
    Vertical,
}

component! {
    /// 支持可选标签的分隔线组件。
    pub struct Divider {
        text: Option<String>,
        orientation: DividerOrientation,
        direction: DividerDirection,
        /// 可选的分隔线颜色覆写；`None` 使用主题边框色。
        pub color: Option<Color>,
        text_size: f32,
        dashed: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let line_color = self.color.unwrap_or(ctx.tokens().color_border_secondary());
        let text_secondary = ctx.tokens().color_text_secondary();

        let draw_line = |ctx: &mut PaintContext, x: f32, y: f32, w: f32, h: f32, color: Color| {
            if !self.dashed {
                ctx.fill_rect(Rect::new(x, y, w, h), color, None);
            } else {
                let seg_len: f32 = 6.0;
                let gap_len: f32 = 4.0;
                let mut dx = 0.0;
                let is_h = h <= w;
                while dx < (if is_h { w } else { h }) {
                    let seg = seg_len.min(if is_h { w - dx } else { h - dx });
                    if is_h {
                        ctx.fill_rect(Rect::new(x + dx, y, seg, h), color, None);
                    } else {
                        ctx.fill_rect(Rect::new(x, y + dx, w, seg), color, None);
                    }
                    dx += seg + gap_len;
                }
            }
        };

        match self.direction {
            DividerDirection::Horizontal => {
                let center_y = frame.y + frame.h * 0.5;

                if let Some(ref text) = self.text {
                    let text_w = text.len() as f32 * 7.5 + 8.0;
                    let _text_h = self.text_size;

                    let text_x = match self.orientation {
                        DividerOrientation::Left => frame.x + 32.0,
                        DividerOrientation::Center => frame.x + (frame.w - text_w) * 0.5,
                        DividerOrientation::Right => frame.x + frame.w - text_w - 32.0,
                    };
                    let text_y = ctx.visual_center_y(frame, self.text_size);

                    let left_end = text_x - 8.0;
                    if left_end > frame.x {
                        draw_line(ctx, frame.x, center_y, left_end - frame.x, 1.0, line_color);
                    }

                    ctx.draw_text(text, Point::new(text_x, text_y), text_secondary, self.text_size);

                    let right_start = text_x + text_w;
                    if right_start < frame.x + frame.w {
                        draw_line(ctx, right_start, center_y, frame.x + frame.w - right_start, 1.0, line_color);
                    }
                } else {
                    draw_line(ctx, frame.x, frame.y, frame.w, frame.h, line_color);
                }
            }
            DividerDirection::Vertical => {
                draw_line(ctx, frame.x, frame.y, frame.w, frame.h, line_color);
            }
        }
    }
}

impl Divider {
    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.orientation = next.orientation;
        self.direction = next.direction;
        self.color = next.color;
        self.text_size = next.text_size;
        self.dashed = next.dashed;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Divider {
            text: self.text.clone(),
            orientation: self.orientation,
            direction: self.direction,
            color: self.color,
            text_size: self.text_size,
            dashed: self.dashed,
        }
    }
}

impl Default for Divider {
    fn default() -> Self {
        Self::new()
    }
}

impl Divider {
    /// 创建居中的无文本水平实线分隔线。
    pub fn new() -> Self {
        Self {
            text: None,
            orientation: DividerOrientation::Center,
            direction: DividerDirection::Horizontal,
            color: None,
            text_size: 14.0,
            dashed: false,
        }
    }

    /// 设置水平分隔线显示的文本。
    pub fn with_text(mut self, t: &str) -> Self {
        self.text = Some(t.to_string());
        self
    }
    /// 设置水平分隔线的文本位置。
    pub fn orientation(mut self, o: DividerOrientation) -> Self {
        self.orientation = o;
        self
    }
    /// 将分隔线方向设为垂直。
    pub fn vertical(mut self) -> Self {
        self.direction = DividerDirection::Vertical;
        self
    }
    /// 设置分隔线颜色。
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// 将分隔线设为虚线。
    pub fn dashed(mut self) -> Self {
        self.dashed = true;
        self
    }

    fn intrinsic_size(&self) -> Size {
        match self.direction {
            DividerDirection::Horizontal => {
                if self.text.is_some() {
                    Size::new(0.0, 24.0)
                } else {
                    Size::new(0.0, 1.0)
                }
            }
            DividerDirection::Vertical => Size::new(1.0, 0.0),
        }
    }
}
