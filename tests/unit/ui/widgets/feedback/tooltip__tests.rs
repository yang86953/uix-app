// 复用被测组件与位置枚举。
use super::{TOOLTIP_VISUAL, TOOLTIP_VISUAL_REF, Tooltip, TooltipPlacement};
// 引入几何基础类型。
use crate::core::Rect;
// 引入共享解析器以核对登记与绘制几何同源。
use crate::ui::widgets::tooltip_primitives::{
    resolve_tooltip_geometry, tooltip_bubble_rect, tooltip_bubble_rect_with_visual_and_size,
    tooltip_bubble_size_with_visual,
};

// 验证 View 构建经过同目录 UIX，并保留调用方显式视觉覆写。
#[test]
fn view_build_uses_uix_visual_and_preserves_authored_values() {
    let tooltip = Tooltip::new("提示")
        .placement(TooltipPlacement::Bottom)
        .arrow(false);
    assert!(std::ptr::eq(tooltip.visual, TOOLTIP_VISUAL_REF));
    let node = crate::ui::view::View::build(tooltip);
    let tooltip = node
        .widget
        .as_any()
        .downcast_ref::<Tooltip>()
        .expect("UIX 根必须保留 Tooltip Rust 内核");
    assert!(std::ptr::eq(tooltip.visual, TOOLTIP_VISUAL_REF));
    assert_eq!(tooltip.placement, TooltipPlacement::Bottom);
    assert!(!tooltip.arrow);
    assert_eq!(tooltip.visual.motion.enter_duration, 0.15);
    assert_eq!(tooltip.visual.motion.exit_duration, 0.1);
}

// 验证 UIX 默认气泡参数与迁移前共享原语保持完全一致。
#[test]
fn uix_bubble_visual_preserves_shared_geometry() {
    let frame = Rect::new(80.0, 40.0, 40.0, 20.0);
    let surface = Rect::new(0.0, 0.0, 200.0, 120.0);
    let legacy = tooltip_bubble_rect("tip", true, TooltipPlacement::Top, frame, surface);
    let declared = tooltip_bubble_rect_with_visual_and_size(
        true,
        TooltipPlacement::Top,
        frame,
        surface,
        tooltip_bubble_size_with_visual("tip", TOOLTIP_VISUAL.bubble),
        TOOLTIP_VISUAL.bubble,
    );
    assert_eq!(declared, legacy);
}

// 靠近表面上边缘时，登记必须使用翻转后的受限气泡。
#[test]
// 测试名称说明显式表面入口的职责。
fn overlay_entry_uses_current_surface_geometry() {
    // 创建作者指定顶部方向的提示组件。
    let mut tooltip = Tooltip::new("tip").placement(TooltipPlacement::Top);
    // 打开提示，使其参与浮层登记。
    tooltip.open();
    // 将触发器放在表面上边缘附近。
    let frame = Rect::new(80.0, 0.0, 40.0, 20.0);
    // 使用足以容纳翻转后气泡的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 200.0, 100.0);
    // 通过组件树使用的显式表面入口创建登记。
    let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测提示组件。
        &tooltip,
        // 使用稳定的测试组件标识。
        crate::core::WidgetId::new(3),
        // 传入靠近上边缘的触发器。
        frame,
        // 传入当前帧的逻辑表面。
        surface,
    )
    // 打开状态必须生成提示登记。
    .expect("打开的提示应生成浮层登记");
    // 使用同一共享解析器计算预期几何。
    let expected = resolve_tooltip_geometry(
        // 保持提示文字一致。
        "tip",
        // 默认提示带箭头。
        true,
        // 保持作者指定的顶部方向。
        TooltipPlacement::Top,
        // 保持触发器不变。
        frame,
        // 使用当前逻辑表面。
        surface,
    );

    // 上方空间不足时应翻转到底部。
    assert_eq!(expected.placement, TooltipPlacement::Bottom);
    // 浮层登记必须与共享解析器的受限气泡完全一致。
    assert_eq!(overlay.bounds_rect(), Some(expected.bubble));
}

// 验证声明刷新只替换配置，不抹除运行时已经建立的可见状态。
#[test]
// 测试名称说明 Tooltip 生命周期状态的所有权边界。
fn refresh_preserves_runtime_visibility() {
    // 创建旧声明对应的运行时组件。
    let mut current = Tooltip::new("旧提示");
    // 通过运行时入口建立可见状态与进入过渡。
    current.open();
    // 创建下一帧声明提供的新文字配置。
    let next = Tooltip::new("新提示");
    // 按组件树协调协议刷新声明字段。
    current.sync_from(next);
    // 声明刷新后运行时可见状态必须继续保留。
    assert!(current.is_visible());
    // 声明刷新应替换新的提示文字。
    assert_eq!(current.text, "新提示");
    // 声明刷新必须同步新的自然尺寸，后续帧无需重新度量文字。
    assert_eq!(
        current.natural_bubble_size.get(),
        tooltip_bubble_size_with_visual(&current.text, current.visual.bubble)
    );
}
