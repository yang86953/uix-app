//! UI-owned 绘制上下文。
//!
//! UI 组件的 `WidgetRender::render` 只接收本类型：它包装底层
//! [`crate::draw::painting::PaintContext`]，并持有 UI 域的主题令牌作用域
//! （`tokens()` / 组件补丁 scope）。底层 draw 上下文只接收已解析的
//! 颜色、尺寸与绘制值，不再引用任何 UI 主题类型，从而维持
//! `graphics → core / platform` 的单向边界。
//!
//! 绘制命令逐一转发给底层 draw 上下文（不使用 `DerefMut`，避免破坏
//! `ctx.fill_rect(rect, ctx.tokens().color_x(), …)` 这类同表达式二段式
//! 借用）；组件无需感知两层边界。

use std::sync::Arc;

use crate::core::{Point, Rect, Size};
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::spatial::{AABB3D, PhysicalUnit, Vec3};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::painting::PaintContext as DrawPaintContext;
use crate::draw::painting::PaintPass;
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::font::text_backend::TextLayout;
use crate::draw::resources::image::{BitmapHandle, ImageService};
use crate::draw::{BlendMode, Color, FontHandle, GradientDirection, Radius, Transform};
use crate::ui::theme::traits::ThemeTokens;
use crate::ui::theme::{ScopedThemeTokens, TokenPatch, TokenScope};

/// 转发 `&mut self` 绘制方法（固有方法享受二段式借用）。
macro_rules! delegate_mut {
    ($($name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?;)*) => {
        $(
            #[inline(always)]
            #[doc = concat!("将绘制命令 `", stringify!($name), "` 转发到底层绘制上下文。")]
            pub fn $name(&mut self, $($arg: $ty),*) $(-> $ret)? {
                self.inner.$name($($arg),*)
            }
        )*
    };
}

/// 转发 `&self` 查询方法。
macro_rules! delegate_shared {
    ($($name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?;)*) => {
        $(
            #[inline(always)]
            #[doc = concat!("将查询 `", stringify!($name), "` 转发到底层绘制上下文。")]
            pub fn $name(&self, $($arg: $ty),*) $(-> $ret)? {
                self.inner.$name($($arg),*)
            }
        )*
    };
}

/// UI 组件可用的绘制上下文。
///
/// 同一作用域内只允许一个可变借用；`replace_token_scope` /
/// `restore_token_scope` 由 widget 渲染边界负责配对，panic 展开时
/// 不得把补丁泄漏到兄弟节点（见 `Widget::render`）。
/// UI 组件可用的绘制上下文。
///
/// 同一作用域内只允许一个可变借用；`replace_token_scope` /
/// `restore_token_scope` 由 widget 渲染边界负责配对，panic 展开时
/// 不得把补丁泄漏到兄弟节点（见 `Widget::render`）。
pub struct PaintContext<'a, 'b> {
    inner: &'a mut DrawPaintContext<'b>,
    tokens: ScopedThemeTokens,
}

impl<'a, 'b> PaintContext<'a, 'b> {
    /// 包装底层 draw 上下文并注入主题根令牌。
    pub(crate) fn new(inner: &'a mut DrawPaintContext<'b>, tokens: Arc<dyn ThemeTokens>) -> Self {
        Self {
            inner,
            tokens: ScopedThemeTokens::new(tokens),
        }
    }

    /// 获取当前生效的设计令牌（含子树主题与组件补丁作用域）。
    #[inline(always)]
    pub fn tokens(&self) -> &dyn ThemeTokens {
        &self.tokens
    }

    /// 替换当前子树的主题与补丁，返回上一作用域以便恢复。
    pub(crate) fn replace_token_scope(
        &mut self,
        theme: Option<Arc<dyn ThemeTokens>>,
        patch: Option<Arc<TokenPatch>>,
    ) -> TokenScope {
        self.tokens.replace_scope(theme, patch)
    }

    /// 恢复上一作用域。
    pub(crate) fn restore_token_scope(&mut self, scope: TokenScope) {
        self.tokens.restore_scope(scope);
    }

