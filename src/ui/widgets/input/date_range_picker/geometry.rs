// 引入日期范围面板几何所需的矩形类型。
use crate::core::Rect;

// 引入 UIX 注入的组合面板完整视觉表。
use super::presentation::DateRangePickerVisual;

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
    let frame = normalize_date_range_rect(frame);
    // 归一化逻辑表面矩形。
    let surface = normalize_date_range_rect(surface);
    // 读取当前组合面板自然高度。
    let natural_height = natural_date_range_popup_height(preset_count, visual);
    // 将自然宽度限制在当前表面内。
    let width = frame.w.max(visual.calendar.min_width).min(surface.w);
    // 空表面不生成可见组合面板。
    if width <= 0.0 || surface.h <= 0.0 {
        // 返回稳定的空矩形。
        return Rect::zero();
    }
    // 计算允许的最右起点。
    let max_x = surface.x + surface.w - width;
    // 将触发器横向锚点限制在表面内。
    let x = frame.x.clamp(surface.x, max_x);
    // 计算保留间距后的下方空间。
    let available_below =
        (surface.y + surface.h - frame.y - frame.h - visual.popup.popup_gap).max(0.0);
    // 计算保留间距后的上方空间。
    let available_above = (frame.y - surface.y - visual.popup.popup_gap).max(0.0);
    // 优先完整向下，其次完整向上，均不足时选择空间更大的一侧。
    let place_below = if natural_height <= available_below {
        // 下方可以完整容纳自然高度。
        true
    // 检查上方是否可以完整容纳自然高度。
    } else if natural_height <= available_above {
        // 上方可以完整容纳时翻转。
        false
    // 两侧都不足时比较可用空间。
    } else {
        // 平局保持默认向下。
        available_below >= available_above
    };
    // 读取最终方向的可用高度。
    let available_height = if place_below {
        // 使用下方可用空间。
        available_below
    // 处理向上布局。
    } else {
        // 使用上方可用空间。
        available_above
    };
    // 将组合面板高度限制到最终方向的可用空间。
    let height = natural_height.min(available_height);
    // 按最终方向计算纵向起点。
    let y = if place_below {
        // 向下面板从触发器底边加间距开始。
        frame.y + frame.h + visual.popup.popup_gap
    // 处理向上布局。
    } else {
        // 向上面板紧贴触发器上方间距。
        frame.y - visual.popup.popup_gap - height
    };
    // 返回绘制、命中、脏区和登记共享的最终矩形。
    Rect::new(x, y, width, height)
}

// 将绝对组合面板转换为相对触发器原点的矩形。
pub(super) fn local_date_range_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 归一化触发器以保证偏移有限。
    let frame = normalize_date_range_rect(frame);
    // 保留面板尺寸并扣除触发器绝对原点。
    Rect::new(popup.x - frame.x, popup.y - frame.y, popup.w, popup.h)
}

// 将相对组合面板转换为窗口绝对矩形。
pub(super) fn absolute_date_range_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 叠加触发器绝对原点并保留面板尺寸。
    Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
}

// 计算触发器与组合面板共同占用的表面内矩形。
pub(super) fn date_range_surface_rect(frame: Rect, popup: Rect, surface: Rect) -> Rect {
    // 合并触发器与组合面板并裁剪到当前表面。
    normalize_date_range_rect(frame)
        // 合并绝对组合面板范围。
        .union(&normalize_date_range_rect(popup))
        // 裁掉表面外不可见区域。
        .intersect(&normalize_date_range_rect(surface))
        // 完全不相交时返回空矩形。
        .unwrap_or_default()
}

// 构造首次正式登记前的有限回退表面。
pub(super) fn date_range_fallback_surface(
    frame: Rect,
    preset_count: usize,
    visual: &DateRangePickerVisual,
) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_date_range_rect(frame);
    // 计算自然组合面板宽度。
    let width = frame.w.max(visual.calendar.min_width);
    // 读取当前组合面板自然高度。
    let height = natural_date_range_popup_height(preset_count, visual);
    // 在触发器上下各预留完整组合面板和间距。
    Rect::new(
        // 从触发器左边开始。
        frame.x,
        // 向上预留完整组合面板和间距。
        frame.y - height - visual.popup.popup_gap,
        // 保留自然组合面板宽度。
        width,
        // 覆盖上下两份面板、两份间距和触发器。
        height * 2.0 + visual.popup.popup_gap * 2.0 + frame.h,
    )
}

// 归一化日期范围面板相关矩形。
pub(super) fn normalize_date_range_rect(rect: Rect) -> Rect {
    // 替换非有限坐标并收敛负尺寸。
    Rect::new(
        // 非有限横坐标回退到原点。
        if rect.x.is_finite() { rect.x } else { 0.0 },
        // 非有限纵坐标回退到原点。
        if rect.y.is_finite() { rect.y } else { 0.0 },
        // 归一化宽度。
        finite_nonnegative(rect.w),
        // 归一化高度。
        finite_nonnegative(rect.h),
    )
}

// 将任意浮点尺寸收敛为有限非负值。
fn finite_nonnegative(value: f32) -> f32 {
    // 只保留有限输入。
    if value.is_finite() {
        // 负尺寸收敛为零。
        value.max(0.0)
    // 处理非有限输入。
    } else {
        // 非有限值回退为零。
        0.0
    }
}
