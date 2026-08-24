//! RichText 主题分隔线的解析、布局与绘制组件。

// 引入主题分隔线布局行类型。
use super::presentation::{RichTextBreakVisual, RichTextMetricsVisual};
use super::{LayoutGlyph, LayoutLine, LayoutLineKind};
// 引入绘制矩形。
use crate::core::Rect;
use crate::draw::Color;
// 引入富文本绘制上下文。
use crate::ui::widget_runtime::paint_context::PaintContext;

// 判断整行是否满足主题分隔线标记契约。
pub(super) fn is_thematic_break_line(text: &str) -> bool {
    // 统计去除 ASCII 空白后的合法分隔符数量。
    let mut marker_count = 0usize;
    // 逐字符验证整行不含其他内容。
    for character in text.chars() {
        // 按权威契约允许三种可混合标记和 ASCII 空白。
        match character {
            // 每个可见标记都计入最小数量。
            '-' | '*' | '_' => marker_count += 1,
            // ASCII 空格和制表符只作为间隔，不计数。
            ' ' | '\t' => {}
            // 任何其他字符都使整行保持字面文本。
            _ => return false,
        }
    }
    // 至少三个可见标记才形成主题分隔线。
    marker_count >= 3
}

// 把主题分隔线登记为独立布局行。
pub(super) fn push_layout_line(
    // 接收已完成的视觉行。
    lines: &mut Vec<LayoutLine>,
    // 接收可能残留的当前文本行字形。
    glyphs: &mut Vec<LayoutGlyph>,
    // 接收默认行高。
    line_height: f32,
    // 接收 UIX 声明的统一行高比例。
    metrics: RichTextMetricsVisual,
) {
    // 主题分隔线是块级行；防御性结算它之前的文本字形。
    if !glyphs.is_empty() {
        // 复用普通文本行结算以保持前序几何。
        super::layout_metrics::flush_line(lines, glyphs, line_height, metrics);
    }
    // 新行顶部紧随前一行底部。
    let y = lines.last().map(|line| line.y + line.height).unwrap_or(0.0);
    // 登记不包含可选择字形的分隔线行。
    lines.push(LayoutLine {
        // 保存稳定的相对行顶部。
        y,
        // 使用普通默认行高并保证有限正值。
        height: line_height.max(1.0),
        // 标记该行由绘制层渲染主题线。
        kind: LayoutLineKind::ThematicBreak,
        // 分隔线零宽且不可字符级选中，因此没有字形。
        glyphs: Vec::new(),
    });
}

// 把主题分隔线的宽度约束规范为有限非负值。
pub(super) fn layout_width(max_width: f32) -> f32 {
    // 非有限约束不应污染组件的布局尺寸。
    if max_width.is_finite() {
        // 负约束按零宽处理。
        max_width.max(0.0)
    } else {
        // 无穷或非数字约束回退到安全零宽。
        0.0
    }
}

// 绘制布局中的全部主题分隔线行。
pub(super) fn draw(
    // 使用 RichText 当前绘制上下文与主题 token。
    ctx: &mut PaintContext,
    // 接收已经平移到组件 frame 的布局行。
    lines: &[LayoutLine],
    // 接收组件可见内容矩形。
    frame: Rect,
    // 接收 UIX 声明的分隔线几何。
    visual: RichTextBreakVisual,
    // 接收当前主题解析出的分隔线颜色。
    color: Color,
) {
    // 线宽不得为负数。
    let width = (frame.w - visual.horizontal_inset * 2.0).max(0.0);
    // 不可见宽度无需生成绘制命令。
    if width <= 0.0 {
        // 保持布局行高但跳过空绘制。
        return;
    }
    // 遍历布局行，只处理主题分隔线类型。
    for line in lines {
        // 普通文本行继续由 RichText 字形路径绘制。
        if line.kind != LayoutLineKind::ThematicBreak {
            // 跳过非分隔线行。
            continue;
        }
        // 一像素主题线在行盒内垂直居中。
        let y = frame.y + line.y + (line.height - visual.stroke) * 0.5;
        // 绘制无圆角的水平主题线。
        ctx.fill_rect(
            // 左右各保留十二像素并占满其余可用宽度。
            Rect::new(frame.x + visual.horizontal_inset, y, width, visual.stroke),
            // 使用主题最淡正文色。
            color,
            // 主题线不需要圆角半径。
            None,
        );
    }
}
