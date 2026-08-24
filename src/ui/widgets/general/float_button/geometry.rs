//! FloatButton 的单一几何解析入口。

// 引入组件所需的点与矩形值类型。
use crate::core::{Point, Rect};
// 引入 overlay 模块拥有的窗口放置语义。
use crate::ui::Placement;
// 引入 UIX 生成的单一几何视觉记录。
use super::FloatButtonGeometryVisual;

// 保存一次 FloatButton 几何解析需要的只读输入。
pub(super) struct FloatButtonGeometryInput<'a> {
    // 保存组件树分配的布局矩形。
    pub(super) frame: Rect,
    // 保存当前窗口逻辑表面。
    pub(super) surface: Rect,
    // 保存作者显式选择的窗口放置方向。
    pub(super) placement: Option<Placement>,
    // 保存作者相对锚点声明的有限偏移。
    pub(super) offset: Point,
    // 保存按钮直径。
    pub(super) size: f32,
    // 借用可选展开说明文字。
    pub(super) description: &'a str,
    // 借用可选提示文字。
    pub(super) tooltip: &'a str,
    // 保存非负数字徽标值。
    pub(super) badge_count: i32,
    // 保存圆点徽标开关。
    pub(super) badge_dot: bool,
    // 标记按钮是否由 FloatButtonGroup 拥有相对布局。
    pub(super) in_group: bool,
    // 标记按钮是否参与普通布局占位。
    pub(super) reserve_layout_space: bool,
    // 保存 UIX 声明的全部几何参数。
    pub(super) visual: FloatButtonGeometryVisual,
}

// 保存绘制、命中、损伤与浮层登记共同消费的几何结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FloatButtonGeometry {
    // 保存完整可交互按钮区域。
    pub(super) control: Rect,
    // 保存 Lucide 图标绘制区域。
    pub(super) icon: Rect,
    // 保存可选说明文字绘制区域。
    pub(super) description: Option<Rect>,
    // 保存可选徽标绘制区域。
    pub(super) badge: Option<Rect>,
    // 保存可选提示框绘制区域。
    pub(super) tooltip: Option<Rect>,
    // 保存包含阴影、徽标和提示框的完整损伤区域。
    pub(super) paint_bounds: Rect,
}

// 从 authored config 与当前 surface 解析唯一几何结果。
pub(super) fn resolve_float_button_geometry(
    // 接收全部只读解析输入。
    input: FloatButtonGeometryInput<'_>,
    // 返回共享几何结果。
) -> FloatButtonGeometry {
    // 判断当前按钮能否使用窗口锚定语义。
    let uses_surface_placement = input.placement.is_some()
        // 组内按钮继续服从父组件的相对布局。
        && !input.in_group
        // 普通布局占位按钮继续服从布局 frame。
        && !input.reserve_layout_space
        // 只有有效表面才能提供窗口锚点。
        && valid_surface(input.surface);
    // 计算说明文字要求的自然控件宽度。
    let natural_width = control_width(input.size, input.description, &input.visual);
    // 按锚定模式或兼容模式解析控件区域。
    let control = if uses_surface_placement {
        // 显式 placement 使用当前窗口逻辑表面。
        anchored_control(&input, natural_width)
    } else {
        // 未显式 placement 时保留既有 frame-relative 行为。
        legacy_control(input.frame, input.offset, natural_width, input.size)
    };
    // 从最终控件高度收敛图标正方形边长。
    let icon_size = input.size.min(control.h).min(control.w).max(0.0);
    // 把图标固定在控件起始侧。
    let icon = Rect::new(control.x, control.y, icon_size, control.h);
    // 从控件剩余空间派生说明区域。
    let description = description_rect(control, icon, input.description, &input.visual);
    // 从最终控件右上角派生徽标区域。
    let badge = badge_rect(control, input.badge_count, input.badge_dot, &input.visual);
    // 从最终控件与当前表面派生提示框区域。
    let tooltip = tooltip_rect(
        // 传入最终按钮区域。
        control,
        // 传入当前逻辑表面。
        input.surface,
        // 传入显式 placement 以选择提示框侧向。
        input.placement,
        // 传入提示文字。
        input.tooltip,
        // 传入是否使用表面锚定。
        uses_surface_placement,
        // 传入 UIX 几何参数。
        &input.visual,
    );
    // 从控件阴影开始建立保守损伤区域。
    let mut paint_bounds = shadow_bounds(control, &input.visual);
    // 徽标存在时合并其绘制区域。
    if let Some(rect) = badge {
        // 把徽标纳入损伤区域。
        paint_bounds = paint_bounds.union(&rect);
    }
    // 提示框存在时合并其绘制区域。
    if let Some(rect) = tooltip {
        // 把提示框纳入损伤区域。
        paint_bounds = paint_bounds.union(&rect);
    }
    // 表面锚定模式下把损伤收敛到当前窗口客户区。
    if uses_surface_placement {
        // 窗口外绘制会被裁剪，因此不应扩大 damage。
        paint_bounds = paint_bounds.intersect(&input.surface).unwrap_or_default();
    }
    // 返回所有调用方共享的不可变几何。
    FloatButtonGeometry {
        // 返回最终交互区域。
        control,
        // 返回最终图标区域。
        icon,
        // 返回可选说明区域。
        description,
        // 返回可选徽标区域。
        badge,
        // 返回可选提示框区域。
        tooltip,
        // 返回最终损伤区域。
        paint_bounds,
    }
}

