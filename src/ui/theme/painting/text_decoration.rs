// 引入文本布局的中性字形与行信息。
use crate::draw::resources::font::text_backend::TextLayout;
// 引入线段坐标、颜色与 UI 绘制上下文。
use crate::{core::Point, draw::Color, ui::widget_runtime::paint_context::PaintContext};
// 引入 UI System 自有的文本装饰契约。
use crate::ui::theme::style::TextDecoration;

// 保存移动拥有型文本布局前计算出的装饰线段。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecorationSegment {
    // 线段的绝对水平起点。
    start_x: f32,
    // 线段的绝对水平终点。
    end_x: f32,
    // 线段的绝对垂直位置。
    y: f32,
}

// 按最终视觉布局生成每行文本装饰线段。
pub fn segments(
    // 借用与文字绘制相同的排版结果。
    layout: &TextLayout,
    // 接收文字绘制的绝对原点。
    origin: Point,
    // 接收用于稳定后备行框的最终字号。
    font_size: f32,
    // 接收已经解析的装饰枚举。
    decoration: TextDecoration,
) -> Vec<DecorationSegment> {
    // 显式 none 不产生任何绘制命令。
    if decoration == TextDecoration::None {
        // 返回空集合供调用方直接绘制。
        return Vec::new();
    }
    // 为每个非空视觉行生成一条装饰线。
    layout
        // 遍历排版产生的视觉行。
        .lines
        // 借用行信息避免复制布局。
        .iter()
        // 空行没有可见文字宽度，因此不绘制装饰。
        .filter_map(|line| {
            // 计算当前行字形范围的安全终点。
            let end = (line.glyph_start + line.glyph_count).min(layout.glyphs.len());
            // 取得当前行连续视觉字形。
            let glyphs = layout.glyphs.get(line.glyph_start..end)?;
            // 至少需要一个字形才能形成线段。
            let first = glyphs.first()?;
            // 从首字形初始化左右边界。
            let (mut left, mut right) = (first.x, first.x + first.width.max(0.0));
            // 聚合全部视觉字形，兼容 RTL 与混排顺序。
            for glyph in glyphs.iter().skip(1) {
                // 扩展当前行的最左边界。
                left = left.min(glyph.x);
                // 扩展当前行的最右边界。
                right = right.max(glyph.x + glyph.width.max(0.0));
            }
            // 使用布局行高并提供稳定正字号后备。
            let line_height = line.height.max(font_size.max(1.0) * 1.2);
            // 按闭合枚举选择装饰线的垂直位置。
            let y = match decoration {
                // none 已在函数入口返回。
                TextDecoration::None => unreachable!("none 不生成文本装饰线段"),
                // 下划线贴近视觉行下缘。
                TextDecoration::Underline => origin.y + line.y + line_height - 1.0,
                // 上划线贴近视觉行上缘。
                TextDecoration::Overline => origin.y + line.y + 1.0,
                // 删除线穿过视觉行中部。
                TextDecoration::LineThrough => origin.y + line.y + line_height * 0.52,
            };
            // 返回绝对坐标线段。
            Some(DecorationSegment {
                // 平移当前行左缘。
                start_x: origin.x + left,
                // 平移当前行右缘。
                end_x: origin.x + right,
                // 保存已解析垂直位置。
                y,
            })
        })
        // 收集为可跨拥有权移动保存的紧凑列表。
        .collect()
}

// 使用 draw System 的中性直线 API 绘制预计算文本装饰。
pub fn paint(
    // 借用当前 UI 绘制上下文。
    ctx: &mut PaintContext<'_, '_>,
    // 借用预计算的绝对坐标线段。
    segments: &[DecorationSegment],
    // 使用与文字相同的最终颜色。
    color: Color,
) {
    // 依次提交每行独立的直线命令。
    for segment in segments {
        // 底层 draw 仅接收几何、颜色与线宽，不依赖 UI 样式类型。
        ctx.draw_line(
            // 传入水平起点。
            segment.start_x,
            // 传入垂直起点。
            segment.y,
            // 传入水平终点。
            segment.end_x,
            // 水平线终点沿用相同垂直坐标。
            segment.y,
            // 传入解析后的文字颜色。
            color,
            // 使用稳定的一逻辑像素线宽。
            1.0,
        );
    }
}
