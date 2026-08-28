// 引入日期面板几何所需的矩形类型。
use crate::core::Rect;

// 引入 UIX 注入的共享月历视觉指标。
use crate::ui::widgets::input::date_calendar::CalendarPanelVisual;

// 复用浮层共享几何纯函数：归一化、相对/绝对互换、表面合并与翻转决策。
use crate::ui::widgets::overlay::{
    absolute_rect, local_rect, normalize_rect, resolve_vertical_dropdown_rect, surface_union_rect,
    vertical_fallback_surface,
};

// 使用当前逻辑表面解析日期面板的最终绝对矩形。
pub(super) fn resolve_date_picker_popup_rect(
    frame: Rect,
    surface: Rect,
    calendar: CalendarPanelVisual,
    popup_gap: f32,
) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_rect(frame);
    // 归一化逻辑表面矩形。
    let surface = normalize_rect(surface);
    // 复用共享垂直下拉解析：保留间距、优先向下、放不下翻向上、两侧不足取大侧。
    // 月历自然高度由视觉表固定声明。
    resolve_vertical_dropdown_rect(
        frame,
        surface,
        frame.w.max(calendar.min_width),
        calendar.height,
        frame.y + frame.h,
        popup_gap,
    )
}

// 将绝对日期面板转换为相对触发器原点的矩形。
pub(super) fn local_date_picker_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 复用共享相对转换。
    local_rect(frame, popup)
}

// 将相对日期面板转换为窗口绝对矩形。
pub(super) fn absolute_date_picker_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 复用共享绝对转换。
    absolute_rect(frame, popup)
}

// 计算触发器与面板共同占用的表面内矩形。
pub(super) fn date_picker_surface_rect(frame: Rect, popup: Rect, surface: Rect) -> Rect {
    // 复用共享表面合并。
    surface_union_rect(frame, popup, surface)
}

// 构造首次正式登记前的有限回退表面。
pub(super) fn date_picker_fallback_surface(
    frame: Rect,
    calendar: CalendarPanelVisual,
    popup_gap: f32,
) -> Rect {
    // 计算自然面板宽度。
    let width = frame.w.max(calendar.min_width);
    // 复用共享回退表面：在触发器上下各预留完整面板和间距（既有两侧份量固定为 2）。
    vertical_fallback_surface(frame, width, calendar.height, popup_gap, 2.0)
}

// 归一化日期面板相关矩形。
pub(super) fn normalize_date_picker_rect(rect: Rect) -> Rect {
    // 复用浮层共享归一化：非有限坐标回退原点、尺寸收敛为有限非负。
    normalize_rect(rect)
}