// 计算带可选说明文字的自然控件宽度。
fn control_width(
    // 接收按钮直径。
    size: f32,
    // 接收说明文字。
    description: &str,
    // 借用 UIX 几何参数。
    visual: &FloatButtonGeometryVisual,
    // 返回自然宽度。
) -> f32 {
    // 空说明保持圆形按钮。
    if description.is_empty() {
        // 返回直径作为宽度。
        return size;
    }
    // 按 Unicode 字符数估算稳定说明宽度。
    let text_width = description.chars().count() as f32 * visual.average_character_width;
    // 合并图标、间距、文字与尾部留白。
    size + visual.description_gap + text_width + visual.description_trailing_padding
}

// 按窗口 placement 解析表面内控件区域。
fn anchored_control(
    // 接收完整解析输入。
    input: &FloatButtonGeometryInput<'_>,
    // 接收自然控件宽度。
    natural_width: f32,
    // 返回表面内控件矩形。
) -> Rect {
    // 有效调用必然携带显式 placement。
    let Some(placement) = input.placement else {
        // 缺失 placement 表示内部几何分派违反了表面锚定前置条件。
        panic!("表面锚定需要显式 placement");
    };
    // 控件宽度不能超过当前表面。
    let width = natural_width.min(input.surface.w).max(0.0);
    // 控件高度不能超过当前表面。
    let height = input.size.min(input.surface.h).max(0.0);
    // 由 overlay Placement 计算未加作者偏移的横向起点。
    let authored_x = input.surface.x
        // 使用 overlay 模块拥有的横向语义。
        + placement.horizontal_start(input.surface.w, width, input.visual.surface_inset)
        // 叠加兼容作者偏移。
        + input.offset.x;
    // 由 overlay Placement 计算未加作者偏移的纵向起点。
    let authored_y = input.surface.y
        // 使用 overlay 模块拥有的纵向语义。
        + placement.vertical_start(input.surface.h, height, input.visual.surface_inset)
        // 叠加兼容作者偏移。
        + input.offset.y;
    // 计算横向允许的最远起点。
    let max_x = input.surface.x + (input.surface.w - width).max(0.0);
    // 计算纵向允许的最远起点。
    let max_y = input.surface.y + (input.surface.h - height).max(0.0);
    // 把作者偏移后的控件重新收敛到窗口内。
    Rect::new(
        // 收敛横向起点。
        authored_x.clamp(input.surface.x, max_x),
        // 收敛纵向起点。
        authored_y.clamp(input.surface.y, max_y),
        // 保存最终宽度。
        width,
        // 保存最终高度。
        height,
    )
}

// 保留未显式 placement 时的 frame-relative 兼容行为。
fn legacy_control(
    // 接收组件布局矩形。
    frame: Rect,
    // 接收作者偏移。
    offset: Point,
    // 接收自然宽度。
    width: f32,
    // 接收按钮高度。
    height: f32,
    // 返回兼容矩形。
) -> Rect {
    // 沿用旧实现的 frame 起点加偏移语义。
    Rect::new(frame.x + offset.x, frame.y + offset.y, width, height)
}

// 从控件剩余空间派生说明文字区域。
fn description_rect(
    // 接收完整控件区域。
    control: Rect,
    // 接收图标区域。
    icon: Rect,
    // 接收说明文字。
    description: &str,
    // 借用 UIX 几何参数。
    visual: &FloatButtonGeometryVisual,
    // 返回可选说明区域。
) -> Option<Rect> {
    // 空说明不产生绘制区域。
    if description.is_empty() {
        // 返回缺省值。
        return None;
    }
    // 计算图标之后的说明起点。
    let start = icon.x + icon.w + visual.description_gap;
    // 计算尾部留白之前的可用宽度。
    let width = (control.x + control.w - start - visual.description_trailing_padding).max(0.0);
    // 没有剩余宽度时隐藏说明，避免生成负矩形。
    if width <= 0.0 {
        // 返回缺省值。
        return None;
    }
    // 返回与控件同高的说明区域。
    Some(Rect::new(start, control.y, width, control.h))
}

