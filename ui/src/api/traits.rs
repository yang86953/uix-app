//! # uix-ui 行为接口（契约定义）
//!
//! 本模块定义 ui 层的所有公开 trait 接口。
//! 实现保留在各领域内部模块中，通过本模块统一暴露契约。

use uix_graphics::painting::PaintContext as RenderContext;
use crate::style::Style;
use crate::widget::{EventResult, WidgetEvent, WidgetId, WidgetNode, WidgetTree};
use std::any::Any;
use uix_graphics::spatial::{Ray3D, SpatialContext};
use uix_graphics::{Color, GraphicsEngine};
use uix_platform::{EdgeInsets, Point, Rect, Size};

// ════════════════════════════════════════════════════════════════════════════
// Widget 组件体系（无上帝接口，每个能力是独立 trait）
// ════════════════════════════════════════════════════════════════════════════

/// 能力位标记。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetCapabilities(u8);

impl WidgetCapabilities {
    pub const LAYOUT: u8 = 0b0001;
    pub const RENDER: u8 = 0b0010;
    pub const EVENT: u8 = 0b0100;
    pub const LIFECYCLE: u8 = 0b1000;

    pub const fn new() -> Self {
        Self(0)
    }
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
    pub fn insert(&mut self, cap: u8) {
        self.0 |= cap;
    }
    pub fn contains(&self, cap: u8) -> bool {
        self.0 & cap != 0
    }
    pub fn bits(&self) -> u8 {
        self.0
    }
}

/// 组件核心标识 — 所有 widget 必须实现。
pub trait WidgetComponent: 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// 返回此 widget 实现了哪些能力。
    fn capabilities(&self) -> WidgetCapabilities;
    /// 返回当前 widget 的内部可见性。
    fn visible(&self) -> bool {
        true
    }
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
        vec![]
    }
    /// 默认 Tab 键导航索引（> 0 表示组件默认可通过 Tab 聚焦）。
    /// 应用层可通过 `WidgetNode::tab_index()` 覆盖。
    fn tab_index(&self) -> i32 {
        0
    }

    // ── 可选能力上转型（宏自动生成） ──
    fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        None
    }
    fn as_render(&self) -> Option<&dyn WidgetRender> {
        None
    }
    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        None
    }
    fn as_event(&self) -> Option<&dyn WidgetEventHandler> {
        None
    }
    fn as_event_mut(&mut self) -> Option<&mut dyn WidgetEventHandler> {
        None
    }
    fn as_lifecycle(&self) -> Option<&dyn WidgetLifecycle> {
        None
    }
    fn as_lifecycle_mut(&mut self) -> Option<&mut dyn WidgetLifecycle> {
        None
    }
}

/// 布局行为：尺寸、弹性、子节点排列。
pub trait WidgetLayout: WidgetComponent {
    fn preferred_size(&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::zero()
    }
    fn flex_grow(&self) -> f32 {
        0.0
    }
    fn flex_shrink(&self) -> f32 {
        0.0
    }
    fn layout_children(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        let _ = (frame, children, tree);
        Vec::new()
    }
}

/// 渲染行为：绘制、覆盖层、脏区域。
pub trait WidgetRender: WidgetComponent {
    fn render(&self, frame: Rect, ctx: &mut RenderContext, tree: &WidgetTree);
    fn post_render(&self, _frame: Rect, _ctx: &mut RenderContext, _tree: &WidgetTree) {}
    fn draw_margin(&self) -> f32 {
        0.0
    }
    fn dirty_rect(&self, frame: Rect) -> Rect {
        let m = self.draw_margin();
        if m > 0.0 {
            Rect::new(
                frame.x - m,
                frame.y - m,
                frame.w + m * 2.0,
                frame.h + m * 2.0,
            )
        } else {
            frame
        }
    }
    fn is_repaint_boundary(&self) -> bool {
        false
    }
    fn children_clip(&self, _frame: Rect) -> Option<Rect> {
        None
    }
}

/// 事件行为：输入事件处理、持续更新、滚动偏移、命中测试。
pub trait WidgetEventHandler: WidgetComponent {
    fn on_event(&mut self, _event: &WidgetEvent) -> EventResult {
        EventResult::NotHandled
    }
    fn needs_continuous_update(&self) -> bool {
        false
    }
    fn scroll_delta(&self, _frame: Rect) -> Option<(f32, f32)> {
        None
    }
    /// 帧间滚动偏移（用于 scroll_region 像素移动优化）。
    fn scroll_delta_for_dirty(&self) -> Option<(f32, f32)> {
        None
    }
    fn hit_test_frame(&self, actual_frame: Rect) -> Rect {
        actual_frame
    }
    fn hit_test_3d(&self, ray: &Ray3D, _spatial: &SpatialContext, frame: Rect) -> bool {
        if let Some(hit_point) = ray.intersect_z0() {
            hit_point.x >= frame.x
                && hit_point.x <= frame.x + frame.w
                && hit_point.y >= frame.y
                && hit_point.y <= frame.y + frame.h
        } else {
            false
        }
    }
}

/// 生命周期行为。
pub trait WidgetLifecycle: WidgetComponent {
    fn on_init(&mut self) {}
    fn on_mount(&mut self) {}
    fn on_unmount(&mut self) {}
    fn on_update(&mut self, _dt: f64) {}
}

/// 转换为 WidgetNode 的 trait。
pub trait IntoWidgetNode {
    fn into_node(self) -> WidgetNode;
}

impl<T: WidgetComponent + 'static> IntoWidgetNode for T {
    fn into_node(self) -> WidgetNode {
        WidgetNode::leaf(Box::new(self))
    }
}

