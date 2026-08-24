// 复用被测模块的评分组件与 UIX 视觉引用。
use super::*;
// 引入 View 构建入口和静态指针比较。
use crate::ui::View;
use std::ptr;

// 验证默认实例和 UIX 构建节点只使用同一份视觉事实。
#[test]
fn rate_uses_colocated_uix_visual() {
    // 构建前不得复制第二份默认视觉表。
    let rate = Rate::default();
    assert!(ptr::eq(rate.visual, RATE_VISUAL_REF));
    // UIX 注入后的叶内核仍指向同一静态记录。
    let node = View::build(rate);
    let rate = node
        .widget
        .as_any()
        .downcast_ref::<Rate>()
        .expect("UIX 根应保留 Rate 内核");
    assert!(ptr::eq(rate.visual, RATE_VISUAL_REF));
    // 默认可见星数也必须来自同一 UIX 记录。
    assert_eq!(rate.count, RATE_VISUAL_REF.default_count);
}
