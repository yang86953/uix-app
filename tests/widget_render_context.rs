//! uix-ui RenderContext 集成测试。
//!
//! 使用 `NullEngine`（NoopCanvas2D）测试 RenderContext 的创建和基本操作。
//! 这些测试不依赖平台层测试框架，可在无 `test-harness` feature 时运行。

use uix::render::font_service::FontService;
use uix::render::null_engine::NullEngine;
use uix::render::{Color, FontHandle, GradientDirection, traits::GraphicsEngine, Orientation, Radius};
use uix::platform::api::geometry::Rect;
use uix::api::widget::DesignTokens;
use uix::api::widget::RenderContext;

// ════════════════════════════════════════════════════════════════════════════
// RenderContext 创建测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn render_context_creation_succeeds() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_light();
    let canvas = null_engine.canvas_2d();

    // 基本创建不应 panic
    let _ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        800,
        600,
    );
}

#[test]
fn render_context_with_gpu_dpr() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_light();
    let canvas = null_engine.canvas_2d();

    // 高 DPI 设置
    let _ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        192.0, // dpi = 2x
        2.0,   // device_pixel_ratio
        Orientation::YDown,
        1600,
        1200,
    );
}

// ════════════════════════════════════════════════════════════════════════════
// 2D 绘制操作测试（验证委托 NoopCanvas2D 不 panic）
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn fill_rect_does_not_panic() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_light();
    let canvas = null_engine.canvas_2d();

    let mut ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        800,
        600,
    );

    ctx.fill_rect(Rect::new(10.0, 10.0, 100.0, 50.0), Color::red(), None);
    ctx.fill_rect(
        Rect::new(50.0, 50.0, 200.0, 100.0),
        Color::blue(),
        Some(Radius::uniform(8.0)),
    );
}

#[test]
fn fill_circle_does_not_panic() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_light();
    let canvas = null_engine.canvas_2d();

    let mut ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        800,
        600,
    );

    ctx.fill_circle(100.0, 100.0, 50.0, Color::green());
    ctx.fill_circle(300.0, 200.0, 25.0, Color::from_rgba(255, 0, 0, 128));
}

#[test]
fn stroke_rect_does_not_panic() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_light();
    let canvas = null_engine.canvas_2d();

    let mut ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        800,
        600,
    );

    ctx.stroke_rect(Rect::new(0.0, 0.0, 50.0, 50.0), Color::white(), 2.0, None);
    ctx.stroke_rect(
        Rect::new(10.0, 10.0, 80.0, 80.0),
        Color::black(),
        1.0,
        Some(Radius::uniform(4.0)),
    );
}

#[test]
fn draw_line_does_not_panic() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_light();
    let canvas = null_engine.canvas_2d();

    let mut ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        800,
        600,
    );

    ctx.draw_line(0.0, 0.0, 100.0, 100.0, Color::red(), 1.0);
    ctx.draw_line(100.0, 0.0, 0.0, 100.0, Color::blue(), 2.5);
}

// ════════════════════════════════════════════════════════════════════════════
// 渐变 & 阴影操作测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn fill_linear_gradient_does_not_panic() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_light();
    let canvas = null_engine.canvas_2d();

    let mut ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        800,
        600,
    );

    ctx.fill_linear_gradient(
        Rect::new(0.0, 0.0, 200.0, 200.0),
        Color::red(),
        Color::blue(),
        GradientDirection::Horizontal,
    );
}

#[test]
fn draw_box_shadow_does_not_panic() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_light();
    let canvas = null_engine.canvas_2d();

    let mut ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        800,
        600,
    );

    ctx.draw_box_shadow(
        Rect::new(10.0, 10.0, 100.0, 100.0),
        8.0,
        0.0,
        2.0,
        Color::from_rgba(0, 0, 0, 128),
        Some(Radius::uniform(6.0)),
    );
}

// ════════════════════════════════════════════════════════════════════════════
// Token 访问测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn tokens_access_returns_design_tokens() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_light();
    let canvas = null_engine.canvas_2d();

    let ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        800,
        600,
    );

    let t = ctx.tokens();
    assert_eq!(t.color_primary(), Color::from_rgba(22, 119, 255, 255));
    assert_eq!(t.font_size(), 14.0);
    assert!(!t.is_dark());
}

#[test]
fn tokens_dark_mode() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_dark();
    let canvas = null_engine.canvas_2d();

    let ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        800,
        600,
    );

    let t = ctx.tokens();
    assert!(t.is_dark());
    assert_eq!(t.color_text(), Color::from_rgba(255, 255, 255, 224));
}

// ════════════════════════════════════════════════════════════════════════════
// 空间上下文访问测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn spatial_context_accessible() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_light();
    let canvas = null_engine.canvas_2d();

    let mut ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        800,
        600,
    );

    let spatial = ctx.spatial();
    // SpatialContext 相关操作不 panic
    spatial.fill_rect(Rect::new(0.0, 0.0, 50.0, 50.0), Color::red(), None);
}

// ════════════════════════════════════════════════════════════════════════════
// 调试渲染测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn debug_render_mode_toggle() {
    let mut null_engine = NullEngine::new();
    let font_service = FontService::new();
    let tokens = DesignTokens::antd_light();
    let canvas = null_engine.canvas_2d();

    let mut ctx = RenderContext::new(
        canvas,
        FontHandle::new(0),
        &font_service,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        800,
        600,
    );

    // 调试渲染默认关闭
    assert!(!ctx.debug_mode());

    // 启用调试渲染
    ctx.set_debug_mode(true);
    assert!(ctx.debug_mode());

    // 再次关闭
    ctx.set_debug_mode(false);
    assert!(!ctx.debug_mode());
}
