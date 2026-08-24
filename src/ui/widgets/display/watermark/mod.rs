use crate::core::{Constraints, Point, Rect, Size};
use std::sync::OnceLock;

use crate::draw::{Color, Transform};
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::ColorValue;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{SnapshotFields, ThemeTokens, WidgetTree};
use crate::widget;

// 保存由 UIX 声明、由 Rust 平铺与绘制算法消费的默认几何。
#[derive(Debug, Clone, Copy, PartialEq)]
struct WatermarkTilingVisual {
    default_opacity: f32,
    default_rotate: f32,
    default_gap_x: f32,
    default_gap_y: f32,
    default_offset_x: f32,
    default_offset_y: f32,
    overscan_tiles: u8,
}

// 水印默认字号使用的主题排版角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatermarkFontRole {
    Body,
}

impl WatermarkFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Body => tokens.font_size(),
        }
    }
}

// 保存由 UIX 声明、由 Rust 文本布局消费的排版参数。
#[derive(Debug, Clone, Copy, PartialEq)]
struct WatermarkTypographyVisual {
    font: WatermarkFontRole,
    line_height_ratio: f32,
}

// 完整视觉配置由全部 Watermark 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
struct WatermarkVisual {
    tiling: WatermarkTilingVisual,
    typography: WatermarkTypographyVisual,
    text_color: ColorValue,
}

// 组合 UIX 声明的平铺间距、偏移、透明度与旋转角。
#[allow(clippy::too_many_arguments)]
const fn watermark_tiling(
    default_opacity: f32,
    default_rotate: f32,
    default_gap_x: f32,
    default_gap_y: f32,
    default_offset_x: f32,
    default_offset_y: f32,
    overscan_tiles: f32,
) -> WatermarkTilingVisual {
    WatermarkTilingVisual {
        default_opacity,
        default_rotate,
        default_gap_x,
        default_gap_y,
        default_offset_x,
        default_offset_y,
        overscan_tiles: overscan_tiles as u8,
    }
}

// 组合 UIX 声明的主题字号角色与行高比例。
const fn watermark_typography(
    font: WatermarkFontRole,
    line_height_ratio: f32,
) -> WatermarkTypographyVisual {
    WatermarkTypographyVisual {
        font,
        line_height_ratio,
    }
}

// 组合 UIX 声明的完整水印视觉配置。
const fn watermark_visual(
    tiling: WatermarkTilingVisual,
    typography: WatermarkTypographyVisual,
    text_color: ColorValue,
) -> WatermarkVisual {
    WatermarkVisual {
        tiling,
        typography,
        text_color,
    }
}

// 向 UIX 提供水印正文主题字号角色。
const fn watermark_body_font() -> WatermarkFontRole {
    WatermarkFontRole::Body
}

// 向 UIX 提供水印正文主题颜色角色。
const fn watermark_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

// Rust 直接构造或绕过 View 声明根时保持既有视觉；正常 View 构建会改用 UIX 静态配置。
static DEFAULT_WATERMARK_VISUAL: WatermarkVisual = watermark_visual(
    watermark_tiling(0.15, -22.0, 200.0, 160.0, 0.0, 0.0, 2.0),
    watermark_typography(watermark_body_font(), 1.4),
    watermark_text_color(),
);

