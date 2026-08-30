// 回归覆盖：聚焦边框换宽后的局部重绘必须覆盖边框外溢带。
// 症状（真窗截图逐像素取证）：聚焦首帧失效矩形只含 frame，2px 聚焦描边
// 以 frame 边界为中心线向外溢出 1px，外溢带既不清除也不重绘，残留上一帧
// 1px 旧边框——呈现为上边 2px、其余三边 1px 的宽度不一致。修复为 Input
// 声明 draw_margin，使 dirty_rect 覆盖边框外溢与 AA 过渡。
#![cfg(feature = "test-harness")]

use uix::prelude::*;
use uix::ui::WidgetRender;

// 聚焦边框 2px 时外溢至少一个像素，再叠加一个像素的分析 AA 过渡。
const MIN_OUTSET: f32 = 1.5;

#[test]
fn textarea_dirty_rect_covers_focus_border_outset() {
    let frame = Rect::new(100.0, 100.0, 200.0, 80.0);
    let dirty = Input::textarea().dirty_rect(frame);
    assert!(
        dirty.x <= frame.x - MIN_OUTSET
            && dirty.y <= frame.y - MIN_OUTSET
            && dirty.x + dirty.w >= frame.x + frame.w + MIN_OUTSET
            && dirty.y + dirty.h >= frame.y + frame.h + MIN_OUTSET,
        "textarea 脏矩形 {dirty:?} 必须覆盖聚焦边框的外溢带（每边 ≥{MIN_OUTSET}px）"
    );
}

#[test]
fn singleline_input_dirty_rect_covers_focus_border_outset() {
    let frame = Rect::new(0.0, 0.0, 160.0, 32.0);
    let dirty = Input::new("").dirty_rect(frame);
    assert!(
        dirty.x <= frame.x - MIN_OUTSET
            && dirty.y <= frame.y - MIN_OUTSET
            && dirty.x + dirty.w >= frame.x + frame.w + MIN_OUTSET
            && dirty.y + dirty.h >= frame.y + frame.h + MIN_OUTSET,
        "单行输入脏矩形 {dirty:?} 必须覆盖聚焦边框的外溢带（每边 ≥{MIN_OUTSET}px）"
    );
}

#[test]
fn focus_transition_layout_rect_stays_inside_dirty_rect() {
    // 走真实焦点切换路径：聚焦前后布局 frame 都必须落在脏矩形内，
    // 保证 clear/clip 不会裁掉新边框或残留旧边框。
    use uix::ui::test_harness::TestApp;

    let mut app = TestApp::new((320.0, 120.0), || {
        column((input().placeholder("焦点边框").automation_id("input.focus"),))
    });
    let frame_before = app.snapshot().find("input.focus").expect("输入框应存在").frame;
    app.focus("input.focus").expect("聚焦应成功");
    app.settle().expect("聚焦重绘应收敛");
    let frame_after = app.snapshot().find("input.focus").expect("输入框应存在").frame;

    let widget = Input::new("");
    let dirty = widget.dirty_rect(frame_after);
    for (label, value) in [
        ("frame_before.x", frame_before.x),
        ("frame_after.x", frame_after.x),
    ] {
        assert!(
            dirty.x <= value + f32::EPSILON,
            "脏矩形左界 {0} 应覆盖 {label}={value}",
            dirty.x
        );
    }
}
