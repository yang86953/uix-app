// 各项自带原 cfg 门控（test/feature/平台组合），在源文件模块作用域内 include! 展开。

// 图片编解码能力开启时绘制布局中的全部图片原子。
#[cfg(all(test, feature = "image-codecs"))]
pub(crate) fn draw(
    ctx: &mut crate::ui::widget_runtime::paint_context::PaintContext,
    segments: &[super::RichTextSegment],
    states: &InlineImageStates,
    lines: &[super::LayoutLine],
    frame: crate::core::Rect,
) {
    let visual = super::presentation::RICH_TEXT_VISUAL;
    let resolved = visual.resolve(crate::draw::Color::default(), true, ctx.tokens());
    draw_visual(ctx, segments, states, lines, frame, visual.image, resolved);
}
