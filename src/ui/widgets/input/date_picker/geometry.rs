// 引入日期面板几何所需的矩形类型。
use crate::core::Rect;

// 引入 UIX 注入的共享月历视觉指标。
use crate::ui::widgets::input::date_calendar::CalendarPanelVisual;

// 使用当前逻辑表面解析日期面板的最终绝对矩形。
pub(super) fn resolve_date_picker_popup_rect(
    frame: Rect,
    surface: Rect,
    calendar: CalendarPanelVisual,
    popup_gap: f32,
) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_date_picker_rect(frame);
    // 归一化逻辑表面矩形。
    let surface = normalize_date_picker_rect(surface);
    // 将自然宽度限制在当前表面内。
    let width = frame.w.max(calendar.min_width).min(surface.w);
    // 空表面不生成可见日期面板。
    if width <= 0.0 || surface.h <= 0.0 {
        // 返回稳定的空矩形。
        return Rect::zero();
    }
    // 计算允许的最右起点。
    let max_x = surface.x + surface.w - width;
    // 将触发器横向锚点限制在表面内。
    let x = frame.x.clamp(surface.x, max_x);
    // 计算保留间距后的下方空间。
    let available_below = (surface.y + surface.h - frame.y - frame.h - popup_gap).max(0.0);
    // 计算保留间距后的上方空间。
    let available_above = (frame.y - surface.y - popup_gap).max(0.0);
    // 优先完整向下，其次完整向上，均不足时选择空间更大的一侧。
    let place_below = if calendar.height <= available_below {
        // 下方可以完整容纳自然高度。
        true
    // 检查上方是否可以完整容纳自然高度。
    } else if calendar.height <= available_above {
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
    // 将面板高度限制到最终方向的可用空间。
    let height = calendar.height.min(available_height);
    // 按最终方向计算纵向起点。
    let y = if place_below {
        // 向下面板从触发器底边加间距开始。
        frame.y + frame.h + popup_gap
    // 处理向上布局。
    } else {
        // 向上面板紧贴触发器上方间距。
        frame.y - popup_gap - height
    };
    // 返回绘制、命中、脏区和登记共享的最终矩形。
    Rect::new(x, y, width, height)
}

// 将绝对日期面板转换为相对触发器原点的矩形。
pub(super) fn local_date_picker_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 归一化触发器以保证偏移有限。
    let frame = normalize_date_picker_rect(frame);
    // 保留面板尺寸并扣除触发器绝对原点。
    Rect::new(popup.x - frame.x, popup.y - frame.y, popup.w, popup.h)
}

// 将相对日期面板转换为窗口绝对矩形。
pub(super) fn absolute_date_picker_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 叠加触发器绝对原点并保留面板尺寸。
    Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
}

// 计算触发器与面板共同占用的表面内矩形。
pub(super) fn date_picker_surface_rect(frame: Rect, popup: Rect, surface: Rect) -> Rect {
    // 合并触发器与面板并裁剪到当前表面。
    normalize_date_picker_rect(frame)
        // 合并绝对面板范围。
        .union(&normalize_date_picker_rect(popup))
        // 裁掉表面外不可见区域。
        .intersect(&normalize_date_picker_rect(surface))
        // 完全不相交时返回空矩形。
        .unwrap_or_default()
}

// 构造首次正式登记前的有限回退表面。
pub(super) fn date_picker_fallback_surface(
    frame: Rect,
    calendar: CalendarPanelVisual,
    popup_gap: f32,
) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_date_picker_rect(frame);
    // 计算自然面板宽度。
    let width = frame.w.max(calendar.min_width);
    // 在触发器上下各预留完整面板和间距。
    Rect::new(
        // 从触发器左边开始。
        frame.x,
        // 向上预留完整面板和间距。
        frame.y - calendar.height - popup_gap,
        // 保留自然面板宽度。
        width,
        // 覆盖上下两份面板、两份间距和触发器。
        calendar.height * 2.0 + popup_gap * 2.0 + frame.h,
    )
}

// 归一化日期面板相关矩形。
pub(super) fn normalize_date_picker_rect(rect: Rect) -> Rect {
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
