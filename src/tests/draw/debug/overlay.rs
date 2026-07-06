use super::*;
use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;

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
fn draw_debug_border_runs_when_on() {
    let d = DebugRenderService::new(true);
    let mut canvas = NoopCanvas2D;
    d.draw_debug_border(&mut canvas, Rect::new(0.0, 0.0, 100.0, 50.0), 0, false);
    d.draw_debug_border(&mut canvas, Rect::new(0.0, 0.0, 100.0, 50.0), 0, true);
}
