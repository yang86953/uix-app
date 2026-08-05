// 复用父模块的纯 Flex 求解类型与辅助函数。
use super::*;

// 验证 grow 子项触顶后把剩余空间完整交给仍可增长的兄弟。
#[test]
// 覆盖超过三轮钳制时旧实现遗留空白的边界。
fn grow_redistributes_after_max_clamp() {
    // 构造一个最大宽度为二十的 grow 子项和一个无上限兄弟。
    let children = [
        // 首项只能从十增长到二十。
        FlexChild {
            // 两个子项使用相同 grow 权重。
            flex_grow: 1.0,
            // 首项主轴最大值固定为二十。
            max_size: Size::new(20.0, f32::MAX),
            // 首项自然尺寸为十乘十。
            measured_size: Size::new(10.0, 10.0),
            // 其余属性沿用稳定默认值。
            ..FlexChild::default()
        },
        // 次项保持无上限并接收剩余空间。
        FlexChild {
            // 次项使用相同 grow 权重。
            flex_grow: 1.0,
            // 次项自然尺寸同样为十乘十。
            measured_size: Size::new(10.0, 10.0),
            // 其余属性沿用稳定默认值。
            ..FlexChild::default()
        },
    ];
    // 在一百像素主轴中执行单行水平布局。
    let output = compute_flex_layout(&FlexInput {
        // 使用水平主轴以便直接核对宽度。
        direction: FlexDirection::Row,
        // 容器提供一百像素可分配宽度。
        container: Rect::new(0.0, 0.0, 100.0, 20.0),
        // 传入两个受测子项。
        children: &children,
        // 交叉轴保持自然尺寸，避免 Stretch 干扰断言。
        align_items: AlignItems::Start,
        // 其余输入沿用稳定默认值。
        ..FlexInput::default()
    });
    // 首项必须停在二十像素最大宽度。
    assert_eq!(output.child_rects[0].w, 20.0);
    // 次项必须接收全部剩余八十像素而不留下尾部空白。
    assert_eq!(output.child_rects[1].w, 80.0);
}

// 验证 shrink 子项触底后把剩余溢出完整交给仍可收缩的兄弟。
#[test]
// 覆盖 min_size 钳制后旧实现仍让整行越界的边界。
fn shrink_redistributes_after_min_clamp() {
    // 构造一个最小宽度七十的子项和一个可继续收缩的兄弟。
    let children = [
        // 首项只能从八十收缩到七十。
        FlexChild {
            // 首项主轴最小值固定为七十。
            min_size: Size::new(70.0, 0.0),
            // 首项自然尺寸为八十乘十。
            measured_size: Size::new(80.0, 10.0),
            // 其余属性沿用默认 shrink 权重。
            ..FlexChild::default()
        },
        // 次项允许继续收缩以吸收剩余溢出。
        FlexChild {
            // 次项自然尺寸同样为八十乘十。
            measured_size: Size::new(80.0, 10.0),
            // 其余属性沿用默认 shrink 权重与零最小值。
            ..FlexChild::default()
        },
    ];
    // 在一百像素主轴中执行单行水平布局。
    let output = compute_flex_layout(&FlexInput {
        // 使用水平主轴以便直接核对宽度。
        direction: FlexDirection::Row,
        // 容器宽度比两项自然宽度之和少六十。
        container: Rect::new(0.0, 0.0, 100.0, 20.0),
        // 传入两个受测子项。
        children: &children,
        // 交叉轴保持自然尺寸，避免 Stretch 干扰断言。
        align_items: AlignItems::Start,
        // 其余输入沿用稳定默认值。
        ..FlexInput::default()
    });
    // 首项必须停在七十像素最小宽度。
    assert_eq!(output.child_rects[0].w, 70.0);
    // 次项继续缩到三十，使整行精确收敛到容器宽度。
    assert_eq!(output.child_rects[1].w, 30.0);
    // 第二项终点不得越出一百像素容器。
    assert_eq!(output.child_rects[1].x + output.child_rects[1].w, 100.0);
}

