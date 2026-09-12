// 各项自带 cfg(test) 门控，在源文件模块作用域内 include! 展开。

#[cfg(test)]
pub(crate) fn layout_rich_text_real_with_images(
    segments: &[RichTextSegment],
    max_width: f32,
    default_font_size: f32,
    palette: RichTextPalette,
    font_service: &FontService,
    font: &FontHandle,
    image_states: &InlineImageStates,
) -> (Vec<LayoutLine>, f32, f32) {
    layout_rich_text_real_with_images_visual(
        segments,
        max_width,
        default_font_size,
        palette,
        font_service,
        font,
        image_states,
        RICH_TEXT_VISUAL.metrics,
    )
}
