// 引入当前绘制字体句柄与 UI 绘制上下文。
use crate::{draw::FontHandle, ui::widget_runtime::paint_context::PaintContext};
// 引入 UI System 自有的字体族列表契约。
use crate::ui::theme::style::FontFamily;

// 把可选 UI 字体族列表单向解析为 draw System 字体句柄。
pub fn resolve(
    // 接收当前绘制上下文以访问字体服务和稳定后备句柄。
    ctx: &mut PaintContext<'_, '_>,
    // 借用显式字体族列表；None 保留当前系统字体。
    family: Option<&FontFamily>,
) -> FontHandle {
    // 复制当前字体作为列表全部不可用时的稳定后备。
    let fallback = *ctx.font();
    // 未声明字体族时不查询注册表。
    let Some(family) = family else {
        // 保留组件进入绘制阶段时的字体身份。
        return fallback;
    };
    // draw 只接收中性族名迭代器，不依赖 UI FontFamily 类型。
    ctx.font_service()
        // 按声明顺序解析第一个已加载字体族。
        .resolve_font_families(family.iter(), fallback)
}
