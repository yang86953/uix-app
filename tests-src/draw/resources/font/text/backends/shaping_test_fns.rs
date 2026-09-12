// 各项自带 cfg(test)，在源模块作用域 include! 展开。

pub(super) fn layout_text(
    // 完整字体文件字节，支持 TTF 以及集合字体。
    data: &[u8],
    // 集合字体中的字体面编号。
    face_index: u32,
    // 字形光栅化使用的稳定字体句柄。
    font: FontHandle,
    // 当前单字体段的原始文本。
    text: &str,
    // 像素字号、宽高与换行约束。
    opts: &TextLayoutOptions,
    // 与 ab_glyph 光栅化一致的字体设计单位到像素缩放。
    glyph_scale: f32,
    // ab_glyph 计算的像素 ascent。
    ascent: f32,
    // ab_glyph 计算的实际字体高度。
    font_height: f32,
    // 调用方解析后的行高。
    line_height: f32,
    // 可选的 UAX #9 已解析方向；空值保留独立后端自动推断。
    direction: Option<TextDirection>,
    // 解析或索引异常时返回空值以启用旧后端回退。
) -> Option<TextLayout> {
    // 直接测试入口仍验证原始字体字节，并复用生产路径的已解析字体面实现。
    let face = rustybuzz::Face::from_slice(data, face_index)?;
    layout_text_with_face(
        &face,
        font,
        text,
        opts,
        glyph_scale,
        ascent,
        font_height,
        line_height,
        direction,
    )
}