// 验证主轴最小值在分行前生效，避免钳制后才把同一行撑出容器。
#[test]
// 覆盖 wrapped 主轴 min_size 改变分行决策的边界。
fn wrapped_main_minimum_reflows_before_positioning() {
    // 构造两个自然宽四十但最小宽六十的子项。
    let children = [
        // 首项的主轴最小值超过自然宽度。
        FlexChild {
            // 首项最小宽度为六十。
            min_size: Size::new(60.0, 0.0),
            // 首项自然尺寸为四十乘十。
            measured_size: Size::new(40.0, 10.0),
            // 其余属性沿用稳定默认值。
            ..FlexChild::default()
        },
        // 次项使用相同约束以触发换行。
        FlexChild {
            // 次项最小宽度同样为六十。
            min_size: Size::new(60.0, 0.0),
            // 次项自然尺寸同样为四十乘十。
            measured_size: Size::new(40.0, 10.0),
            // 其余属性沿用稳定默认值。
            ..FlexChild::default()
        },
    ];
    // 在一百像素主轴中执行可换行布局。
    let output = compute_flex_layout(&FlexInput {
        // 使用水平主轴。
        direction: FlexDirection::Row,
        // 开启换行路径。
        wrap: true,
        // 容器无法在同一行容纳两个六十像素子项。
        container: Rect::new(0.0, 0.0, 100.0, 30.0),
        // 传入两个受测子项。
        children: &children,
        // 保持自然交叉轴尺寸。
        align_items: AlignItems::Start,
        // 其余输入沿用稳定默认值。
        ..FlexInput::default()
    });
    // 首项保持在第一行并落实六十像素最小宽度。
    assert_eq!(output.child_rects[0], Rect::new(0.0, 0.0, 60.0, 10.0));
    // 次项必须换到第二行，而不是从第一行 x=60 继续越界。
    assert_eq!(output.child_rects[1], Rect::new(0.0, 10.0, 60.0, 10.0));
}

// 验证交叉轴最小值在行高计算前生效，避免相邻 wrapped 行重叠。
#[test]
// 覆盖 wrapped 交叉轴 min_size 与行推进共用同一尺寸账本。
fn wrapped_cross_minimum_advances_next_line() {
    // 构造两个自然高五但最小高二十的宽子项。
    let children = [
        // 首项宽度迫使每行只能容纳一个子项。
        FlexChild {
            // 首项最小高度为二十。
            min_size: Size::new(0.0, 20.0),
            // 首项自然尺寸为六十乘五。
            measured_size: Size::new(60.0, 5.0),
            // 其余属性沿用稳定默认值。
            ..FlexChild::default()
        },
        // 次项使用相同约束进入下一行。
        FlexChild {
            // 次项最小高度同样为二十。
            min_size: Size::new(0.0, 20.0),
            // 次项自然尺寸同样为六十乘五。
            measured_size: Size::new(60.0, 5.0),
            // 其余属性沿用稳定默认值。
            ..FlexChild::default()
        },
    ];
    // 在一百像素主轴和五像素 gap 中执行可换行布局。
    let output = compute_flex_layout(&FlexInput {
        // 使用水平主轴。
        direction: FlexDirection::Row,
        // 开启换行路径。
        wrap: true,
        // 主轴一次只能容纳一个六十像素子项。
        container: Rect::new(0.0, 0.0, 100.0, 50.0),
        // 主轴与交叉轴行间距均为五。
        gap: 5.0,
        // 传入两个受测子项。
        children: &children,
        // 保持落实后的交叉轴最小尺寸。
        align_items: AlignItems::Start,
        // 其余输入沿用稳定默认值。
        ..FlexInput::default()
    });
    // 首项落实二十像素最小高度。
    assert_eq!(output.child_rects[0], Rect::new(0.0, 0.0, 60.0, 20.0));
    // 次项从首项二十像素行高加五像素 gap 后开始。
    assert_eq!(output.child_rects[1], Rect::new(0.0, 25.0, 60.0, 20.0));
}