// 首次 UIX 构建固化声明值，后续实例共享同一份只读视觉配置。
static UIX_WATERMARK_VISUAL: OnceLock<WatermarkVisual> = OnceLock::new();

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
        #[snapshot(skip)]
        opacity_authored: bool,
        rotate: f32,
        #[snapshot(skip)]
        rotate_authored: bool,
        gap_x: f32,
        #[snapshot(skip)]
        gap_x_authored: bool,
        gap_y: f32,
        #[snapshot(skip)]
        gap_y_authored: bool,
        x_offset: f32,
        #[snapshot(skip)]
        x_offset_authored: bool,
        y_offset: f32,
        #[snapshot(skip)]
        y_offset_authored: bool,
        #[snapshot(skip)]
        visual: &'static WatermarkVisual,
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
        let color = self.effective_color(self.visual.text_color.resolve(ctx.tokens()));
        // 默认字号在拥有主题上下文的绘制阶段解析。
        let font_size = self.resolved_font_size(ctx.tokens());
        // 完全透明的水印没有可见输出，也无需生成文字绘制命令。
        if color.a == 0 {
            return;
        }
        let overscan = usize::from(self.visual.tiling.overscan_tiles);
        let columns = (frame.w.max(0.0) / self.gap_x).ceil() as usize + overscan;
        let rows = (frame.h.max(0.0) / self.gap_y).ceil() as usize + overscan;
        // 全部平铺实例共享同一角度，每帧只构造一次旋转矩阵。
        let rotation = Transform::rotate(self.rotate.to_radians());

        ctx.push_clip(frame);
        // 每个实例按完整文本行绘制，保留字距、连字、双向文本与字体回退整形。
        self.paint_tiled_text(ctx, frame, columns, rows, color, font_size, rotation);
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
fn build_watermark_view(mut kernel: Watermark, declared_visual: WatermarkVisual) -> ViewNode {
    let visual = UIX_WATERMARK_VISUAL.get_or_init(|| declared_visual);
    if !kernel.opacity_authored {
        kernel.opacity = visual.tiling.default_opacity;
    }
    if !kernel.rotate_authored {
        kernel.rotate = visual.tiling.default_rotate;
    }
    if !kernel.gap_x_authored {
        kernel.gap_x = visual.tiling.default_gap_x;
    }
    if !kernel.gap_y_authored {
        kernel.gap_y = visual.tiling.default_gap_y;
    }
    if !kernel.x_offset_authored {
        kernel.x_offset = visual.tiling.default_offset_x;
    }
    if !kernel.y_offset_authored {
        kernel.y_offset = visual.tiling.default_offset_y;
    }
    kernel.visual = visual;
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
    /// 创建使用主题文本样式与默认平铺参数的文字水印。
    pub fn new(text: &str) -> Self {
        let visual = &DEFAULT_WATERMARK_VISUAL;
        Self {
            text: text.to_string(),
            // 透明哨兵表示绘制时使用当前主题正文色。
            color: Color::TRANSPARENT,
            // 零值哨兵表示绘制时使用当前主题正文字号。
            font_size: 0.0,
            opacity: visual.tiling.default_opacity,
            opacity_authored: false,
            rotate: visual.tiling.default_rotate,
            rotate_authored: false,
            gap_x: visual.tiling.default_gap_x,
            gap_x_authored: false,
            gap_y: visual.tiling.default_gap_y,
            gap_y_authored: false,
            x_offset: visual.tiling.default_offset_x,
            x_offset_authored: false,
            y_offset: visual.tiling.default_offset_y,
            y_offset_authored: false,
            visual,
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
        if o.is_finite() {
            self.opacity = o.clamp(0.0, 1.0);
            self.opacity_authored = true;
        } else {
            self.opacity = DEFAULT_WATERMARK_VISUAL.tiling.default_opacity;
            self.opacity_authored = false;
        }
        self
    }
    /// 设置水印文字旋转角度；非有限值回退默认角度。
    pub fn rotate(mut self, r: f32) -> Self {
        if r.is_finite() {
            self.rotate = r;
            self.rotate_authored = true;
        } else {
            self.rotate = DEFAULT_WATERMARK_VISUAL.tiling.default_rotate;
            self.rotate_authored = false;
        }
        self
    }
    /// 设置相邻水印的水平与垂直间距；非法分量分别回退默认间距。
    pub fn gap(mut self, x: f32, y: f32) -> Self {
        self.gap_x_authored = x.is_finite() && x >= 1.0;
        self.gap_y_authored = y.is_finite() && y >= 1.0;
        self.gap_x = Self::positive_or(x, DEFAULT_WATERMARK_VISUAL.tiling.default_gap_x);
        self.gap_y = Self::positive_or(y, DEFAULT_WATERMARK_VISUAL.tiling.default_gap_y);
        self
    }
    /// 设置平铺起点偏移；非有限分量分别回退为零。
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.x_offset_authored = x.is_finite();
        self.y_offset_authored = y.is_finite();
        self.x_offset = if x.is_finite() {
            x
        } else {
            DEFAULT_WATERMARK_VISUAL.tiling.default_offset_x
        };
        self.y_offset = if y.is_finite() {
            y
        } else {
            DEFAULT_WATERMARK_VISUAL.tiling.default_offset_y
        };
        self
    }

    pub(crate) fn tile_position(&self, frame: Rect, gx: usize, gy: usize) -> Point {
        Point::new(
            frame.x + gx as f32 * self.gap_x + self.x_offset,
            frame.y + gy as f32 * self.gap_y + self.y_offset,
        )
    }

    fn paint_tiled_text(
        &self,
        ctx: &mut PaintContext,
        frame: Rect,
        columns: usize,
        rows: usize,
        color: Color,
        font_size: f32,
        rotation: Transform,
    ) {
        // 行高从本次绘制解析出的实际字号派生。
        let line_height = font_size * self.visual.typography.line_height_ratio;
        for gy in 0..rows {
            for gx in 0..columns {
                let origin = self.tile_position(frame, gx, gy);
                // 平移与旋转矩阵复用同一份旋转基向量，避免每个字符重复几何运算。
                let tile_transform = Transform::translate(origin.x, origin.y).concat(rotation);
                ctx.save();
                ctx.concat_transform(tile_transform);
                for (line_index, line) in self.text.split('\n').enumerate() {
                    if line.is_empty() {
                        continue;
                    }
                    ctx.draw_text(
                        line,
                        Point::new(0.0, line_index as f32 * line_height),
                        color,
                        font_size,
                    );
                }
                ctx.restore();
            }
        }
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
            self.visual.typography.font.resolve(tokens)
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.color = next.color;
        self.font_size = next.font_size;
        self.opacity = next.opacity;
        self.opacity_authored = next.opacity_authored;
        self.rotate = next.rotate;
        self.rotate_authored = next.rotate_authored;
        self.gap_x = next.gap_x;
        self.gap_x_authored = next.gap_x_authored;
        self.gap_y = next.gap_y;
        self.gap_y_authored = next.gap_y_authored;
        self.x_offset = next.x_offset;
        self.x_offset_authored = next.x_offset_authored;
        self.y_offset = next.y_offset;
        self.y_offset_authored = next.y_offset_authored;
        self.visual = next.visual;
    }

    // 测试目标观察 UIX 声明的共享视觉契约，不暴露到公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, u8) {
        (
            self.visual.tiling.default_opacity,
            self.visual.tiling.default_rotate,
            self.visual.tiling.default_gap_x,
            self.visual.tiling.default_gap_y,
            self.visual.typography.line_height_ratio,
            self.visual.tiling.overscan_tiles,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉配置。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
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
