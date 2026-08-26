// 复用被测提示气泡原语与共享类型。
use super::*;

// 标记靠近表面边缘时提示气泡必须保持可见。
#[test]
// 顶部空间不足时不能继续把气泡放到负坐标。
fn top_bubble_near_surface_edge_stays_inside() {
    // 构造一个有限逻辑表面。
    let surface = Rect::new(0.0, 0.0, 200.0, 100.0);
    // 将目标放在表面上边缘附近。
    let frame = Rect::new(90.0, 4.0, 20.0, 20.0);
    // 按当前顶部配置和有限表面解析提示气泡。
    let geometry = resolve_tooltip_geometry("tip", true, TooltipPlacement::Top, frame, surface);
    // 读取最终气泡矩形。
    let bubble = geometry.bubble;

    // 顶部空间不足时必须翻转到目标下方。
    assert_eq!(geometry.placement, TooltipPlacement::Bottom);

    // 最终气泡必须完整位于当前表面内。
    assert!(
        // 检查左边界没有越出表面。
        bubble.x >= surface.x
                // 检查上边界没有越出表面。
                && bubble.y >= surface.y
                // 检查右边界没有越出表面。
                && bubble.x + bubble.w <= surface.x + surface.w
                // 检查下边界没有越出表面。
                && bubble.y + bubble.h <= surface.y + surface.h
    );
}

// 标记超长提示文字必须服从有限表面宽度。
#[test]
// 窄表面不能生成比表面更宽的气泡矩形。
fn long_bubble_width_is_constrained_by_surface() {
    // 构造比长提示文字更窄的逻辑表面。
    let surface = Rect::new(0.0, 0.0, 80.0, 100.0);
    // 将目标放在表面中央。
    let frame = Rect::new(30.0, 40.0, 20.0, 20.0);
    // 使用足以超过窄表面的提示文字。
    let bubble = tooltip_bubble_rect(
        // 传入长提示内容。
        "a tooltip value that is wider than the surface",
        // 保留箭头间距。
        true,
        // 使用顶部位置。
        TooltipPlacement::Top,
        // 传入目标矩形。
        frame,
        // 传入当前有限表面。
        surface,
    );

    // 最终气泡宽度不得超过当前表面。
    assert!(bubble.w <= surface.w);
}
