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

// 验证单行 Stretch 在交叉轴最终尺寸阶段仍遵守子项上下限。
#[test]
// 同时覆盖父容器过小时的最小值和父容器宽裕时的最大值。
fn single_line_stretch_respects_cross_axis_bounds() {
    // 构造自然高五但最小高二十的单个子项。
    let minimum_children = [FlexChild {
        // 交叉轴最小高度固定为二十。
        min_size: Size::new(0.0, 20.0),
        // 自然尺寸低于交叉轴最小值。
        measured_size: Size::new(20.0, 5.0),
        // 其余属性沿用稳定默认值。
        ..FlexChild::default()
    }];
    // 在只有十像素高的容器内执行默认 Stretch。
    let minimum_output = compute_flex_layout(&FlexInput {
        // 使用水平主轴，使高度成为受测交叉轴。
        direction: FlexDirection::Row,
        // 父容器交叉轴小于子项最小高度。
        container: Rect::new(0.0, 0.0, 100.0, 10.0),
        // 传入最小值受测子项。
        children: &minimum_children,
        // 其余输入沿用默认 Stretch。
        ..FlexInput::default()
    });
    // Stretch 不得把子项压到二十像素最小高度以下。
    assert_eq!(minimum_output.child_rects[0].h, 20.0);

    // 构造自然高十但最大高三十的单个子项。
    let maximum_children = [FlexChild {
        // 交叉轴最大高度固定为三十。
        max_size: Size::new(f32::MAX, 30.0),
        // 自然尺寸低于交叉轴最大值。
        measured_size: Size::new(20.0, 10.0),
        // 其余属性沿用稳定默认值。
        ..FlexChild::default()
    }];
    // 在五十像素高的容器内执行默认 Stretch。
    let maximum_output = compute_flex_layout(&FlexInput {
        // 使用水平主轴，使高度成为受测交叉轴。
        direction: FlexDirection::Row,
        // 父容器交叉轴高于子项最大高度。
        container: Rect::new(0.0, 0.0, 100.0, 50.0),
        // 传入最大值受测子项。
        children: &maximum_children,
        // 其余输入沿用默认 Stretch。
        ..FlexInput::default()
    });
    // Stretch 不得把子项拉到三十像素最大高度以上。
    assert_eq!(maximum_output.child_rects[0].h, 30.0);
}

// 验证 wrapped Stretch 扩大行盒时仍遵守每个子项的交叉轴最大值。
#[test]
// 行盒可以消费剩余空间，但受限子项不能随行盒一起突破上限。
fn wrapped_stretch_respects_cross_axis_maximum() {
    // 构造两个宽六十且最大高度十二的子项。
    let children = [
        // 首项单独占据第一行。
        FlexChild {
            // 首项交叉轴最大高度固定为十二。
            max_size: Size::new(f32::MAX, 12.0),
            // 首项自然尺寸为六十乘十。
            measured_size: Size::new(60.0, 10.0),
            // 其余属性沿用稳定默认值。
            ..FlexChild::default()
        },
        // 次项单独占据第二行。
        FlexChild {
            // 次项交叉轴最大高度同样固定为十二。
            max_size: Size::new(f32::MAX, 12.0),
            // 次项自然尺寸同样为六十乘十。
            measured_size: Size::new(60.0, 10.0),
            // 其余属性沿用稳定默认值。
            ..FlexChild::default()
        },
    ];
    // 在四十五像素交叉轴内执行两行默认 Stretch。
    let output = compute_flex_layout(&FlexInput {
        // 使用水平主轴。
        direction: FlexDirection::Row,
        // 开启换行，使两个六十像素子项分到两行。
        wrap: true,
        // 五像素间距同时作为行间固定 gap。
        gap: 5.0,
        // 二十像素自然行组之外的空间会扩展两个行盒。
        container: Rect::new(0.0, 0.0, 100.0, 45.0),
        // 传入两个受测子项。
        children: &children,
        // 其余输入沿用默认 Stretch。
        ..FlexInput::default()
    });
    // 首项停在十二像素最大高度，行盒自身仍可更高。
    assert_eq!(output.child_rects[0], Rect::new(0.0, 0.0, 60.0, 12.0));
    // 次项从扩展后的第二行起点开始并同样停在最大高度。
    assert_eq!(output.child_rects[1], Rect::new(0.0, 25.0, 60.0, 12.0));
}
