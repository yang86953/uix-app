//! # uix-ui 行为接口（契约定义）
//!
//! 本模块定义 ui 层的所有公开 trait 接口。
//! 实现保留在各领域内部模块中，通过本模块统一暴露契约。

use std::any::Any;
use uix_platform::{Point, Rect, Size, EdgeInsets};
use uix_graphics::{Color, GraphicsEngine};
use uix_graphics::spatial::{Ray3D, SpatialContext};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetId, WidgetTree, WidgetNode};
use crate::theme::ShadowToken;
use crate::style::Style;

// ════════════════════════════════════════════════════════════════════════════
// Widget 组件体系（无上帝接口，每个能力是独立 trait）
// ════════════════════════════════════════════════════════════════════════════

/// 能力位标记。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetCapabilities(u8);

impl WidgetCapabilities {
    pub const LAYOUT: u8   = 0b0001;
    pub const RENDER: u8   = 0b0010;
    pub const EVENT: u8    = 0b0100;
    pub const LIFECYCLE: u8 = 0b1000;

    pub const fn new() -> Self { Self(0) }
    pub const fn from_bits(bits: u8) -> Self { Self(bits) }
    pub fn insert(&mut self, cap: u8) { self.0 |= cap; }
    pub fn contains(&self, cap: u8) -> bool { self.0 & cap != 0 }
    pub fn bits(&self) -> u8 { self.0 }
}

/// 组件核心标识 — 所有 widget 必须实现。
pub trait WidgetComponent: 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// 返回此 widget 实现了哪些能力。
    fn capabilities(&self) -> WidgetCapabilities;
    /// 返回当前 widget 的内部可见性。
    fn visible(&self) -> bool { true }
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> { vec![] }
    /// 默认 Tab 键导航索引（> 0 表示组件默认可通过 Tab 聚焦）。
    /// 应用层可通过 `WidgetNode::tab_index()` 覆盖。
    fn tab_index(&self) -> i32 { 0 }

    // ── 可选能力上转型（宏自动生成） ──
    fn as_layout(&self) -> Option<&dyn WidgetLayout> { None }
    fn as_render(&self) -> Option<&dyn WidgetRender> { None }
    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> { None }
    fn as_event(&self) -> Option<&dyn WidgetEventHandler> { None }
    fn as_event_mut(&mut self) -> Option<&mut dyn WidgetEventHandler> { None }
    fn as_lifecycle(&self) -> Option<&dyn WidgetLifecycle> { None }
    fn as_lifecycle_mut(&mut self) -> Option<&mut dyn WidgetLifecycle> { None }
}

/// 布局行为：尺寸、弹性、子节点排列。
pub trait WidgetLayout: WidgetComponent {
    fn preferred_size(&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::zero()
    }
    fn flex_grow(&self) -> f32 { 0.0 }
    fn flex_shrink(&self) -> f32 { 0.0 }
    fn layout_children(
        &self, frame: Rect, children: &[WidgetId], tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        let _ = (frame, children, tree);
        Vec::new()
    }
}

/// 渲染行为：绘制、覆盖层、脏区域。
pub trait WidgetRender: WidgetComponent {
    fn render(&self, frame: Rect, ctx: &mut RenderContext, tree: &WidgetTree);
    fn post_render(&self, _frame: Rect, _ctx: &mut RenderContext, _tree: &WidgetTree) {}
    fn draw_margin(&self) -> f32 { 0.0 }
    fn dirty_rect(&self, frame: Rect) -> Rect {
        let m = self.draw_margin();
        if m > 0.0 {
            Rect::new(frame.x - m, frame.y - m, frame.w + m * 2.0, frame.h + m * 2.0)
        } else { frame }
    }
    fn is_repaint_boundary(&self) -> bool { false }
    fn children_clip(&self, _frame: Rect) -> Option<Rect> { None }
}

