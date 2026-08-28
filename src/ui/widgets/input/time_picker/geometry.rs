// 引入时间面板几何所需的矩形类型。
use crate::core::Rect;

// 引入 UIX 注入的时间面板视觉表。
use super::presentation::TimePickerVisual;

// 复用浮层共享几何纯函数：归一化、相对/绝对互换、表面合并与翻转决策。
use crate::ui::widgets::overlay::{
    absolute_rect, local_rect, normalize_rect, resolve_vertical_dropdown_rect, surface_union_rect,
    vertical_fallback_surface,
};

// 使用当前逻辑表面解析时间面板的最终绝对矩形。
pub(super) fn resolve_time_popup_rect(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收当前逻辑表面。
    surface: Rect,
    // 接收 UIX 声明的面板尺寸与间隙。
    visual: &TimePickerVisual,
) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_rect(frame);
    // 归一化逻辑表面矩形。
    let surface = normalize_rect(surface);
    // 复用共享垂直下拉解析：保留间隙、优先向下、放不下翻向上、两侧不足取大侧。
    // 时间面板高度由视觉表固定声明。
    resolve_vertical_dropdown_rect(
        frame,
        surface,
        frame.w.max(visual.layout.popup_min_width),
        visual.layout.popup_height,
        frame.y + frame.h,
        visual.layout.popup_gap,
    )
}

// 将绝对时间面板转换为相对触发器原点的矩形。
pub(super) fn local_time_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 复用共享相对转换。
    local_rect(frame, popup)
}

// 将相对时间面板转换为窗口绝对矩形。
pub(super) fn absolute_time_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 复用共享绝对转换。
    absolute_rect(frame, popup)
}

// 计算触发器与时间面板共同占用的表面内矩形。
pub(super) fn time_surface_rect(frame: Rect, popup: Rect, surface: Rect) -> Rect {
    // 复用共享表面合并。
    surface_union_rect(frame, popup, surface)
}

// 构造首次正式登记前的有限回退表面。
pub(super) fn time_fallback_surface(frame: Rect, visual: &TimePickerVisual) -> Rect {
    // 计算自然时间面板宽度。
    let width = frame.w.max(visual.layout.popup_min_width);
    // 复用共享回退表面：在触发器上下各预留完整时间面板和间隙（既有两侧份量固定为 2）。
    vertical_fallback_surface(
        frame,
        width,
        visual.layout.popup_height,
        visual.layout.popup_gap,
        2.0,
    )
}

// 归一化时间面板相关矩形。
pub(super) fn normalize_time_rect(rect: Rect) -> Rect {
    // 复用浮层共享归一化：非有限坐标回退原点、尺寸收敛为有限非负。
    normalize_rect(rect)
}