// 从控件右上角派生数字或圆点徽标。
fn badge_rect(
    // 接收最终控件区域。
    control: Rect,
    // 接收非负数字徽标值。
    badge_count: i32,
    // 接收圆点徽标开关。
    badge_dot: bool,
    // 借用 UIX 几何参数。
    visual: &FloatButtonGeometryVisual,
    // 返回可选徽标区域。
) -> Option<Rect> {
    // 圆点语义优先于数字显示。
    let size = if badge_dot {
        // 圆点使用紧凑直径。
        visual.badge_dot_size
    } else if badge_count > 0 {
        // 正数徽标使用数字直径。
        visual.badge_count_size
    } else {
        // 无徽标配置时不产生区域。
        return None;
    };
    // 把徽标保持在控件右上角内部，确保表面约束稳定。
    Some(Rect::new(
        control.x + control.w - size,
        control.y,
        size,
        size,
    ))
}

// 从按钮位置解析表面内提示框。
fn tooltip_rect(
    // 接收最终控件区域。
    control: Rect,
    // 接收当前逻辑表面。
    surface: Rect,
    // 接收可选放置方向。
    placement: Option<Placement>,
    // 接收提示文字。
    tooltip: &str,
    // 标记是否使用当前表面约束。
    constrained: bool,
    // 借用 UIX 几何参数。
    visual: &FloatButtonGeometryVisual,
    // 返回可选提示框。
) -> Option<Rect> {
    // 空提示不产生绘制区域。
    if tooltip.is_empty() {
        // 返回缺省值。
        return None;
    }
    // 按字符数估算提示框自然宽度。
    let natural_width = (tooltip.chars().count() as f32 * visual.average_character_width
        // 加上水平留白。
        + visual.tooltip_horizontal_padding)
        // 保持短提示可读。
        .max(visual.tooltip_min_width);
    // 有效表面内限制提示框宽度。
    let width = if constrained {
        // 收敛到当前表面宽度。
        natural_width.min(surface.w).max(0.0)
    } else {
        // 兼容模式保留旧自然宽度。
        natural_width
    };
    // 有效表面内限制提示框高度。
    let height = if constrained {
        // 收敛到当前表面高度。
        visual.tooltip_height.min(surface.h).max(0.0)
    } else {
        // 兼容模式保留旧固定高度。
        visual.tooltip_height
    };
    // 左侧锚定按钮优先把提示框放到右侧。
    let prefers_right = matches!(
        // 检查显式放置语义。
        placement,
        // 列出所有左侧 placement。
        Some(Placement::TopLeft | Placement::BottomLeft | Placement::Left)
    );
    // 按放置侧向计算候选横坐标。
    let candidate_x = if prefers_right {
        // 左侧按钮的提示框向右展开。
        control.x + control.w + visual.tooltip_gap
    } else {
        // 其他按钮沿用向左展开。
        control.x - width - visual.tooltip_gap
    };
    // 垂直居中提示框。
    let candidate_y = control.y + (control.h - height) * 0.5;
    // 无表面约束时保留旧候选坐标。
    if !constrained {
        // 返回兼容提示框。
        return Some(Rect::new(candidate_x, candidate_y, width, height));
    }
    // 计算横向最大起点。
    let max_x = surface.x + (surface.w - width).max(0.0);
    // 计算纵向最大起点。
    let max_y = surface.y + (surface.h - height).max(0.0);
    // 返回收敛到窗口内的提示框。
    Some(Rect::new(
        // 收敛横坐标。
        candidate_x.clamp(surface.x, max_x),
        // 收敛纵坐标。
        candidate_y.clamp(surface.y, max_y),
        // 保存最终宽度。
        width,
        // 保存最终高度。
        height,
    ))
}

// 计算控件阴影覆盖区域。
fn shadow_bounds(
    // 接收最终控件区域。
    control: Rect,
    // 借用 UIX 几何参数。
    visual: &FloatButtonGeometryVisual,
    // 返回保守阴影矩形。
) -> Rect {
    // 与既有 draw_box_shadow 参数保持一致的保守外扩。
    Rect::new(
        // 向左外扩十个逻辑像素。
        control.x - visual.shadow_left_outset,
        // 向上外扩十个逻辑像素。
        control.y - visual.shadow_top_outset,
        // 横向总计外扩二十个逻辑像素。
        control.w + visual.shadow_width_extra,
        // 下方阴影更长，因此纵向总计外扩二十四个逻辑像素。
        control.h + visual.shadow_height_extra,
    )
}

// 判断矩形能否作为窗口逻辑表面。
fn valid_surface(
    // 接收候选表面。
    surface: Rect,
    // 返回有效性。
) -> bool {
    // 要求全部坐标与尺寸有限。
    surface.x.is_finite()
        // 要求纵坐标有限。
        && surface.y.is_finite()
        // 要求宽度有限且为正。
        && surface.w.is_finite()
        // 要求高度有限且为正。
        && surface.h.is_finite()
        // 空宽度不能提供锚点。
        && surface.w > 0.0
        // 空高度不能提供锚点。
        && surface.h > 0.0
}
