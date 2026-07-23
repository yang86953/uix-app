use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::draw::debug::overlay::*;
use crate::draw::renderer::RenderMetrics;
use crate::tests::common::*;

#[test]
fn set_debug_mode_toggle() {
    let mut d = DebugRenderService::new(false);
    assert!(!d.debug_mode);
    d.set_debug_mode(true);
    assert!(d.debug_mode);
    d.set_debug_mode(false);
    assert!(!d.debug_mode);
}

#[test]
fn draw_debug_border_skip_when_off() {
    let d = DebugRenderService::new(false);
    let mut canvas = NoopCanvas2D;
    d.draw_debug_border(&mut canvas, Rect::new(0.0, 0.0, 100.0, 50.0), 0, false);
}

#[test]
fn draw_debug_border_skips_non_hovered() {
    // 实用组合：非 hover 不画淡彩框，避免满屏干扰。
    let d = DebugRenderService::new(true);
    let mut canvas = NoopCanvas2D;
    d.draw_debug_border(&mut canvas, Rect::new(0.0, 0.0, 100.0, 50.0), 0, false);
}

#[test]
fn draw_debug_border_runs_when_hovered() {
    let d = DebugRenderService::new(true);
    let mut canvas = NoopCanvas2D;
    d.draw_debug_border(&mut canvas, Rect::new(0.0, 0.0, 100.0, 50.0), 0, true);
}

#[test]
fn telemetry_hud_lines_include_toggle_hint() {
    let m = RenderMetrics::default();
    let lines = DebugRenderService::telemetry_hud_lines(&m);
    assert_eq!(lines.len(), 6);
    assert!(lines[0].starts_with("inv:"));
    assert!(lines[5].contains("Ctrl+Shift+D"));
}