    /// 访问底层 draw 上下文（draw 层回调 / 录制路径使用）。
    pub(crate) fn as_draw_mut(&mut self) -> &mut DrawPaintContext<'b> {
        self.inner
    }

    /// 在闭包内应用透明度；闭包接收底层 draw 上下文（令牌需先解析）。
    pub fn with_opacity<R>(
        &mut self,
        opacity: f32,
        draw: impl FnOnce(&mut DrawPaintContext) -> R,
    ) -> R {
        self.inner.with_opacity(opacity, draw)
    }

    delegate_shared! {
        paint_pass() -> PaintPass;
        dpi() -> f32;
        surface_size() -> Size;
        image_service() -> &ImageService;
        font() -> &FontHandle;
        debug_mode() -> bool;
        recording_complete() -> bool;
    }

    delegate_mut! {
        set_paint_pass(pass: PaintPass);
        is_rect_visible(rect: Rect) -> bool;
        font_service() -> &FontService;
        fill_rect(rect: Rect, color: Color, radius: Option<Radius>);
        fill_rounded_rect(rect: Rect, color: Color, radius: Radius);
        fill_circle(cx: f32, cy: f32, r: f32, color: Color);
        draw_point(point: Point, color: Color, diameter: f32);
        fill_ellipse(rect: Rect, color: Color);
        fill_sector(cx: f32, cy: f32, r: f32, start_angle: f32, end_angle: f32, color: Color);
        fill_path(path: &Path, color: Color, fill_rule: FillRule);
        fill_polygon(points: &[Point], color: Color, fill_rule: FillRule);
        stroke_rect(rect: Rect, color: Color, line_width: f32, radius: Option<Radius>);
        stroke_rounded_rect(rect: Rect, color: Color, line_width: f32, radius: Radius);
        stroke_circle(cx: f32, cy: f32, r: f32, color: Color, line_width: f32);
        stroke_arc(cx: f32, cy: f32, r: f32, start_angle: f32, end_angle: f32, color: Color, line_width: f32);
        stroke_path(path: &Path, color: Color, opts: &StrokeOptions);
        stroke_polyline(points: &[Point], color: Color, options: &StrokeOptions);
        stroke_polygon(points: &[Point], color: Color, options: &StrokeOptions);
        stroke_ellipse(rect: Rect, color: Color, options: &StrokeOptions);
        draw_line(x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32);
        fill_linear_gradient(rect: Rect, color_a: Color, color_b: Color, dir: GradientDirection);
        fill_radial_gradient(cx: f32, cy: f32, inner_r: f32, outer_r: f32, inner_color: Color, outer_color: Color);
        draw_box_shadow(rect: Rect, blur_radius: f32, offset_x: f32, offset_y: f32, color: Color, corner_radius: Option<Radius>);
        draw_box_shadow_ambient(rect: Rect, blur_radius: f32, offset_x: f32, offset_y: f32, color: Color, corner_radius: Option<Radius>);
        translate(dx: f32, dy: f32);
        current_transform() -> Transform;
        set_transform(transform: Transform);
        concat_transform(transform: Transform);
        set_opacity(opacity: f32);
        set_blend_mode(mode: BlendMode);
        save();
        restore();
        push_clip(rect: Rect);
        push_clip_path(path: &Path);
        pop_clip();
        draw_text_spatial(text: &str, pos: Vec3, color: Color, font_size: PhysicalUnit);
        text_center_spatial(text: &str, box_3d: AABB3D, color: Color, font_size: PhysicalUnit);
        draw_text(text: &str, pos: Point, color: Color, font_size: f32);
        draw_text_baseline(text: &str, x: f32, baseline_y: f32, color: Color, font_size: f32);
        text_center(text: &str, rect: Rect, color: Color, font_size: f32);
        draw_text_in_frame(text: &str, rect: Rect, color: Color, font_size: f32);
        draw_text_wrapped(text: &str, rect: Rect, color: Color, font_size: f32);
        draw_text_with_selection(text: &str, pos: Point, color: Color, font_size: f32, selection: Option<(usize, usize)>, selection_bg: Color);
        selection_rects(text: &str, font_size: f32, pos: Point, start: usize, end: usize) -> Vec<Rect>;
        measure_text(text: &str, font_size: f32) -> Size;
        line_box_height(font_size: f32) -> f32;
        measure_text_wrapped(text: &str, font_size: f32, max_width: f32) -> Size;
        text_hit_test(text: &str, font_size: f32, point: Point) -> Option<usize>;
        text_cursor_x(text: &str, font_size: f32, char_index: usize) -> f32;
        visual_center_y(rect: Rect, font_size: f32) -> f32;
        set_max_text_width(width: f32);
        blit_glyph_layout(layout: &TextLayout, pos: Point, color: Color, font_size: f32);
        draw_image(handle: BitmapHandle, bounds: Rect);
        draw_image_fill(handle: BitmapHandle, bounds: Rect);
        set_font(font: FontHandle);
        set_debug_mode(mode: bool);
        draw_debug_border(rect: Rect, depth: usize, hovered: bool);
        draw_debug_label(node_slot: usize, depth: usize, rect: Rect);
        draw_debug_frame_info(node_slot: usize, rect: Rect);
    }

    /// 当前绘制表面的设备像素比（UI 内部使用）。
    pub(crate) fn device_pixel_ratio(&self) -> f32 {
        self.inner.device_pixel_ratio()
    }

    /// 当前绘制表面的逻辑尺寸（UI 内部使用）。
    pub(crate) fn logical_surface_size(&self) -> Size {
        self.inner.logical_surface_size()
    }

    /// 绘制并录制 owned glyph layout（UI 内部使用）。
    pub(crate) fn blit_owned_glyph_layout(
        &mut self,
        layout: TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        self.inner
            .blit_owned_glyph_layout(layout, pos, color, font_size);
    }
}
