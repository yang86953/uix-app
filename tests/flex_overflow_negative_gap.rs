// 导入负行间距契约使用的核心几何类型。
use uix::core::{ComponentId, EdgeInsets, Rect, Size};
// 导入负行间距契约使用的共享布局入口。
use uix::ui::{AlignItems, FlexDirection, FlexLayout, JustifyContent, LayoutChild, LayoutEngine};

// 验证负行间距重叠时自然交叉轴仍覆盖所有可见行。
#[test]
// 较矮末行不得让总尺寸退回到较高首行的物理末端之前。
fn wrapped_overflow_negative_gap_keeps_the_farthest_line_end() {
    // 构造二十乘三十的高首行子项。
    let first = LayoutChild::new(ComponentId::new(80), Size::new(20.0, 30.0));
    // 构造二十乘五的矮末行子项。
    let second = LayoutChild::new(ComponentId::new(81), Size::new(20.0, 5.0));
    // 二十五像素主轴即使扣除八像素负 gap 仍会让两个子项分成两行。
    let children = [first, second];
    // 构造从交叉轴起点排列的水平布局。
    let mut layout = FlexLayout::row()
        // 负八像素 gap 同时让相邻行发生八像素重叠。
        .with_gap(-8.0)
        // 起点对齐隔离自然内容账本。
        .with_align(AlignItems::Start);
    // 开启实际换行路径。
    layout.wrap = true;
    // 保留两项自然主轴尺寸。
    layout.overflow_content = true;
    // 零交叉轴 frame 让输出完全由自然行组撑开。
    let output = layout.layout(Rect::new(4.0, 6.0, 25.0, 0.0), &children);
    // 首行从内容原点开始并保留三十像素高度。
    assert_eq!(output.positions[0], Rect::new(4.0, 6.0, 20.0, 30.0));
    // 末行在首行末端前八像素开始并与其重叠。
    assert_eq!(output.positions[1], Rect::new(4.0, 28.0, 20.0, 5.0));
    // 自然交叉轴必须覆盖结束于三十像素处的高首行，而不是末行的二十七像素末端。
    assert_eq!(output.total_size, Size::new(20.0, 30.0));
}

// 验证相同负行间距账本在垂直主轴下保持轴转置语义。
#[test]
// 较窄末列不得让总宽度退回到较宽首列的物理末端之前。
fn wrapped_overflow_negative_gap_keeps_the_farthest_column_end() {
    // 构造三十乘二十的宽首列子项。
    let first = LayoutChild::new(ComponentId::new(82), Size::new(30.0, 20.0));
    // 构造五乘二十的窄末列子项。
    let second = LayoutChild::new(ComponentId::new(83), Size::new(5.0, 20.0));
    // 二十五像素主轴即使扣除八像素负 gap 仍会让两个子项分成两列。
    let children = [first, second];
    // 构造从交叉轴起点排列的垂直布局。
    let mut layout = FlexLayout::column()
        // 负八像素 gap 同时让相邻列发生八像素重叠。
        .with_gap(-8.0)
        // 起点对齐隔离自然内容账本。
        .with_align(AlignItems::Start);
    // 开启实际换行路径。
    layout.wrap = true;
    // 保留两项自然主轴尺寸。
    layout.overflow_content = true;
    // 零交叉轴 frame 让输出完全由自然列组撑开。
    let output = layout.layout(Rect::new(6.0, 4.0, 0.0, 25.0), &children);
    // 首列从内容原点开始并保留三十像素宽度。
    assert_eq!(output.positions[0], Rect::new(6.0, 4.0, 30.0, 20.0));
    // 末列在首列末端前八像素开始并与其重叠。
    assert_eq!(output.positions[1], Rect::new(28.0, 4.0, 5.0, 20.0));
    // 自然交叉轴必须覆盖结束于三十像素处的宽首列，而不是末列的二十七像素末端。
    assert_eq!(output.total_size, Size::new(30.0, 20.0));
}

