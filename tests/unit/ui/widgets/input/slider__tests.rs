// 复用被测滑块与提示位置枚举。
use super::{SLIDER_VISUAL_REF, Slider, TooltipPlacement};
// 引入几何基础类型。
use crate::core::Rect;
// 引入共享解析器以核对登记与绘制几何同源。
use crate::ui::widgets::tooltip_primitives::resolve_tooltip_geometry;
// 引入 View 构建入口和静态指针比较。
use crate::ui::View;
use std::ptr;

// 验证默认实例和 UIX 构建节点只使用同一份视觉事实。
#[test]
fn slider_uses_colocated_uix_visual() {
    // 构建前不得复制第二份默认视觉表。
    let slider = Slider::default();
    assert!(ptr::eq(slider.visual, SLIDER_VISUAL_REF));
    // UIX 注入后的叶内核仍指向同一静态记录。
    let node = View::build(slider);
    let slider = node
        .widget
        .as_any()
        .downcast_ref::<Slider>()
        .expect("UIX 根应保留 Slider 内核");
    assert!(ptr::eq(slider.visual, SLIDER_VISUAL_REF));
}

// 靠近表面上边缘拖动时，提示登记必须翻转并保持在表面内。
#[test]
// 测试名称说明显式表面入口的职责。
fn overlay_entry_uses_current_surface_geometry() {
    // 创建带顶部拖动提示的滑块。
    let mut slider = Slider::new(0.0..=100.0)
        // 将滑块值置于轨道中点。
        .default_value(50.0)
        // 配置作者期望的顶部方向。
        .tooltip(TooltipPlacement::Top);
    // 模拟正在拖动，使提示参与浮层登记。
    slider.dragging = true;
    // 将滑块放在表面上边缘。
    let frame = Rect::new(20.0, 0.0, 160.0, 28.0);
    // 使用足以容纳翻转后气泡的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 200.0, 100.0);
    // 读取滑块当前值对应的提示目标。
    let (placement, target) = slider
        // 使用与登记相同的滑块 frame。
        .tooltip_target(frame)
        // 拖动状态下必须存在提示目标。
        .expect("拖动中的滑块应生成提示目标");
    // 通过组件树使用的显式表面入口创建登记。
    let overlay = crate::ui::widget_runtime::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测滑块。
        &slider,
        // 使用稳定的测试组件标识。
        crate::core::WidgetId::new(4),
        // 传入靠近上边缘的滑块 frame。
        frame,
        // 传入当前帧的逻辑表面。
        surface,
    )
    // 拖动状态必须生成提示登记。
    .expect("拖动中的滑块应生成浮层登记");
    // 使用同一共享解析器计算预期几何。
    let expected = resolve_tooltip_geometry(
        // 使用滑块当前值的显示文字。
        &slider.current_value().to_string(),
        // 滑块拖动提示始终带箭头。
        slider.visual.tooltip_arrow,
        // 使用提示目标返回的作者方向。
        placement,
        // 使用滑块拇指目标矩形。
        target,
        // 使用当前逻辑表面。
        surface,
    );

    // 上方空间不足时应翻转到底部。
    assert_eq!(expected.placement, TooltipPlacement::Bottom);
    // 浮层登记必须与共享解析器的受限气泡完全一致。
    assert_eq!(overlay.bounds_rect(), Some(expected.bubble));
}
