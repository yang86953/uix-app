use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::{SnapshotFields, WidgetTree};

// ════════════════════════════════════════════════════════════════════════════
// Watermark
// ════════════════════════════════════════════════════════════════════════════

component! {
    pub struct Watermark {
        text: String,
        color: Color,
        font_size: f32,
        opacity: f32,
        rotate: f32,
        gap_x: f32,
        gap_y: f32,
        x_offset: f32,
        y_offset: f32,
    }

    measure => (&self, _constraints: Constraints) -> Size {
        Size::zero()
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if self.text.is_empty() { return; }
        let color = self.effective_color();
        let columns = (frame.w.max(0.0) / self.gap_x).ceil() as usize + 2;
        let rows = (frame.h.max(0.0) / self.gap_y).ceil() as usize + 2;

        ctx.push_clip(frame);
        for gy in 0..rows {
            for gx in 0..columns {
                self.paint_rotated_text(ctx, self.tile_position(frame, gx, gy), color);
            }
        }
        ctx.pop_clip();
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // Watermark 通过 render 在全帧范围绘制水印，
        // dirty_rect 返回实际 frame 区域（通常是全屏），
        // 禁止返回硬编码巨型区域（原 -10000~20000）避免脏区域爆炸。
        frame
    }
}
impl Watermark {
    const DEFAULT_FONT_SIZE: f32 = 14.0;
    const DEFAULT_OPACITY: f32 = 0.15;
    const DEFAULT_ROTATE: f32 = -22.0;
    const DEFAULT_GAP_X: f32 = 200.0;
    const DEFAULT_GAP_Y: f32 = 160.0;

    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            color: Color::from_rgba(0, 0, 0, 255),
            font_size: Self::DEFAULT_FONT_SIZE,
            opacity: Self::DEFAULT_OPACITY,
            rotate: Self::DEFAULT_ROTATE,
            gap_x: Self::DEFAULT_GAP_X,
            gap_y: Self::DEFAULT_GAP_Y,
            x_offset: 0.0,
            y_offset: 0.0,
        }
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size = Self::positive_or(s, Self::DEFAULT_FONT_SIZE);
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = if o.is_finite() {
            o.clamp(0.0, 1.0)
        } else {
            Self::DEFAULT_OPACITY
        };
        self
    }
    pub fn rotate(mut self, r: f32) -> Self {
        self.rotate = if r.is_finite() {
            r
        } else {
            Self::DEFAULT_ROTATE
        };
        self
    }
    pub fn gap(mut self, x: f32, y: f32) -> Self {
        self.gap_x = Self::positive_or(x, Self::DEFAULT_GAP_X);
        self.gap_y = Self::positive_or(y, Self::DEFAULT_GAP_Y);
        self
    }
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.x_offset = if x.is_finite() { x } else { 0.0 };
        self.y_offset = if y.is_finite() { y } else { 0.0 };
        self
    }

    pub(crate) fn tile_position(&self, frame: Rect, gx: usize, gy: usize) -> Point {
        Point::new(
            frame.x + gx as f32 * self.gap_x + self.x_offset,
            frame.y + gy as f32 * self.gap_y + self.y_offset,
        )
    }

    fn paint_rotated_text(&self, ctx: &mut PaintContext, origin: Point, color: Color) {
        let angle = self.rotate.to_radians();
        let (sin_a, cos_a) = angle.sin_cos();
        let line_height = self.font_size * 1.4;
        for (line_index, line) in self.text.split('\n').enumerate() {
            let normal_offset = line_index as f32 * line_height;
            let line_origin = Point::new(
                origin.x - sin_a * normal_offset,
                origin.y + cos_a * normal_offset,
            );
            let mut advance = 0.0;
            for character in line.chars() {
                let glyph = character.to_string();
                let position = Self::rotated_advance(line_origin, advance, sin_a, cos_a);
                ctx.draw_text(&glyph, position, color, self.font_size);
                advance += ctx.measure_text(&glyph, self.font_size).w;
            }
        }
    }

    fn rotated_advance(origin: Point, advance: f32, sin_a: f32, cos_a: f32) -> Point {
        Point::new(origin.x + cos_a * advance, origin.y + sin_a * advance)
    }

    fn effective_color(&self) -> Color {
        let alpha = (self.color.a as f32 * self.opacity)
            .round()
            .clamp(0.0, 255.0) as u8;
        self.color.with_alpha(alpha)
    }

    fn positive_or(value: f32, fallback: f32) -> f32 {
        if value.is_finite() && value >= 1.0 {
            value
        } else {
            fallback
        }
    }

    // 测试目标保留水印有效颜色观测入口，供主题样式测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn effective_color_for_test(&self) -> Color {
        self.effective_color()
    }

    // 测试目标保留水印旋转 advance 观测入口，供排版几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn rotated_advance_for_test(&self, origin: Point, advance: f32) -> Point {
        let (sin_a, cos_a) = self.rotate.to_radians().sin_cos();
        Self::rotated_advance(origin, advance, sin_a, cos_a)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.color = next.color;
        self.font_size = next.font_size;
        self.opacity = next.opacity;
        self.rotate = next.rotate;
        self.gap_x = next.gap_x;
        self.gap_y = next.gap_y;
        self.x_offset = next.x_offset;
        self.y_offset = next.y_offset;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Watermark {
            text: self.text.clone(),
            color: self.color,
            font_size: self.font_size,
            opacity: self.opacity,
            rotate: self.rotate,
            gap_x: self.gap_x,
            gap_y: self.gap_y,
            x_offset: self.x_offset,
            y_offset: self.y_offset,
        }
    }
}