impl IntoWidgetNode for WidgetNode {
    fn into_node(self) -> WidgetNode {
        self
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ════════════════════════════════════════════════════════════════════════════
// 渲染服务组件
// ════════════════════════════════════════════════════════════════════════════

use uix_graphics::font_service::FontService;
use uix_graphics::spatial::PhysicalUnit;
use uix_graphics::traits::Canvas2D;
use uix_graphics::FontHandle;

/// 文本渲染服务接口 — 字体管理、文本布局、glyph 光栅化。
pub trait TextRenderer {
    /// 更新字体句柄。
    fn set_font(&mut self, font: FontHandle);
    /// 设置文本绘制最大宽度。
    fn set_max_text_width(&mut self, width: f32);
    /// 获取当前字体句柄。
    fn font(&self) -> &FontHandle;
    /// 获取字体服务引用。
    fn font_service(&mut self) -> &FontService;

    // ── 2D 文本绘制 ──
    fn draw_text(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        pos: Point,
        color: Color,
        font_size: f32,
    );
    fn draw_text_baseline(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        x: f32,
        baseline_y: f32,
        color: Color,
        font_size: f32,
    );
    fn text_center(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    );
    fn draw_text_in_frame(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    );
    fn draw_text_wrapped(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        rect: Rect,
        color: Color,
        font_size: f32,
    );

    // ── 文本选中 ──
    fn draw_text_with_selection(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        pos: Point,
        color: Color,
        font_size: f32,
        selection: Option<(usize, usize)>,
        selection_bg: Color,
    );
    fn selection_rects(
        &mut self,
        canvas: &mut dyn Canvas2D,
        text: &str,
        font_size: f32,
        pos: Point,
        start: usize,
        end: usize,
    ) -> Vec<Rect>;

    // ── 文本测量 ──
    fn measure_text(&mut self, text: &str, font_size: f32) -> Size;
    fn measure_text_wrapped(&mut self, text: &str, font_size: f32, max_width: f32) -> Size;
    fn text_hit_test(&mut self, text: &str, font_size: f32, point: Point) -> Option<usize>;
    fn text_cursor_x(&mut self, text: &str, font_size: f32, char_index: usize) -> f32;

    // ── 辅助 ──
    fn visual_center_y(&mut self, rect: Rect, font_size: f32) -> f32;

    // ── 3D 空间文本 ──
    fn draw_text_spatial(
        &mut self,
        canvas: &mut dyn Canvas2D,
        spatial: &SpatialContext,
        text: &str,
        pos: uix_graphics::spatial::Vec3,
        color: Color,
        font_size: PhysicalUnit,
    );
    fn text_center_spatial(
        &mut self,
        canvas: &mut dyn Canvas2D,
        spatial: &SpatialContext,
        text: &str,
        box_3d: uix_graphics::spatial::AABB3D,
        color: Color,
        font_size: PhysicalUnit,
    );

    // ── 底层 glyph 绘制 ──
    fn blit_to(
        &mut self,
        canvas: &mut dyn Canvas2D,
        layout: &uix_graphics::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    );
}

/// 调试渲染服务接口。
pub trait DebugRenderer {
    fn set_debug_mode(&mut self, mode: bool);
    fn debug_mode(&self) -> bool;
    fn draw_debug_border(&self, canvas: &mut dyn Canvas2D, rect: Rect, depth: usize, hovered: bool);
}

// ════════════════════════════════════════════════════════════════════════════
// 布局引擎
// ════════════════════════════════════════════════════════════════════════════

/// 统一布局引擎 trait — Flex 和 Grid 的公共抽象。
pub trait LayoutEngine {
    /// 在内容区域内计算子节点位置。
    fn layout(
        &self,
        content_rect: Rect,
        children: &[crate::layout::engine::LayoutChild],
    ) -> crate::layout::engine::LayoutOutput;
}

// ════════════════════════════════════════════════════════════════════════════
// 动画
// ════════════════════════════════════════════════════════════════════════════

/// 可在线性空间中进行插值的类型。
pub trait Animatable: Clone + Copy + Send + 'static {
    fn lerp(from: Self, to: Self, t: f64) -> Self;
    fn delta(from: Self, to: Self) -> f64;
}

// ════════════════════════════════════════════════════════════════════════════
// 设计令牌（主题）— 聚合 trait 定义在 graphics/painting，UI 扩展 style_* 助手
// ════════════════════════════════════════════════════════════════════════════

pub use uix_graphics::painting::{
    IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, ShadowToken, ThemeTokens,
};

/// 抽象设计令牌提供者 — 聚合 ThemeTokens 与 UI 域 style 助手。
pub trait TokenProvider: ThemeTokens + Send + Sync {
    fn style_container(&self) -> Style {
        Style {
            background: None,
            border_color: None,
            border_width: 0.0,
            border_radius: self.border_radius(),
            color: self.color_text(),
            font_size: self.font_size(),
            ..Style::default()
        }
    }

    fn style_label(&self) -> Style {
        Style {
            color: self.color_text(),
            font_size: self.font_size(),
            padding: EdgeInsets::new(2.0, 0.0, 0.0, 0.0),
            ..Style::default()
        }
    }

    fn style_button_default(&self) -> Style {
        Style {
            background: None,
            border_color: Some(self.color_border()),
            border_width: 1.0,
            border_radius: self.border_radius(),
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: self.color_text(),
            font_size: self.font_size(),
            ..Style::default()
        }
    }

    fn style_button_primary(&self) -> Style {
        Style {
            background: Some(self.color_primary()),
            border_color: Some(self.color_primary()),
            border_width: 1.0,
            border_radius: self.border_radius(),
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: self.color_white(),
            font_size: self.font_size(),
            ..Style::default()
        }
    }
}
