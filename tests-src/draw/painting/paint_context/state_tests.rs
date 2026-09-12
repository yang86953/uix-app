//! `draw/painting/paint_context/state.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use crate::draw::FontHandle;
use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::draw::geometry::spatial::Orientation;
use crate::draw::painting::{DisplayList, PaintSurfaceConfig};
use crate::draw::resources::{FontService, ImageService};

fn record_text_variants(
    ctx: &mut PaintContext<'_>,
    list: &mut DisplayList,
    text: &str,
    x: f32,
) {
    list.begin_rewrite();
    ctx.with_recorder(list, |ctx| {
        ctx.draw_text(text, Point::new(x, 1.0), Color::black(), 12.0);
        ctx.draw_text_baseline(text, x, 14.0, Color::red(), 13.0);
        ctx.text_center(text, Rect::new(x, 2.0, 30.0, 12.0), Color::green(), 14.0);
        ctx.draw_text_in_frame(text, Rect::new(x, 3.0, 30.0, 12.0), Color::blue(), 15.0);
        ctx.draw_text_wrapped(text, Rect::new(x, 4.0, 30.0, 24.0), Color::white(), 16.0);
        ctx.fill_text_selection(text, 16.5, Point::new(x, 4.5), 1, 4, Color::green());
        ctx.draw_text_with_selection(
            text,
            Point::new(x, 5.0),
            Color::black(),
            17.0,
            None,
            Color::blue(),
        );
    });
    list.finish_rewrite();
}

fn recorded_texts(list: &DisplayList) -> Vec<Arc<str>> {
    list.ops()
        .iter()
        .map(|op| match op {
            PaintOp::DrawText { text, .. }
            | PaintOp::DrawTextBaseline { text, .. }
            | PaintOp::TextCenter { text, .. }
            | PaintOp::DrawTextInFrame { text, .. }
            | PaintOp::DrawTextWrapped { text, .. }
            | PaintOp::FillTextSelection { text, .. }
            | PaintOp::DrawTextWithSelection { text, .. } => Arc::clone(text),
            other => panic!("只应录制文字操作，实际为 {other:?}"),
        })
        .collect()
}

// 七类文字操作都应在稳定内容下复用 Arc，并在内容变化时正确替换。
#[test]
fn text_variants_reuse_stable_content_and_replace_changed_content() {
    let mut canvas = NoopCanvas2D;
    let font_service = FontService::new();
    let image_service = ImageService::new();
    let mut ctx = PaintContext::new(
        &mut canvas,
        FontHandle::new(0),
        &font_service,
        &image_service,
        PaintSurfaceConfig {
            dpi: 96.0,
            device_pixel_ratio: 1.0,
            orientation: Orientation::YDown,
            surface_w: 64,
            surface_h: 64,
        },
    );
    let mut list = DisplayList::new();

    record_text_variants(&mut ctx, &mut list, "steady", 1.0);
    let first_texts = recorded_texts(&list);
    record_text_variants(&mut ctx, &mut list, "steady", 2.0);
    let second_texts = recorded_texts(&list);
    assert_eq!(first_texts.len(), 7);
    assert!(
        first_texts
            .iter()
            .zip(&second_texts)
            .all(|(first, second)| Arc::ptr_eq(first, second))
    );
    let PaintOp::DrawText { pos, .. } = &list.ops()[0] else {
        panic!("首条操作应为普通文字");
    };
    assert_eq!(*pos, Point::new(2.0, 1.0));

    record_text_variants(&mut ctx, &mut list, "changed", 3.0);
    let changed_texts = recorded_texts(&list);
    assert!(changed_texts.iter().all(|text| text.as_ref() == "changed"));
    assert!(
        second_texts
            .iter()
            .zip(&changed_texts)
            .all(|(steady, changed)| !Arc::ptr_eq(steady, changed))
    );
}
