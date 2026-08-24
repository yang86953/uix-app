// 引入矩形基础类型。
use crate::core::Rect;

// 引入 UIX 生成的唯一自动完成视觉表。
use super::AUTOCOMPLETE_VISUAL_REF;

// 使用当前逻辑表面解析自动完成弹层最终绝对矩形。
pub(super) fn resolve_autocomplete_popup_rect(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收当前候选行数。
    item_count: usize,
    // 接收当前窗口逻辑表面。
    surface: Rect,
    // 返回限制在表面内的绝对弹层矩形。
) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_autocomplete_rect(frame);
    // 归一化逻辑表面矩形。
    let surface = normalize_autocomplete_rect(surface);
    // 计算既有规格要求的自然宽度。
    let layout = AUTOCOMPLETE_VISUAL_REF.layout;
    let natural_width = frame.w.max(layout.min_popup_width);
    // 计算至少一行且不超过最大视口的自然高度。
    let natural_height = (item_count.max(1) as f32 * layout.row_height)
        // 限制到既有最大自然视口高度。
        .min(layout.max_popup_height);
    // 将弹层宽度限制在当前表面内。
    let width = finite_nonnegative(natural_width).min(surface.w);
    // 无可用表面时返回稳定空矩形。
    if width <= 0.0 || surface.h <= 0.0 {
        // 空弹层不参与绘制和命中。
        return Rect::zero();
    }

    // 计算横向起点允许的最大值。
    let max_x = surface.x + surface.w - width;
    // 将触发器锚点横向收敛到当前表面。
    let x = frame.x.clamp(surface.x, max_x);
    // 计算触发器下方可用高度。
    let available_below = (surface.y + surface.h - frame.y - frame.h).max(0.0);
    // 计算触发器上方可用高度。
    let available_above = (frame.y - surface.y).max(0.0);
    // 优先完整向下；否则完整向上；两侧都不足时选择更大空间。
    let place_below = if natural_height <= available_below {
        // 下方完整容纳自然高度时保持默认方向。
        true
    } else if natural_height <= available_above {
        // 只有上方完整容纳时翻转。
        false
    } else {
        // 两侧都不足时选择空间更大的一侧，平局保持向下。
        available_below >= available_above
    };
    // 读取最终方向的实际可用高度。
    let available_height = if place_below {
        // 使用触发器下方空间。
        available_below
    } else {
        // 使用触发器上方空间。
        available_above
    };
    // 将自然高度限制在最终方向的可用空间内。
    let height = natural_height.min(available_height).max(0.0);
    // 计算最终绝对纵坐标。
    let y = if place_below {
        // 向下弹层紧贴触发器底边。
        frame.y + frame.h
    } else {
        // 向上弹层用实际高度紧贴触发器顶边。
        frame.y - height
    };

    // 返回所有消费者共享的最终绝对矩形。
    Rect::new(x, y, width, height)
}

// 将绝对弹层矩形转换为相对触发器原点的缓存。
pub(super) fn local_autocomplete_popup_rect(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收已经解析的绝对弹层矩形。
    popup: Rect,
    // 返回可供组件本地事件复用的矩形。
) -> Rect {
    // 归一化触发器以保证偏移有限。
    let frame = normalize_autocomplete_rect(frame);
    // 从绝对坐标扣除触发器原点并保留最终尺寸。
    Rect::new(
        // 保存横向相对偏移。
        popup.x - frame.x,
        // 保存纵向相对偏移。
        popup.y - frame.y,
        // 保留最终宽度。
        popup.w,
        // 保留最终高度。
        popup.h,
    )
}

// 将相对触发器的弹层矩形转换为窗口绝对坐标。
pub(super) fn absolute_autocomplete_popup_rect(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收相对触发器的弹层矩形。
    popup: Rect,
    // 返回窗口绝对矩形。
) -> Rect {
    // 叠加触发器原点并保留最终尺寸。
    Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
}

// 计算触发器与弹层共同占用的表面内矩形。
pub(super) fn autocomplete_surface_rect(
    // 接收触发器绝对布局矩形。
    frame: Rect,
    // 接收绝对弹层或保守脏区矩形。
    popup: Rect,
    // 接收当前逻辑表面。
    surface: Rect,
    // 返回限制在当前表面内的合并矩形。
) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_autocomplete_rect(frame);
    // 归一化弹层矩形。
    let popup = normalize_autocomplete_rect(popup);
    // 归一化逻辑表面矩形。
    let surface = normalize_autocomplete_rect(surface);
    // 合并触发器与弹层并裁到表面。
    frame
        // 将弹层并入占用区域。
        .union(&popup)
        // 裁掉逻辑表面外不可见区域。
        .intersect(&surface)
        // 完全不相交时返回空矩形。
        .unwrap_or_default()
}

// 构造尚未取得真实窗口表面时的有限回退表面。
pub(super) fn autocomplete_fallback_surface(frame: Rect, item_count: usize) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_autocomplete_rect(frame);
    // 计算既有规格要求的自然宽度。
    let layout = AUTOCOMPLETE_VISUAL_REF.layout;
    let width = frame.w.max(layout.min_popup_width);
    // 计算当前行数对应的自然视口高度。
    let height = (item_count.max(1) as f32 * layout.row_height)
        // 限制到既有最大自然视口高度。
        .min(layout.max_popup_height);
    // 在触发器上下各预留一份自然弹层空间。
    Rect::new(
        // 横向从触发器左边开始。
        frame.x,
        // 纵向向上预留完整弹层高度。
        frame.y - height,
        // 保留自然弹层宽度。
        width,
        // 覆盖上下两份弹层与触发器。
        height * layout.fallback_popup_sides + frame.h,
    )
}

// 归一化自动完成相关矩形。
pub(super) fn normalize_autocomplete_rect(rect: Rect) -> Rect {
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
        // 负值收敛为零。
        value.max(0.0)
    } else {
        // 非有限值回退为零。
        0.0
    }
}
