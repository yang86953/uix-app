// 引入当前组件的私有行为与公开构建器。
use super::*;
// 引入 View 构建入口和静态指针比较。
use crate::ui::View;
use std::ptr;

// 验证默认实例和 UIX 构建节点只使用同一份视觉事实。
#[test]
fn input_number_uses_colocated_uix_visual() {
    // 构建前不得复制第二份默认视觉表。
    let input = InputNumber::default();
    assert!(ptr::eq(input.visual, INPUT_NUMBER_VISUAL_REF));
    // UIX 注入后的叶内核仍指向同一静态记录。
    let node = View::build(input);
    let input = node
        .widget
        .as_any()
        .downcast_ref::<InputNumber>()
        .expect("UIX 根应保留 InputNumber 内核");
    assert!(ptr::eq(input.visual, INPUT_NUMBER_VISUAL_REF));
}

// 验证显式精度同时控制值量化、步进与显示文本。
#[test]
fn precision_quantizes_bound_value_step_and_display() {
    // 构造带多余小数位的受控状态。
    let state = State::new(1.239_f64);
    // 先声明精度再绑定，验证构建顺序的主路径。
    let mut input = InputNumber::new()
        // 保留两位小数。
        .precision(2)
        // 设置百分位步长。
        .step(0.01)
        // 绑定外部状态。
        .value(&state);
    // 运行时缓存必须量化为两位小数。
    assert!((input.current_value() - 1.24).abs() < f64::EPSILON);
    // 向上步进一次必须得到稳定的百分位结果。
    input.step_by(1.0);
    // 内部值应更新为一点二五。
    assert!((input.current_value() - 1.25).abs() < f64::EPSILON);
    // 双向绑定必须收到同一量化值。
    assert!((state.get() - 1.25).abs() < f64::EPSILON);
    // 读取公开组件快照。
    let SnapshotFields::InputNumber {
        // 提取精度配置。
        precision,
        // 提取最终显示文本。
        display_value,
        // 忽略本测试不关心的其他字段。
        ..
    } = input.snapshot_fields()
    else {
        // 组件快照类型不匹配表示实现错误。
        panic!("InputNumber 必须生成数值输入快照");
    };
    // 快照必须保留精度配置和固定小数位文本。
    assert_eq!(precision, Some(2));
    // 显示文本必须稳定保留两位小数。
    assert_eq!(display_value.as_deref(), Some("1.25"));
    // 另建状态验证 value 在 precision 之前声明也保持一致。
    let reverse_state = State::new(1.239_f64);
    // 先绑定再声明精度，覆盖公开构建器的反向顺序。
    let reverse = InputNumber::new().value(&reverse_state).precision(2);
    // 反向构建顺序同样必须量化为两位小数。
    assert!((reverse.current_value() - 1.24).abs() < f64::EPSILON);
}

// 验证过高精度统一收敛到 f64 可兑现上限。
#[test]
fn precision_is_bounded_by_f64_contract() {
    // 直接 API 的动态精度可能越界，运行时必须归一化。
    let input = InputNumber::new().precision(u8::MAX).default_value(1.2_f64);
    // 读取声明式配置快照。
    let SnapshotFields::InputNumber {
        // 提取归一化后的精度。
        precision,
        // 提取固定小数位显示文本。
        display_value,
        // 忽略其他配置字段。
        ..
    } = input.snapshot_fields()
    else {
        // 组件快照类型不匹配表示实现错误。
        panic!("InputNumber 必须生成数值输入快照");
    };
    // 精度必须收敛到十五位。
    assert_eq!(precision, Some(MAX_PRECISION));
    // 显示文本必须兑现归一化后的固定精度。
    assert_eq!(display_value.as_deref(), Some("1.200000000000000"));
}
