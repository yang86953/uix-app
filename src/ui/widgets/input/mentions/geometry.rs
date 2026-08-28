// 引入矩形基础类型。
use crate::core::Rect;

// 引入 UIX 注入的提及组件视觉表。
use super::presentation::MentionsVisual;

// 复用浮层共享几何纯函数：归一化、相对/绝对互换、表面合并与翻转决策。
use crate::ui::widgets::overlay::{
    absolute_rect, local_rect, normalize_rect, resolve_vertical_dropdown_rect, surface_union_rect,
    vertical_fallback_surface,
};

// 使用当前逻辑表面解析提及弹层最终绝对矩形。
pub(super) fn resolve_mentions_popup_rect(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收当前候选行数。
    item_count: usize,
    // 接收当前窗口逻辑表面。
    surface: Rect,
    // 接收同目录 UIX 注入的视觉表。
    visual: &MentionsVisual,
    // 返回限制在表面内的绝对弹层矩形。
) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_rect(frame);
    // 归一化逻辑表面矩形。
    let surface = normalize_rect(surface);
    // 计算既有规格要求的自然宽度。
    let natural_width = frame.w.max(visual.layout.min_popup_width);
    // 计算至少一行且不超过最大视口的自然高度。
    let natural_height = (item_count.max(1) as f32 * visual.layout.suggestion_row_height)
        // 限制到既有最大自然视口高度。
        .min(visual.layout.max_popup_height);
    // 复用共享垂直下拉解析：优先向下、放不下翻向上、两侧不足取大侧并钳制表面。
    resolve_vertical_dropdown_rect(
        frame,
        surface,
        natural_width,
        natural_height,
        frame.y + frame.h,
        0.0,
    )
}

// 将绝对弹层矩形转换为相对触发器原点的缓存。
pub(super) fn local_mentions_popup_rect(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收已经解析的绝对弹层矩形。
    popup: Rect,
    // 返回可供组件本地事件复用的矩形。
) -> Rect {
    // 复用共享相对转换。
    local_rect(frame, popup)
}

// 将相对触发器的弹层矩形转换为窗口绝对坐标。
pub(super) fn absolute_mentions_popup_rect(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收相对触发器的弹层矩形。
    popup: Rect,
    // 返回窗口绝对矩形。
) -> Rect {
    // 复用共享绝对转换。
    absolute_rect(frame, popup)
}

// 计算触发器与弹层共同占用的表面内矩形。
pub(super) fn mentions_surface_rect(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收绝对弹层或保守脏区矩形。
    popup: Rect,
    // 接收当前逻辑表面。
    surface: Rect,
    // 返回限制在当前表面内的合并矩形。
) -> Rect {
    // 复用共享表面合并。
    surface_union_rect(frame, popup, surface)
}

// 构造尚未取得真实窗口表面时的有限回退表面。
pub(super) fn mentions_fallback_surface(
    frame: Rect,
    item_count: usize,
    visual: &MentionsVisual,
) -> Rect {
    // 计算既有规格要求的自然宽度。
    let width = frame.w.max(visual.layout.min_popup_width);
    // 计算当前行数对应的自然视口高度。
    let height = (item_count.max(1) as f32 * visual.layout.suggestion_row_height)
        // 限制到既有最大自然视口高度。
        .min(visual.layout.max_popup_height);
    // 复用共享回退表面：在触发器上下各预留一份自然弹层空间（既有两侧份量固定为 2）。
    vertical_fallback_surface(frame, width, height, 0.0, 2.0)
}

// 归一化提及弹层相关矩形。
pub(super) fn normalize_mentions_rect(rect: Rect) -> Rect {
    // 复用浮层共享归一化：非有限坐标回退原点、尺寸收敛为有限非负。
    normalize_rect(rect)
}
