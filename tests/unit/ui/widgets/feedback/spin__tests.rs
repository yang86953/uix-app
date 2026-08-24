// 复用被测加载指示器。
use super::{SPIN_VISUAL, SPIN_VISUAL_REF, Spin};
// 引入动画更新公开组件契约。
use crate::ui::widget_runtime::traits::WidgetAnimation;
// 引入公开声明构建契约以验证同目录 UIX 根。
use crate::ui::view::View;
// 引入延迟配置类型。
use std::time::Duration;

// 验证旋转递推与原逐点三角函数几何在亚像素误差内等价。
#[test]
fn dot_rotation_recurrence_preserves_geometry() {
    // 选择非轴对齐相位与常用轨道半径，覆盖实际动画路径。
    let phase = 0.37_f32;
    let radius = 12.6_f32;
    let mut offsets = Vec::with_capacity(8);
    // 收集递推结果仅用于测试；生产绘制仍以回调流式提交，无逐帧分配。
    let dots = SPIN_VISUAL.dots;
    Spin::for_each_dot_offset(
        phase,
        radius,
        dots.count,
        dots.step_sin,
        dots.step_cos,
        |index, dx, dy| {
            offsets.push((index, dx, dy));
        },
    );
    // 必须继续生成原有八个等间距圆点。
    assert_eq!(offsets.len(), 8);
    for (index, dx, dy) in offsets {
        // 以原始逐点公式作为质量基准。
        let angle = f32::from(index) * std::f32::consts::TAU / 8.0 + phase;
        let expected_dx = angle.cos() * radius;
        let expected_dy = angle.sin() * radius;
        // 递推舍入误差必须远低于一个逻辑像素。
        assert!((dx - expected_dx).abs() <= 0.000_01, "index={index}");
        assert!((dy - expected_dy).abs() <= 0.000_01, "index={index}");
    }
}

// 验证 UIX 根桥接保持 Spin 动态类型与原有子树形状。
#[test]
fn uix_root_preserves_spin_kernel_and_children() {
    // 构造一个拥有型子节点，模拟包裹模式的任意声明子树。
    let child = View::build(Spin::new().spinning(false));
    // 经公开代码生成桥接进入组件自己的 UIX 声明。
    let node = Spin::new()
        .tip("正在加载")
        .wrapper_mode()
        .build_view_with_children(vec![child]);
    // UIX 声明不得增加额外包装节点。
    assert_eq!(node.children.len(), 1);
    // 根动态类型必须继续是承载动画与绘制机制的 Spin。
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Spin>()
        .expect("UIX 根必须保留 Spin 内核");
    // 声明配置与包裹模式必须无损进入原内核。
    assert_eq!(kernel.tip, "正在加载");
    assert!(kernel.wrapper_mode);
    assert_eq!(kernel.visual.layout.default_diameter, 24.0);
    assert_eq!(kernel.visual.dots.count, 8);
    assert_eq!(kernel.visual.motion.turns_per_second, 1.0);
    assert!(!kernel.size_authored);
}

// 验证显式尺寸覆盖 UIX 默认值，完整视觉表由实例共享。
#[test]
fn uix_visual_configuration_is_shared_and_keeps_authored_size() {
    // 构建前默认值直接读取 UIX 生成的唯一静态视觉事实。
    assert!(std::ptr::eq(Spin::new().visual, SPIN_VISUAL_REF));
    let first = View::build(Spin::new().large());
    let second = View::build(Spin::new());
    let first = first
        .widget
        .as_any()
        .downcast_ref::<Spin>()
        .expect("第一个 UIX 根必须保留 Spin 内核");
    let second = second
        .widget
        .as_any()
        .downcast_ref::<Spin>()
        .expect("第二个 UIX 根必须保留 Spin 内核");
    assert_eq!(first.diameter(), 36.0);
    assert!(first.size_authored);
    assert_eq!(second.diameter(), 24.0);
    assert!(std::ptr::eq(first.visual, second.visual));
    assert!(std::ptr::eq(first.visual, SPIN_VISUAL_REF));
}

// 验证配置刷新在延迟未变化时保留运行时动画进度。
#[test]
// 测试名称说明运行时状态所有权边界。
fn refresh_preserves_runtime_animation_progress() {
    // 创建带稳定延迟的当前运行时组件。
    let mut current = Spin::new().delay(Duration::from_secs(1));
    // 推进延迟计时直至指示器进入可见阶段。
    WidgetAnimation::update_animation(&mut current, 1.0);
    // 继续推进动画以建立非零运行时相位。
    WidgetAnimation::update_animation(&mut current, 0.25);
    // 记录刷新前的动画相位。
    let phase = current.phase();
    // 记录刷新前的延迟进度。
    let delay_elapsed = current.delay_elapsed;
    // 前置步骤必须确实建立非零动画相位。
    assert!(phase > 0.0);
    // 前置步骤必须确实完成延迟计时。
    assert!(delay_elapsed > 0.0);
    // 创建下一帧声明，只更新提示文字并保持延迟配置不变。
    let next = Spin::new()
        // 保持同一延迟配置。
        .delay(Duration::from_secs(1))
        // 更新声明提示文字。
        .tip("仍在加载");
    // 按组件树协调协议刷新声明字段。
    current.sync_from(next);
    // 声明刷新后动画相位必须继续保留。
    assert_eq!(current.phase(), phase);
    // 未改变延迟时计时进度必须继续保留。
    assert_eq!(current.delay_elapsed, delay_elapsed);
    // 声明字段仍必须更新为新的提示文字。
    assert_eq!(current.tip, "仍在加载");
}
