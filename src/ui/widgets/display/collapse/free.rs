//! 折叠面板文本辅助。

use super::*;

pub(super) fn single_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

pub(super) fn conservative_text_width(ctx: &mut PaintContext, text: &str, font_size: f32) -> f32 {
    ctx.measure_text(text, font_size).w.max(
        crate::draw::resources::font::text_backend::estimate_text_metrics(
            text,
            f32::INFINITY,
            font_size,
        )
        .max_line_width,
    )
}

pub(super) fn elide_single_line(
    ctx: &mut PaintContext,
    text: &str,
    font_size: f32,
    max_width: f32,
) -> Option<String> {
    if !max_width.is_finite() || max_width <= 0.0 {
        return None;
    }
    let text = single_line(text);
    if conservative_text_width(ctx, &text, font_size) <= max_width {
        return Some(text);
    }
    const ELLIPSIS: &str = "…";
    if conservative_text_width(ctx, ELLIPSIS, font_size) > max_width {
        return None;
    }
    let mut visible = String::new();
    for ch in text.chars() {
        visible.push(ch);
        visible.push_str(ELLIPSIS);
        let fits = conservative_text_width(ctx, &visible, font_size) <= max_width;
        visible.pop();
        if !fits {
            visible.pop();
            break;
        }
    }
    visible.push_str(ELLIPSIS);
    Some(visible)
}
