//! 反馈组件：弹层、提示与加载状态。

// 引入反馈组件共享的几何与已解析绘制颜色值。
use crate::core::{Point, Rect};
use crate::draw::Color;
use crate::ui::widget_runtime::paint_context::PaintContext;

// 按动画不透明度衰减主题 token 已有的 alpha。
pub(crate) fn fade_token_color(color: Color, opacity: f32) -> Color {
    // 将动画范围限制到有效不透明度区间。
    let opacity = opacity.clamp(0.0, 1.0);
    // 保留 token 自身基础 alpha，只缩放当前动画进度。
    let alpha = (f32::from(color.a) * opacity).round().clamp(0.0, 255.0) as u8;
    // 返回保持原 RGB 的动画颜色。
    color.with_alpha(alpha)
}

// 在矩形内绘制保守省略后的单行文本；`centered` 时水平居中，否则左对齐垂直居中。
// 反馈组件通用的阴影/弹层文字绘制入口，收敛各组件曾经的重复实现。
pub(crate) fn paint_elided_text(
    ctx: &mut PaintContext,
    value: &str,
    frame: Rect,
    color: Color,
    font_size: f32,
    centered: bool,
) {
    // 无可见文本或零面积矩形直接早退，保持各组件原有的不绘制策略。
    if value.is_empty() || frame.w <= 0.0 || frame.h <= 0.0 {
        return;
    }
    // 复用 UI 绘制上下文拥有的保守单行省略算法。
    let Some(value) = ctx.elide_single_line_cow(value, font_size, frame.w) else {
        return;
    };
    ctx.push_clip(frame);
    if centered {
        ctx.text_center(value.as_ref(), frame, color, font_size);
    } else {
        let y = ctx.visual_center_y(frame, font_size);
        ctx.draw_text(value.as_ref(), Point::new(frame.x, y), color, font_size);
    }
    ctx.pop_clip();
}

// 把矩形四周外扩 `amount`，用于阴影边界；退化矩形不产生外扩结果。
pub(crate) fn expand_rect(rect: Rect, amount: f32) -> Rect {
    if rect.w <= 0.0 || rect.h <= 0.0 {
        Rect::zero()
    } else {
        Rect::new(
            rect.x - amount,
            rect.y - amount,
            rect.w + amount * 2.0,
            rect.h + amount * 2.0,
        )
    }
}

pub mod alert;
// 声明租约组件与关闭事实共享反馈 capability。
pub mod declaration;
/// 抽屉式窗口内浮层组件。
pub mod drawer;
pub mod message;
/// 模态对话框与焦点陷阱组件。
pub mod modal;
pub mod notification;
/// 由触发器拥有的气泡确认组件。
pub mod popconfirm;
/// 由触发器拥有的通用气泡内容组件。
pub mod popover;
pub mod progress;
pub mod spin;
pub(crate) mod toast_motion;
/// 提示文字浮层组件。
pub mod tooltip;

pub use alert::*;
pub use declaration::*;
pub use drawer::*;
pub use message::*;
pub use modal::*;
pub use notification::*;
pub use popconfirm::*;
pub use popover::*;
pub use progress::*;
pub use spin::*;
pub use tooltip::*;
