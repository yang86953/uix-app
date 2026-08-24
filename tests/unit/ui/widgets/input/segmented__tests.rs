// 复用被测模块的分段选择器与 UIX 视觉引用。
use super::*;
// 引入 View 构建入口和静态指针比较。
use crate::ui::View;
use std::ptr;

// 验证默认实例和 UIX 构建节点只使用同一份视觉事实。
#[test]
fn segmented_uses_colocated_uix_visual() {
    // 构建前不得复制第二份默认视觉表。
    let segmented = Segmented::default();
    assert!(ptr::eq(segmented.visual, SEGMENTED_VISUAL_REF));
    // UIX 注入后的叶内核仍指向同一静态记录。
    let node = View::build(segmented);
    let segmented = node
        .widget
        .as_any()
        .downcast_ref::<Segmented>()
        .expect("UIX 根应保留 Segmented 内核");
    assert!(ptr::eq(segmented.visual, SEGMENTED_VISUAL_REF));
}

// 验证无临时 Vec 的缩放宽度仍精确消费整个 frame。
#[test]
fn scaled_segment_widths_preserve_frame_total() {
    let segmented = Segmented::new(["短", "longer", "中"]);
    let height = segmented.control_height();
    let frame_width = 257.0;
    let total = segmented.nominal_width_total(height);
    let ratio = frame_width / total;
    let mut used = 0.0;
    let last = segmented.options.len() - 1;
    for index in 0..segmented.options.len() {
        let width = segmented.scaled_segment_width(
            index,
            last,
            height,
            frame_width,
            total,
            ratio,
            0.0,
            used,
        );
        used += width;
    }
    assert!((used - frame_width).abs() < f32::EPSILON * 16.0);
}
