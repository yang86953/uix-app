/// 帧渲染结果。
#[derive(Debug, Clone, PartialEq)]
pub enum RenderOutcome {
    /// 零帧开销，未渲染（无需呈现）
    Idle,
    /// 已渲染，需呈现。`None` = 全屏，`Some((x,y,w,h))` = 局部损伤矩形
    Present(Option<(i32, i32, i32, i32)>),
}

use std::cell::RefCell;

use crate::base::{Rect, Size};
use crate::diag::Error;
use crate::graphics::types::*;
use crate::graphics::{Color, DirtyRegion};
use crate::ui::theme::Theme;
use crate::ui::widget::WidgetTree;

/// GraphicsEngine — abstract 2D rendering interface.
///
/// Design principle: pure renderer — no platform dependencies.
/// File I/O is the caller's responsibility (pass `&[u8]` for assets).
/// Presentation is handled externally (e.g. `GdiPresenter`).
pub trait GraphicsEngine: 'static {
    /// 用于向下转型到具体引擎类型。
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    // Lifetime
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error>;
    fn shutdown(&mut self);
    fn resize(&mut self, width: i32, height: i32);

    // Frame control
    fn begin_frame(&mut self, dirty: &DirtyRegion);
    fn end_frame(&mut self, dirty: &DirtyRegion);

    /// Scroll (shift) the pixel content within `viewport` by `dy` pixels.
    ///
    /// 滚动优化：将 viewport 内的像素缓冲上/下移动 dy 像素（memmove），
    /// 再结合局部 dirty region 只渲染新增 strip。
    /// `dy > 0` = 滚动向下（内容上移），`dy < 0` = 滚动向上（内容下移）。
    fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32);

    // Clip & state
    fn push_clip_rect(&mut self, rect: Rect);
    fn pop_clip_rect(&mut self);
    fn set_opacity(&mut self, opacity: f32);
    fn opacity(&self) -> f32;
    fn save(&mut self);
    fn restore(&mut self);
    fn set_transform(&mut self, t: Transform);
    fn reset_transform(&mut self);
    fn set_blend_mode(&mut self, mode: BlendMode);

    // Shapes
    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>);
    fn stroke_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Option<Radius>);
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color);
    fn fill_circle_radial(&mut self, cx: f32, cy: f32, r: f32, color: Color);
    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32);
    fn fill_sector(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
    );
    fn fill_ellipse(&mut self, rect: Rect, color: Color);
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32);

    // Shadows
    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    );

    /// Ambient box shadow — wider, softer falloff for ambient layers.
    fn draw_box_shadow_ambient(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    );

    // Path — 任意形状渲染
    fn fill_path(
        &mut self,
        _path: &crate::graphics::path::Path,
        _color: Color,
        _fill_rule: crate::graphics::path::FillRule,
    ) {
    }
    fn stroke_path(
        &mut self,
        _path: &crate::graphics::path::Path,
        _color: Color,
        _options: &crate::graphics::stroker::StrokeOptions,
    ) {
    }

    // Gradients
    fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
    );
    fn fill_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        inner_r: f32,
        outer_r: f32,
        inner_color: Color,
        outer_color: Color,
    );

    // Text —
    // - Layout / font-loading / glyph-rasterization: FontService (standalone, renderer-independent)
    // - Glyph pixel drawing: draw_glyph_raster (engine must implement)
    // - Text measurement (for layout only): measure_text (default returns zero)

    /// 测量文本尺寸（仅用于布局阶段辅助计算首选尺寸）。
    /// 渲染阶段的文本布局和绘制由 `FontService` + `draw_glyph_raster` 完成。
    /// 引擎只需要实现 `draw_glyph_raster`，此方法有默认实现。
    fn measure_text(&self, _font: &FontHandle, _text: &str, _opts: &TextLayoutOptions) -> Size {
        Size::zero()
    }

    /// 绘制一个已光栅化的字形（coverage bitmap）。
    /// `x`, `y` 是目标位置的像素坐标，`color` 是预乘 ARGB 颜色。
    /// `coverage` 是 `width * height` 的逐像素覆盖值数组（0-255）。
    fn draw_glyph_raster(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        width: usize,
        height: usize,
        color: Color,
    );

    // Images — caller responsible for reading file bytes
    fn load_image(&mut self, data: &[u8]) -> Result<&mut ImageHandle, Error>;
    fn unload_image(&mut self, image: &ImageHandle);
    fn image_size(&self, image: &ImageHandle) -> Size;
    fn draw_image(&mut self, image: &ImageHandle, src: Rect, dst: Rect);

    // Offscreen
    fn create_offscreen(&mut self, w: i32, h: i32) -> Result<&mut ImageHandle, Error>;
    fn destroy_offscreen(&mut self, offscreen: &ImageHandle);
    fn begin_offscreen(&mut self, offscreen: &ImageHandle);
    fn end_offscreen(&mut self);

    // Query
    fn pixels(&self) -> &[u32];
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    // Supersample control for software renderer (0 = adaptive/default).
    fn set_supersample_level(&mut self, _level: u8) {}
    fn supersample_level(&self) -> u8 {
        0
    }

    /// 执行一帧的完整渲染循环。
    ///
    /// 引擎内部处理：dirty_region 读取、FrameGraph Pass 编排、
    /// begin_frame/end_frame、scroll_region、布局计算、
    /// render_geometry + render_overlays 遍历、reset_dirty。
    ///
    /// 返回值：
    /// - `RenderOutcome::Idle` — 零帧开销，未渲染
    /// - `RenderOutcome::Present(None)` — 渲染了全屏
    /// - `RenderOutcome::Present(Some((x,y,w,h)))` — 渲染了局部损伤
    fn render_frame(
        &mut self,
        tree: &mut WidgetTree,
        theme: &RefCell<Theme>,
        first_frame: bool,
        keep_polling: bool,
    ) -> RenderOutcome;
}
