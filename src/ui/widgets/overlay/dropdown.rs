//! 垂直下拉弹层的共享几何纯函数。
//!
//! 单一实现承载输入弹层族的矩形归一化、有限收敛、相对/绝对互换、表面合并、
//! 「优先向下、放不下翻向上、两侧不足取大侧、钳制表面」的翻转决策与回退
//! 表面构造。各组件保留自己的解析入口：自然宽高来自各自的视觉表与内容，
//! 动画偏移与命中结构等差异也留在组件侧。

use crate::core::Rect;

// 将任意浮点值收敛为有限非负值。
pub(crate) fn finite_nonnegative(value: f32) -> f32 {
    // 只保留有限输入。
    if value.is_finite() {
        // 负值收敛为零。
        value.max(0.0)
    } else {
        // 非有限值回退为零。
        0.0
    }
}

// 归一化浮层相关矩形：非有限坐标回退原点、尺寸收敛为有限非负。
pub(crate) fn normalize_rect(rect: Rect) -> Rect {
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

// 将绝对弹层矩形转换为相对触发器原点的矩形。
pub(crate) fn local_rect(frame: Rect, popup: Rect) -> Rect {
    // 归一化触发器以保证偏移有限。
    let frame = normalize_rect(frame);
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
pub(crate) fn absolute_rect(frame: Rect, popup: Rect) -> Rect {
    // 叠加触发器原点并保留最终尺寸。
    Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
}

// 计算触发器与弹层共同占用的表面内矩形。
pub(crate) fn surface_union_rect(frame: Rect, popup: Rect, surface: Rect) -> Rect {
    // 合并触发器与弹层并裁到表面。
    normalize_rect(frame)
        // 将弹层并入占用区域。
        .union(&normalize_rect(popup))
        // 裁掉逻辑表面外不可见区域。
        .intersect(&normalize_rect(surface))
        // 完全不相交时返回空矩形。
        .unwrap_or_default()
}

// 构造垂直下拉族共享的回退表面：上下各预留 sides 份自然弹层与间距。
pub(crate) fn vertical_fallback_surface(
    frame: Rect,
    popup_width: f32,
    popup_height: f32,
    gap: f32,
    sides: f32,
) -> Rect {
    // 归一化触发器矩形。
    let frame = normalize_rect(frame);
    // 在触发器上下各预留完整弹层与间距。
    Rect::new(
        // 横向从触发器左边开始。
        frame.x,
        // 纵向向上预留完整弹层与间距。
        frame.y - popup_height - gap,
        // 保留自然弹层宽度。
        popup_width,
        // 覆盖上下多份弹层、对应间距和触发器。
        popup_height * sides + gap * sides + frame.h,
    )
}

// 共享垂直翻转决策：优先完整向下，其次完整向上，两侧都不足时取更大空间，平局向下。
// `anchor_bottom` 是弹层下贴的触发器底边（可能带控件实际高度），
// `frame_top` 是触发器顶边；两侧可用空间均已扣除间距。
pub(crate) fn fit_vertical_dropdown(
    frame_top: f32,
    anchor_bottom: f32,
    surface: Rect,
    gap: f32,
    natural_height: f32,
) -> (bool, f32) {
    // 计算保留间距后的下方可用高度。
    let available_below = (surface.y + surface.h - anchor_bottom - gap).max(0.0);
    // 计算保留间距后的上方可用高度。
    let available_above = (frame_top - surface.y - gap).max(0.0);
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
    // 返回最终方向与其可用高度。
    if place_below {
        (true, available_below)
    } else {
        (false, available_above)
    }
}

// 使用共享翻转决策解析垂直下拉弹层的最终绝对矩形。
// 调用方先归一化 frame 与 surface；空表面或不可见高度返回稳定空矩形。
pub(crate) fn resolve_vertical_dropdown_rect(
    frame: Rect,
    surface: Rect,
    natural_width: f32,
    natural_height: f32,
    anchor_bottom: f32,
    gap: f32,
) -> Rect {
    // 将弹层宽度限制在当前表面内。
    let width = natural_width.min(surface.w).max(0.0);
    // 无可用表面或不可见高度时返回稳定空矩形。
    if width <= 0.0 || surface.h <= 0.0 || natural_height <= 0.0 {
        // 空弹层不参与绘制和命中。
        return Rect::zero();
    }
    // 计算横向起点允许的最大值并将触发器锚点收敛到当前表面。
    let max_x = surface.x + surface.w - width;
    let x = frame.x.clamp(surface.x, max_x);
    // 读取共享翻转决策的最终方向与可用高度。
    let (place_below, available_height) =
        fit_vertical_dropdown(frame.y, anchor_bottom, surface, gap, natural_height);
    // 将自然高度限制在最终方向的可用空间内。
    let height = natural_height.min(available_height).max(0.0);
    // 按最终方向计算纵向起点。
    let y = if place_below {
        // 向下弹层紧贴触发器底边（含间距）。
        anchor_bottom + gap
    } else {
        // 向上弹层用受限高度紧贴触发器上方间距。
        frame.y - gap - height
    };
    // 返回绘制、命中、滚动、脏区和登记共享的最终矩形。
    Rect::new(x, y, width, height)
}