// 验证负 gap 在全部主轴分布模式和正反向布局中保持物理镜像。
#[test]
// 该矩阵同时覆盖水平与垂直主轴的实际分行路径。
fn wrapped_overflow_negative_gap_mirrors_distribution_modes() {
    // 构造首项并保留非对称主轴外边距。
    let mut first = LayoutChild::new(ComponentId::new(94), Size::new(20.0, 8.0));
    // 首项的水平外占用为二十三像素。
    first.margin = EdgeInsets::new(2.0, 1.0, 1.0, 2.0);
    // 构造第二项，使其与首项在负 gap 下保持同一行。
    let mut second = LayoutChild::new(ComponentId::new(95), Size::new(16.0, 7.0));
    // 第二项的水平外占用为十九像素。
    second.margin = EdgeInsets::new(1.0, 2.0, 2.0, 1.0);
    // 构造第三项，使其在负 gap 后进入第二行。
    let mut third = LayoutChild::new(ComponentId::new(96), Size::new(18.0, 6.0));
    // 第三项保留不同的水平起止外边距。
    third.margin = EdgeInsets::new(3.0, 1.0, 1.0, 2.0);
    // 保存水平正向参考输入。
    let row_children = [first, second, third];
    // 将水平输入逐项转置为垂直输入。
    let column_children = row_children.clone().map(|mut child| {
        // 读取待转置的自然尺寸。
        let size = child.measured_size;
        // 读取待转置的物理外边距。
        let margin = child.margin;
        // 交换子项自然尺寸的宽高。
        child.measured_size = Size::new(size.h, size.w);
        // 按轴转置规则交换四侧外边距。
        child.margin = EdgeInsets::new(margin.top, margin.left, margin.bottom, margin.right);
        // 返回完成转置的子项。
        child
    });
    // 枚举水平与垂直主轴的正反向配对。
    let direction_pairs = [
        // 水平正向与反向必须互为物理镜像。
        (FlexDirection::Row, FlexDirection::RowReverse),
        // 垂直正向与反向必须互为物理镜像。
        (FlexDirection::Column, FlexDirection::ColumnReverse),
    ];
    // 枚举公开的全部主轴分布模式。
    let justifications = [
        // 起点分布覆盖自然主轴游标。
        JustifyContent::Start,
        // 居中分布覆盖正剩余空间的半量偏移。
        JustifyContent::Center,
        // 末端分布覆盖正剩余空间的全量偏移。
        JustifyContent::End,
        // 两端分布覆盖负 gap 与动态项间距组合。
        JustifyContent::SpaceBetween,
        // 环绕分布覆盖行首、行尾与项间空间。
        JustifyContent::SpaceAround,
        // 均匀分布覆盖两端和项间空间。
        JustifyContent::SpaceEvenly,
        // Stretch 在溢出模式中应保持自然主轴尺寸。
        JustifyContent::Stretch,
    ];
    // 使用三十五像素真实主轴和零交叉轴启动尺寸。
    let row_frame = Rect::new(4.0, 6.0, 35.0, 0.0);
    // 逐条主轴验证正向与反向布局。
    for (pair_index, (forward_direction, reverse_direction)) in
        direction_pairs.into_iter().enumerate()
    {
        // 水平使用原始输入，垂直使用转置输入。
        let forward_children = if pair_index == 0 {
            // 选择水平参考子项。
            &row_children
        } else {
            // 选择垂直转置子项。
            &column_children
        };
        // 垂直分支交换 frame 两轴以保持主轴约束一致。
        let frame = if pair_index == 0 {
            // 水平分支直接使用参考 frame。
            row_frame
        } else {
            // 垂直分支把水平主轴换成交叉轴。
            Rect::new(row_frame.y, row_frame.x, row_frame.h, row_frame.w)
        };
        // 逐种主轴分布验证负 gap 的镜像语义。
        for justify in justifications {
            // 构造当前正向的实际分行布局。
            let mut forward_layout = FlexLayout::new()
                .with_direction(forward_direction)
                .with_gap(-8.0)
                .with_justify(justify)
                .with_align(AlignItems::Start);
            // 开启真实换行路径。
            forward_layout.wrap = true;
            // 冻结弹性因子，保留负 gap 的自然布局结果。
            forward_layout.overflow_content = true;
            // 执行正向参考布局。
            let forward_output = forward_layout.layout(frame, forward_children);
            // 构造交换主轴起止边距的反向输入。
            let reverse_children = forward_children.clone().map(|mut child| {
                // 读取待交换的四侧外边距。
                let margin = child.margin;
                // 根据当前主轴交换物理起止边距。
                child.margin = if pair_index == 0 {
                    // 水平主轴交换左右外边距。
                    EdgeInsets::new(margin.right, margin.top, margin.left, margin.bottom)
                } else {
                    // 垂直主轴交换上下外边距。
                    EdgeInsets::new(margin.left, margin.bottom, margin.right, margin.top)
                };
                // 返回反向布局使用的子项。
                child
            });
            // 复用当前分布参数并切换到反向主轴。
            let mut reverse_layout = forward_layout.clone();
            // 应用反向主轴方向。
            reverse_layout.direction = reverse_direction;
            // 执行反向布局。
            let reverse_output = reverse_layout.layout(frame, &reverse_children);
            // 把正向矩形围绕真实主轴边界镜像。
            let mirrored_positions: Vec<_> = forward_output
                .positions
                .iter()
                .copied()
                .map(|rect| {
                    // 水平和垂直方向分别镜像对应主轴坐标。
                    if pair_index == 0 {
                        // 水平镜像保留纵向坐标与尺寸。
                        Rect::new(
                            frame.x + frame.w - (rect.x - frame.x) - rect.w,
                            rect.y,
                            rect.w,
                            rect.h,
                        )
                    } else {
                        // 垂直镜像保留横向坐标与尺寸。
                        Rect::new(
                            rect.x,
                            frame.y + frame.h - (rect.y - frame.y) - rect.h,
                            rect.w,
                            rect.h,
                        )
                    }
                })
                .collect();
            // 反向布局只能改变主轴镜像后的矩形。
            assert_eq!(
                reverse_output.positions, mirrored_positions,
                "positions diverged for {reverse_direction:?}, {justify:?}, {frame:?}"
            );
            // 反向布局不得改变自然内容总尺寸。
            assert_eq!(
                reverse_output.total_size, forward_output.total_size,
                "total size diverged for {reverse_direction:?}, {justify:?}, {frame:?}"
            );
            // 读取首行的前两个可见矩形。
            let first_rect = forward_output.positions[0];
            // 读取负 gap 后的第二个可见矩形。
            let second_rect = forward_output.positions[1];
            // 读取第三项用于确认真实换行。
            let third_rect = forward_output.positions[2];
            // 负 gap 必须让前两个主轴矩形发生可见重叠。
            if pair_index == 0 {
                // 水平主轴比较横向交集。
                assert!(
                    second_rect.x < first_rect.x + first_rect.w
                        && first_rect.x < second_rect.x + second_rect.w,
                    "row negative gap did not overlap: {justify:?}"
                );
                // 第三项必须落到第二行。
                assert_ne!(third_rect.y, first_rect.y, "row negative gap did not wrap");
            } else {
                // 垂直主轴比较纵向交集。
                assert!(
                    second_rect.y < first_rect.y + first_rect.h
                        && first_rect.y < second_rect.y + second_rect.h,
                    "column negative gap did not overlap: {justify:?}"
                );
                // 第三项必须落到第二列。
                assert_ne!(
                    third_rect.x, first_rect.x,
                    "column negative gap did not wrap"
                );
            }
        }
    }
}
