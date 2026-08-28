// 引入日期范围面板几何所需的矩形类型。
use crate::core::Rect;

// 引入 UIX 注入的组合面板完整视觉表。
use super::presentation::DateRangePickerVisual;

// 复用浮层共享几何纯函数：归一化、相对/绝对互换、表面合并与翻转决策。
use crate::ui::widgets::overlay::{
    absolute_rect, local_rect, normalize_rect, resolve_vertical_dropdown_rect, surface_union_rect,
    vertical_fallback_surface,
};

// 描述最终组合面板及其月历与预设分区。
#[derive(Debug, Clone, Copy)]
pub(super) struct DateRangePopupParts {
    // 保存最终月历分区矩形。
    pub(super) calendar: Rect,
    // 保存可选的最终预设页脚矩形。
    pub(super) footer: Option<Rect>,
    // 保存缩放后的预设垂直内边距。
    pub(super) preset_vertical_inset: f32,
    // 保存缩放后的预设行高。
    pub(super) preset_row_height: f32,
    // 保存缩放后的预设水平内边距。
    pub(super) preset_horizontal_inset: f32,
    // 保存缩放后的预设字号。
    pub(super) preset_font_size: f32,
}

// 返回指定预设数对应的组合面板自然高度。
pub(super) fn natural_date_range_popup_height(
    preset_count: usize,
    visual: &DateRangePickerVisual,
) -> f32 {
    // 无预设时只保留月历自然高度。
    if preset_count == 0 {
        // 返回月历自然高度。
        visual.calendar.height
    // 处理带预设页脚的组合面板。
    } else {
        // 合并月历、分区间隙、页脚内边距与全部预设行。
        visual.calendar.height
            // 加入月历与页脚之间的间隙。
            + visual.popup.preset_gap
            // 加入页脚上下内边距。
            + visual.popup.preset_vertical_inset * 2.0
            // 加入全部预设行的自然高度。
            + visual.popup.preset_row_height * preset_count as f32
    }
}

// 从最终组合面板派生绘制与命中共享的内部分区。
pub(super) fn date_range_popup_parts(
    // 接收最终组合面板矩形。
    popup: Rect,
    // 接收当前预设数量。
    preset_count: usize,
    // 接收同目录 UIX 注入的视觉表。
    visual: &DateRangePickerVisual,
) -> DateRangePopupParts {
    // 归一化最终组合面板矩形。
    let popup = normalize_date_range_rect(popup);
    // 读取当前组合面板自然高度。
    let natural_height = natural_date_range_popup_height(preset_count, visual);
    // 计算所有纵向分区共享的缩放比例。
    let y_scale = (popup.h / natural_height).clamp(0.0, 1.0);
    // 计算水平内边距与字号使用的宽度比例。
    let x_scale = (popup.w / visual.calendar.min_width).clamp(0.0, 1.0);
    // 计算最终月历高度。
    let calendar_height = visual.calendar.height * y_scale;
    // 构造最终月历分区。
    let calendar = Rect::new(popup.x, popup.y, popup.w, calendar_height);
    // 按当前预设数量构造可选页脚。
    let footer = (preset_count > 0).then(|| {
        // 计算缩放后的月历与页脚间隙。
        let gap = visual.popup.preset_gap * y_scale;
        // 计算缩放后的页脚内容高度。
        let height = (visual.popup.preset_vertical_inset * 2.0
            // 加入全部缩放预设行。
            + visual.popup.preset_row_height * preset_count as f32)
            // 应用组合面板纵向比例。
            * y_scale;
        // 返回与月历共用宽度的最终页脚矩形。
        Rect::new(popup.x, popup.y + calendar_height + gap, popup.w, height)
    });
    // 返回所有消费者共享的最终分区与指标。
    DateRangePopupParts {
        // 保存最终月历分区矩形。
        calendar,
        // 保存可选预设页脚矩形。
        footer,
        // 按纵向比例缩放页脚内边距。
        preset_vertical_inset: visual.popup.preset_vertical_inset * y_scale,
        // 按纵向比例缩放预设行高。
        preset_row_height: visual.popup.preset_row_height * y_scale,
        // 按横向比例缩放预设水平内边距。
        preset_horizontal_inset: visual.popup.preset_horizontal_inset * x_scale,
        // 字号使用较小轴比例避免裁切。
        preset_font_size: visual.popup.preset_font_size * x_scale.min(y_scale),
    }
}

// 使用当前逻辑表面解析日期范围组合面板的最终绝对矩形。
pub(super) fn resolve_date_range_popup_rect(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收当前预设数量。
    preset_count: usize,
    // 接收当前逻辑表面。
    surface: Rect,
    // 接收同目录 UIX 注入的视觉表。
    visual: &DateRangePickerVisual,
) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_rect(frame);
    // 归一化逻辑表面矩形。
    let surface = normalize_rect(surface);
    // 读取当前组合面板自然高度。
    let natural_height = natural_date_range_popup_height(preset_count, visual);
    // 复用共享垂直下拉解析：保留间距、优先向下、放不下翻向上、两侧不足取大侧。
    // 双月布局与预设页脚的纵向分区差异由 date_range_popup_parts 单独处理，
    // 弹层外框本身与其它日期面板共享同一翻转与钳制语义。
    resolve_vertical_dropdown_rect(
        frame,
        surface,
        frame.w.max(visual.calendar.min_width),
        natural_height,
        frame.y + frame.h,
        visual.popup.popup_gap,
    )
}

// 将绝对组合面板转换为相对触发器原点的矩形。
pub(super) fn local_date_range_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 复用共享相对转换。
    local_rect(frame, popup)
}

// 将相对组合面板转换为窗口绝对矩形。
pub(super) fn absolute_date_range_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 复用共享绝对转换。
    absolute_rect(frame, popup)
}

// 计算触发器与组合面板共同占用的表面内矩形。
pub(super) fn date_range_surface_rect(frame: Rect, popup: Rect, surface: Rect) -> Rect {
    // 复用共享表面合并。
    surface_union_rect(frame, popup, surface)
}

// 构造首次正式登记前的有限回退表面。
pub(super) fn date_range_fallback_surface(
    frame: Rect,
    preset_count: usize,
    visual: &DateRangePickerVisual,
) -> Rect {
    // 计算自然组合面板宽度。
    let width = frame.w.max(visual.calendar.min_width);
    // 读取当前组合面板自然高度。
    let height = natural_date_range_popup_height(preset_count, visual);
    // 复用共享回退表面：在触发器上下各预留完整组合面板和间距（既有两侧份量固定为 2）。
    vertical_fallback_surface(frame, width, height, visual.popup.popup_gap, 2.0)
}

// 归一化日期范围面板相关矩形。
pub(super) fn normalize_date_range_rect(rect: Rect) -> Rect {
    // 复用浮层共享归一化：非有限坐标回退原点、尺寸收敛为有限非负。
    normalize_rect(rect)
}
