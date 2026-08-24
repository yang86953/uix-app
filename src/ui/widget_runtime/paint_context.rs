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

use std::borrow::Cow;
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

// 定义所有 UI 单行截断共享的 Unicode 省略号。
const TEXT_ELLIPSIS: &str = "…";

// 合并真实字体宽度与无字体估算宽度，始终返回更保守的结果。
fn conservative_width_from_measurement(measured_width: f32, text: &str, font_size: f32) -> f32 {
    // 计算不依赖已加载字体的稳定后备宽度。
    let estimated_width = crate::draw::resources::font::text_backend::estimate_text_metrics(
        text,
        f32::INFINITY,
        font_size,
    )
    .max_line_width;
    // 防止真实字体缺失或低估时压缩组件内容。
    measured_width.max(estimated_width)
    // 结束保守宽度合并。
}

// 对已经不含换行的文本执行 Unicode 安全单行省略。
fn elide_normalized_single_line_by<'a>(
    text: &'a str,
    max_width: f32,
    mut text_width: impl FnMut(&str) -> f32,
) -> Option<Cow<'a, str>> {
    // 非有限或非正可用宽度无法显示任何文本。
    if !max_width.is_finite() || max_width <= 0.0 {
        // 返回空值让组件保持自身的不绘制策略。
        return None;
        // 结束无可用宽度分支。
    }
    // 窄入口只接受已经完成换行规范化的文本。
    debug_assert!(!text.contains(['\r', '\n']));
    // 完整文本能容纳时避免不必要的分配和省略。
    if text_width(text) <= max_width {
        // 常见完整容纳路径直接借用调用方文本，不分配临时 String。
        return Some(Cow::Borrowed(text));
        // 结束完整文本分支。
    }
    // 极窄宽度连省略号也无法容纳时不绘制文本。
    if text_width(TEXT_ELLIPSIS) > max_width {
        // 返回空值避免绘制越界省略号。
        return None;
        // 结束省略号宽度门禁。
    }
    // 按 Unicode 标量逐步构建可见前缀。
    let mut visible = String::new();
    // 遍历规范化文本中的每个 Unicode 字符。
    for character in text.chars() {
        // 暂时加入当前候选字符。
        visible.push(character);
        // 暂时附加省略号以度量最终显示宽度。
        visible.push_str(TEXT_ELLIPSIS);
        // 记录当前前缀加省略号是否仍可容纳。
        let fits = text_width(&visible) <= max_width;
        // 移除临时省略号以继续构建前缀。
        visible.pop();
        // 首个无法容纳的字符不属于最终可见前缀。
        if !fits {
            // 移除刚加入且无法容纳的 Unicode 字符。
            visible.pop();
            // 后续字符不可能恢复单调宽度，停止搜索。
            break;
            // 结束当前字符无法容纳分支。
        }
        // 结束 Unicode 前缀搜索。
    }
    // 为最终可见前缀追加唯一省略号。
    visible.push_str(TEXT_ELLIPSIS);
    // 返回保持在宽度边界内的单行结果。
    Some(Cow::Owned(visible))
    // 结束共享单行省略算法。
}

// 使用调用方提供的保守度量函数规范化换行并执行单行省略。
fn elide_single_line_cow_by<'a>(
    text: &'a str,
    max_width: f32,
    mut text_width: impl FnMut(&str) -> f32,
) -> Option<Cow<'a, str>> {
    // 常见无换行文本直接进入借用型窄入口，完整容纳时不分配 String。
    if !text.contains(['\r', '\n']) {
        return elide_normalized_single_line_by(text, max_width, text_width);
    }
    // 存在换行时只做一次规范化；返回值必须取得所有权以离开局部缓冲区。
    let normalized = text.replace(['\r', '\n'], " ");
    elide_normalized_single_line_by(&normalized, max_width, &mut text_width)
        .map(|value| Cow::Owned(value.into_owned()))
}

// 保留旧拥有型入口，现有调用方继续取得 String。
fn elide_single_line_by(
    text: &str,
    max_width: f32,
    text_width: impl FnMut(&str) -> f32,
) -> Option<String> {
    elide_single_line_cow_by(text, max_width, text_width).map(Cow::into_owned)
}

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

    /// 返回真实字体度量与无字体估算中的较大文本宽度。
    pub(crate) fn conservative_text_width(&mut self, text: &str, font_size: f32) -> f32 {
        // 从当前字体后端获取真实测量宽度。
        let measured_width = self.measure_text(text, font_size).w;
        // 与稳定后备估算合并，保留字体缺失时的布局语义。
        conservative_width_from_measurement(measured_width, text, font_size)
        // 结束 UI 保守文本宽度查询。
    }

    /// 将换行规范化为空格，并按保守宽度生成可选的单行省略文本。
    pub(crate) fn elide_single_line(
        &mut self,
        text: &str,
        font_size: f32,
        max_width: f32,
    ) -> Option<String> {
        // 复用当前上下文的真实字体与后备估算组合度量。
        elide_single_line_by(text, max_width, |candidate| {
            self.conservative_text_width(candidate, font_size)
        })
        // 结束 UI 单行省略入口。
    }

    /// 规范化换行并执行单行省略；无换行且完整容纳时直接借用输入。
    pub(crate) fn elide_single_line_cow<'c>(
        &mut self,
        text: &'c str,
        font_size: f32,
        max_width: f32,
    ) -> Option<Cow<'c, str>> {
        elide_single_line_cow_by(text, max_width, |candidate| {
            self.conservative_text_width(candidate, font_size)
        })
    }

    /// 对调用方已经规范化换行的文本执行保守单行省略。
    pub(crate) fn elide_normalized_single_line(
        &mut self,
        text: &str,
        font_size: f32,
        max_width: f32,
    ) -> Option<String> {
        // 调用方已承担唯一一次换行替换，此处只执行宽度门禁与 Unicode 截断。
        self.elide_normalized_single_line_cow(text, font_size, max_width)
            .map(Cow::into_owned)
    }

    /// 对已规范化文本执行单行省略，完整容纳时直接借用输入。
    pub(crate) fn elide_normalized_single_line_cow<'c>(
        &mut self,
        text: &'c str,
        font_size: f32,
        max_width: f32,
    ) -> Option<Cow<'c, str>> {
        elide_normalized_single_line_by(text, max_width, |candidate| {
            self.conservative_text_width(candidate, font_size)
        })
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
        fill_text_selection(text: &str, font_size: f32, pos: Point, start: usize, end: usize, color: Color);
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

    /// 返回映射到当前组件绘制坐标的逻辑表面矩形。
    pub(crate) fn logical_surface_rect(&mut self) -> Rect {
        self.inner.logical_surface_rect()
    }

    /// 绘制并录制共享 glyph layout（UI 内部使用）。
    pub(crate) fn blit_shared_glyph_layout(
        &mut self,
        layout: Arc<TextLayout>,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        self.inner
            .blit_shared_glyph_layout(layout, pos, color, font_size);
    }
}

// 只在测试构建中编译共享文本度量契约。
#[cfg(test)]
// 将纯算法测试限制在拥有实现的 UI widget Module 内。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/ui/widget_runtime/paint_context__text_measure_tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod text_measure_tests;
