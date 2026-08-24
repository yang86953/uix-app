// 复用被测模块的范围滑块与 UIX 视觉引用。
use super::*;
// 引入 View 构建入口和静态指针比较。
use crate::ui::View;
use std::ptr;

// 验证默认实例和 UIX 构建节点只使用同一份视觉事实。
#[test]
fn range_slider_uses_colocated_uix_visual() {
    // 构建前不得复制第二份默认视觉表。
    let slider = RangeSlider::default();
    assert!(ptr::eq(slider.visual, RANGE_SLIDER_VISUAL_REF));
    // UIX 注入后的叶内核仍指向同一静态记录。
    let node = View::build(slider);
    let slider = node
        .widget
        .as_any()
        .downcast_ref::<RangeSlider>()
        .expect("UIX 根应保留 RangeSlider 内核");
    assert!(ptr::eq(slider.visual, RANGE_SLIDER_VISUAL_REF));
}