/// 事件行为：输入事件处理、持续更新、滚动偏移、命中测试。
pub trait WidgetEventHandler: WidgetComponent {
    fn on_event(&mut self, _event: &WidgetEvent) -> EventResult { EventResult::NotHandled }
    fn needs_continuous_update(&self) -> bool { false }
    fn scroll_delta(&self, _frame: Rect) -> Option<(f32, f32)> { None }
    /// 帧间滚动偏移（用于 scroll_region 像素移动优化）。
    fn scroll_delta_for_dirty(&self) -> Option<(f32, f32)> { None }
    fn hit_test_frame(&self, actual_frame: Rect) -> Rect { actual_frame }
    fn hit_test_3d(&self, ray: &Ray3D, _spatial: &SpatialContext, frame: Rect) -> bool {
        if let Some(hit_point) = ray.intersect_z0() {
            hit_point.x >= frame.x && hit_point.x <= frame.x + frame.w
                && hit_point.y >= frame.y && hit_point.y <= frame.y + frame.h
        } else { false }
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

use uix_graphics::traits::Canvas2D;
use uix_graphics::font_service::FontService;
use uix_graphics::spatial::PhysicalUnit;
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
    fn draw_text(&mut self, canvas: &mut dyn Canvas2D, text: &str, pos: Point, color: Color, font_size: f32);
    fn draw_text_baseline(&mut self, canvas: &mut dyn Canvas2D, text: &str, x: f32, baseline_y: f32, color: Color, font_size: f32);
    fn text_center(&mut self, canvas: &mut dyn Canvas2D, text: &str, rect: Rect, color: Color, font_size: f32);
    fn draw_text_in_frame(&mut self, canvas: &mut dyn Canvas2D, text: &str, rect: Rect, color: Color, font_size: f32);
    fn draw_text_wrapped(&mut self, canvas: &mut dyn Canvas2D, text: &str, rect: Rect, color: Color, font_size: f32);

    // ── 文本选中 ──
    fn draw_text_with_selection(
        &mut self, canvas: &mut dyn Canvas2D, text: &str, pos: Point, color: Color,
        font_size: f32, selection: Option<(usize, usize)>, selection_bg: Color,
    );
    fn selection_rects(
        &mut self, canvas: &mut dyn Canvas2D, text: &str, font_size: f32,
        pos: Point, start: usize, end: usize,
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
        &mut self, canvas: &mut dyn Canvas2D, spatial: &SpatialContext,
        text: &str, pos: uix_graphics::spatial::Vec3, color: Color, font_size: PhysicalUnit,
    );
    fn text_center_spatial(
        &mut self, canvas: &mut dyn Canvas2D, spatial: &SpatialContext,
        text: &str, box_3d: uix_graphics::spatial::AABB3D, color: Color, font_size: PhysicalUnit,
    );

    // ── 底层 glyph 绘制 ──
    fn blit_to(
        &mut self, canvas: &mut dyn Canvas2D,
        layout: &uix_graphics::text_backend::TextLayout,
        pos: Point, color: Color, font_size: f32,
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
    fn layout(&self, content_rect: Rect, children: &[crate::layout::engine::LayoutChild])
        -> crate::layout::engine::LayoutOutput;
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
// 设计令牌（主题）
// ════════════════════════════════════════════════════════════════════════════

/// 颜色设计令牌 — brand, background, border, fill, text, semantic, shadow, link。
pub trait IColorTokens: Send + Sync {
    fn color_primary(&self) -> Color;
    fn color_primary_hover(&self) -> Color;
    fn color_primary_active(&self) -> Color;
    fn color_primary_bg(&self) -> Color;
    fn color_primary_border(&self) -> Color;
    fn color_bg_container(&self) -> Color;
    fn color_bg_elevated(&self) -> Color;
    fn color_bg_raised(&self) -> Color;
    fn color_bg_overlay(&self) -> Color;
    fn color_bg_layout(&self) -> Color;
    fn color_bg_spotlight(&self) -> Color;
    fn color_bg_mask(&self) -> Color;
    fn color_border(&self) -> Color;
    fn color_border_secondary(&self) -> Color;
    fn color_fill(&self) -> Color;
    fn color_fill_secondary(&self) -> Color;
    fn color_fill_tertiary(&self) -> Color;
    fn color_fill_quaternary(&self) -> Color;
    fn color_text(&self) -> Color;
    fn color_text_secondary(&self) -> Color;
    fn color_text_tertiary(&self) -> Color;
    fn color_text_quaternary(&self) -> Color;
    fn color_white(&self) -> Color;
    fn color_black(&self) -> Color;
    fn color_shadow(&self) -> Color;
    fn color_shadow_secondary(&self) -> Color;
    fn color_success(&self) -> Color;
    fn color_success_bg(&self) -> Color;
    fn color_success_border(&self) -> Color;
    fn color_warning(&self) -> Color;
    fn color_warning_bg(&self) -> Color;
    fn color_warning_border(&self) -> Color;
    fn color_error(&self) -> Color;
    fn color_error_bg(&self) -> Color;
    fn color_error_border(&self) -> Color;
    fn color_info(&self) -> Color;
    fn color_info_bg(&self) -> Color;
    fn color_info_border(&self) -> Color;
    fn color_link(&self) -> Color;
    fn color_link_hover(&self) -> Color;
    fn color_link_active(&self) -> Color;
}

/// 排版设计令牌。
pub trait ITypographyTokens: Send + Sync {
    fn font_family(&self) -> &str;
    fn font_size_sm(&self) -> f32 { 12.0 }
    fn font_size(&self) -> f32 { 14.0 }
    fn font_size_lg(&self) -> f32 { 16.0 }
    fn font_size_xl(&self) -> f32 { 20.0 }
    fn font_size_heading_1(&self) -> f32 { 38.0 }
    fn font_size_heading_2(&self) -> f32 { 30.0 }
    fn font_size_heading_3(&self) -> f32 { 24.0 }
    fn font_size_heading_4(&self) -> f32 { 20.0 }
    fn font_size_heading_5(&self) -> f32 { 16.0 }
    fn font_weight_regular(&self) -> f32 { 400.0 }
    fn font_weight_medium(&self) -> f32 { 500.0 }
    fn font_weight_semibold(&self) -> f32 { 600.0 }
    fn font_weight_bold(&self) -> f32 { 700.0 }
    fn line_height(&self) -> f32 { 1.5715 }
}

/// 间距与尺寸设计令牌。
pub trait ISpacingTokens: Send + Sync {
    fn padding_xss(&self) -> f32 { 4.0 }
    fn padding_xs(&self) -> f32 { 8.0 }
    fn padding_sm(&self) -> f32 { 12.0 }
    fn padding(&self) -> f32 { 16.0 }
    fn padding_md(&self) -> f32 { 20.0 }
    fn padding_lg(&self) -> f32 { 24.0 }
    fn padding_xl(&self) -> f32 { 32.0 }
    fn border_radius(&self) -> f32 { 6.0 }
    fn border_radius_sm(&self) -> f32 { 4.0 }
    fn border_radius_lg(&self) -> f32 { 8.0 }
    fn border_radius_xl(&self) -> f32 { 12.0 }
    fn border_radius_round(&self) -> f32 { 999.0 }
    fn control_height_sm(&self) -> f32 { 24.0 }
    fn control_height(&self) -> f32 { 32.0 }
    fn control_height_lg(&self) -> f32 { 40.0 }
    fn motion_duration_fast(&self) -> f32 { 0.1 }
    fn motion_duration_mid(&self) -> f32 { 0.2 }
    fn motion_duration_slow(&self) -> f32 { 0.3 }
    fn motion_easing_default(&self) -> &str { "cubic-bezier(0.25, 0.1, 0.25, 1)" }
    fn motion_easing_in(&self) -> &str { "cubic-bezier(0.42, 0, 1, 1)" }
    fn motion_easing_out(&self) -> &str { "cubic-bezier(0, 0, 0.58, 1)" }
    fn motion_easing_in_out(&self) -> &str { "cubic-bezier(0.42, 0, 0.58, 1)" }
    fn screen_xs(&self) -> f32 { 480.0 }
    fn screen_sm(&self) -> f32 { 576.0 }
    fn screen_md(&self) -> f32 { 768.0 }
    fn screen_lg(&self) -> f32 { 992.0 }
    fn screen_xl(&self) -> f32 { 1200.0 }
    fn screen_xxl(&self) -> f32 { 1600.0 }
}

/// 结构化多层阴影令牌。
pub trait IBoxShadowTokens: Send + Sync {
    fn box_shadow(&self) -> ShadowToken;
    fn box_shadow_secondary(&self) -> ShadowToken;
}

/// 抽象设计令牌提供者 — 聚合全部子 trait。
pub trait TokenProvider:
    IColorTokens + ITypographyTokens + ISpacingTokens + IBoxShadowTokens + Send + Sync
{
    fn is_dark(&self) -> bool { false }

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
