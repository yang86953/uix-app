// 引入当前模块公开组件。
use super::{SPLITTER_VISUAL_REF, Splitter};
// 引入 View 构建入口和指针比较。
use crate::ui::View;
use std::ptr;

// 验证构建前默认状态和构建后节点都复用 UIX 生成的唯一视觉。
#[test]
fn splitter_uses_colocated_uix_visual() {
    // 构造器不得复制或重建另一份视觉默认表。
    let splitter = Splitter::new();
    assert!(ptr::eq(splitter.visual, SPLITTER_VISUAL_REF));
    // UIX 根注入后仍应保留同一静态视觉引用。
    let node = View::build(splitter);
    let splitter = node
        .widget
        .as_any()
        .downcast_ref::<Splitter>()
        .expect("UIX 根应保留 Splitter 内核");
    assert!(ptr::eq(splitter.visual, SPLITTER_VISUAL_REF));
}

// 验证合法、越界与非有限初始比例的归一化结果。
#[test]
fn default_ratio_builds_normalized_two_panel_contract() {
    // 构造常规三七比例。
    let split = Splitter::new().default_ratio(0.3);
    // 两个面板必须保留互补比例。
    assert!((split.ratios()[0] - 0.3).abs() < f32::EPSILON);
    // 互补面板允许浮点减法产生机器精度误差。
    assert!((split.ratios()[1] - 0.7).abs() < f32::EPSILON);
    // 越界动态值必须夹紧而不是破坏总和。
    assert_eq!(
        // 读取夹紧后的双面板比例。
        Splitter::new().default_ratio(2.0).ratios(),
        // 上界对应第一个面板占满。
        &[1.0, 0.0]
    );
    // 非有限输入必须恢复文档默认值。
    assert_eq!(
        // 读取非有限输入的回退比例。
        Splitter::new().default_ratio(f32::NAN).ratios(),
        // 默认契约为均分。
        &[0.5, 0.5]
    );
}
