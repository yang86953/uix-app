use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{SnapshotFields, ThemeTokens, WidgetTree};
use crate::widget;

// ════════════════════════════════════════════════════════════════════════════
// Watermark
// ════════════════════════════════════════════════════════════════════════════

widget! {
    /// 在组件区域内按配置间距、偏移与角度平铺文字的水印组件。
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
        // 默认颜色在拥有主题上下文的绘制阶段解析。
        let color = self.effective_color(ctx.tokens().color_text());
        // 默认字号在拥有主题上下文的绘制阶段解析。
        let font_size = self.resolved_font_size(ctx.tokens());
        let columns = (frame.w.max(0.0) / self.gap_x).ceil() as usize + 2;
        let rows = (frame.h.max(0.0) / self.gap_y).ceil() as usize + 2;

        ctx.push_clip(frame);
        for gy in 0..rows {
            for gx in 0..columns {
                // 平铺文字复用本次绘制解析出的主题字号与颜色。
                self.paint_rotated_text(ctx, self.tile_position(frame, gx, gy), color, font_size);
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

// 把水印平铺配置与绘制内核融合为 UIX 声明的单一叶节点。
fn build_watermark_view(kernel: Watermark) -> ViewNode {
    ViewNode::leaf(kernel)
}

impl View for Watermark {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根，Rust 内核继续独占平铺、旋转排版与绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/watermark/watermark.uix")
    }
}

impl Watermark {
    const DEFAULT_OPACITY: f32 = 0.15;
    const DEFAULT_ROTATE: f32 = -22.0;
    const DEFAULT_GAP_X: f32 = 200.0;
    const DEFAULT_GAP_Y: f32 = 160.0;

    /// 创建使用主题文本样式与默认平铺参数的文字水印。
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            // 透明哨兵表示绘制时使用当前主题正文色。
            color: Color::TRANSPARENT,
            // 零值哨兵表示绘制时使用当前主题正文字号。
            font_size: 0.0,
            opacity: Self::DEFAULT_OPACITY,
            rotate: Self::DEFAULT_ROTATE,
            gap_x: Self::DEFAULT_GAP_X,
            gap_y: Self::DEFAULT_GAP_Y,
            x_offset: 0.0,
            y_offset: 0.0,
        }
    }
    /// 设置水印文字颜色；透明色表示绘制时使用主题正文色。
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
    /// 设置水印字号；非正数或非有限值回退到主题正文字号。
    pub fn font_size(mut self, s: f32) -> Self {
        // 非法显式字号回退到主题默认哨兵。
        self.font_size = Self::positive_or(s, 0.0);
        self
    }
    /// 设置水印不透明度，有限值会夹紧到 `0..=1`，非法值回退默认值。
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = if o.is_finite() {
            o.clamp(0.0, 1.0)
        } else {
            Self::DEFAULT_OPACITY
        };
        self
    }
    /// 设置水印文字旋转角度；非有限值回退默认角度。
    pub fn rotate(mut self, r: f32) -> Self {
        self.rotate = if r.is_finite() {
            r
        } else {
            Self::DEFAULT_ROTATE
        };
        self
    }
    /// 设置相邻水印的水平与垂直间距；非法分量分别回退默认间距。
    pub fn gap(mut self, x: f32, y: f32) -> Self {
        self.gap_x = Self::positive_or(x, Self::DEFAULT_GAP_X);
        self.gap_y = Self::positive_or(y, Self::DEFAULT_GAP_Y);
        self
    }
    /// 设置平铺起点偏移；非有限分量分别回退为零。
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

    fn paint_rotated_text(
        &self,
        ctx: &mut PaintContext,
        origin: Point,
        color: Color,
        font_size: f32,
    ) {
        let angle = self.rotate.to_radians();
        let (sin_a, cos_a) = angle.sin_cos();
        // 行高从本次绘制解析出的实际字号派生。
        let line_height = font_size * 1.4;
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
                // 使用主题解析后的实际字号绘制当前字符。
                ctx.draw_text(&glyph, position, color, font_size);
                // 度量与绘制必须共享相同字号。
                advance += ctx.measure_text(&glyph, font_size).w;
            }
        }
    }

    fn rotated_advance(origin: Point, advance: f32, sin_a: f32, cos_a: f32) -> Point {
        Point::new(origin.x + cos_a * advance, origin.y + sin_a * advance)
    }

    // 解析默认主题色并应用作者透明度。
    fn effective_color(&self, theme_text: Color) -> Color {
        // 透明哨兵表示调用方没有显式覆写水印颜色。
        let base = if self.color == Color::TRANSPARENT {
            // 默认路径使用当前主题正文色。
            theme_text
        } else {
            // 显式颜色保持最高优先级。
            self.color
        };
        // 在基础颜色 alpha 上叠加作者 opacity。
        let alpha = (base.a as f32 * self.opacity).round().clamp(0.0, 255.0) as u8;
        // 返回保留基础 RGB 的最终绘制颜色。
        base.with_alpha(alpha)
    }

    // 解析默认主题字号。
    fn resolved_font_size(&self, tokens: &dyn ThemeTokens) -> f32 {
        // 正值表示调用方已经显式覆写字号。
        if self.font_size >= 1.0 {
            // 返回显式作者字号。
            self.font_size
        } else {
            // 默认路径使用当前主题正文字号。
            tokens.font_size()
        }
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
    pub(crate) fn effective_color_for_test(&self, theme_text: Color) -> Color {
        // 测试入口复用生产颜色解析。
        self.effective_color(theme_text)
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

// 验证 Watermark 默认样式随主题解析。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/other/misc/watermark__theme_tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod theme_tests;
