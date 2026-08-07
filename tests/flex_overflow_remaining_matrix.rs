// 导入联合矩阵契约使用的核心几何类型。
use uix::core::{ComponentId, EdgeInsets, Rect, Size};
// 导入联合矩阵契约使用的共享布局入口。
use uix::ui::{AlignItems, FlexDirection, FlexLayout, JustifyContent, LayoutChild, LayoutEngine};

// 验证负起始边距、负 gap 与全部轴向组合共同闭合。
#[test]
// 同时覆盖四种方向、全部分布、全部交叉轴对齐和三种交叉轴范围。
fn wrapped_overflow_remaining_negative_margin_gap_matrix_is_symmetric() {
    // 构造带负水平起始边距的首项。
    let mut first = LayoutChild::new(ComponentId::new(98), Size::new(20.0, 8.0));
    // 首项的水平起始边距为负六像素，尾侧仍保留正占用。
    first.margin = EdgeInsets::new(-6.0, 1.0, 2.0, 2.0);
    // 首项显式覆盖为交叉轴起点，验证逐项对齐优先级。
    first.align_self = Some(AlignItems::Start);
    // 构造保留非对称边距的第二项。
    let mut second = LayoutChild::new(ComponentId::new(99), Size::new(16.0, 7.0));
    // 第二项的四侧边距参与负 gap 下的首行账本。
    second.margin = EdgeInsets::new(1.0, 2.0, 2.0, 1.0);
    // 第二项显式覆盖为交叉轴居中，补齐逐项居中组合。
    second.align_self = Some(AlignItems::Center);
    // 构造会在负 gap 后进入第二行的第三项。
    let mut third = LayoutChild::new(ComponentId::new(100), Size::new(18.0, 6.0));
    // 第三项保留与前两项不同的四侧边距。
    third.margin = EdgeInsets::new(3.0, 1.0, 1.0, 2.0);
    // 第三项显式覆盖为交叉轴拉伸，覆盖另一种逐项模式。
    third.align_self = Some(AlignItems::Stretch);
    // 保存水平正向参考输入。
    let row_children = [first, second, third];
    // 定义尺寸的轴转置，水平分量与垂直分量互换。
    let transpose_size = |size: Size| Size::new(size.h, size.w);
    // 定义物理边距的轴转置，左上右下映射为上左右下。
    let transpose_margin = |margin: EdgeInsets| {
        // 返回完成轴转置的四侧边距。
        EdgeInsets::new(margin.top, margin.left, margin.bottom, margin.right)
    };
    // 定义矩形的轴转置，同时交换原点与尺寸分量。
    let transpose_rect = |rect: Rect| Rect::new(rect.y, rect.x, rect.h, rect.w);
    // 从水平输入逐项构造垂直正向输入。
    let column_children = row_children.clone().map(|mut child| {
        // 交换子项自然尺寸的宽高。
        child.measured_size = transpose_size(child.measured_size);
        // 交换子项物理边距的四侧坐标。
        child.margin = transpose_margin(child.margin);
        // 返回保留身份与逐项对齐配置的转置子项。
        child
    });
    // 从水平输入构造交换主轴起止边距的反向输入。
    let row_reverse_children = row_children.clone().map(|mut child| {
        // 读取待交换的水平主轴边距。
        let margin = child.margin;
        // 交换左右边距并保留上下交叉轴边距。
        child.margin = EdgeInsets::new(margin.right, margin.top, margin.left, margin.bottom);
        // 返回水平反向布局使用的子项。
        child
    });
    // 从垂直正向输入构造交换主轴起止边距的反向输入。
    let column_reverse_children = column_children.clone().map(|mut child| {
        // 读取待交换的垂直主轴边距。
        let margin = child.margin;
        // 交换上下边距并保留左右交叉轴边距。
        child.margin = EdgeInsets::new(margin.left, margin.bottom, margin.right, margin.top);
        // 返回垂直反向布局使用的子项。
        child
    });
    // 定义可复用的联合矩阵布局构造器。
    let make_layout = |direction: FlexDirection, justify: JustifyContent, align: AlignItems| {
        // 设置当前方向、负 gap、主轴分布和交叉轴对齐。
        let mut layout = FlexLayout::new()
            .with_direction(direction)
            .with_gap(-8.0)
            .with_justify(justify)
            .with_align(align);
        // 开启真实多行路径。
        layout.wrap = true;
        // 冻结增长与压缩，保留自然尺寸布局。
        layout.overflow_content = true;
        // 同时覆盖固有主轴入口。
        layout.intrinsic_main = true;
        // 返回当前组合的布局参数。
        layout
    };
    // 定义水平正向与反向的物理镜像变换。
    let mirror_horizontal = |rect: Rect, frame: Rect| {
        // 只镜像主轴坐标并保留交叉轴几何。
        Rect::new(
            frame.x + frame.w - (rect.x - frame.x) - rect.w,
            rect.y,
            rect.w,
            rect.h,
        )
    };
    // 定义垂直正向与反向的物理镜像变换。
    let mirror_vertical = |rect: Rect, frame: Rect| {
        // 只镜像主轴坐标并保留交叉轴几何。
        Rect::new(
            rect.x,
            frame.y + frame.h - (rect.y - frame.y) - rect.h,
            rect.w,
            rect.h,
        )
    };
    // 枚举公开主轴分布的完整集合。
    let justifications = [
        // 覆盖主轴起点分布。
        JustifyContent::Start,
        // 覆盖主轴居中分布。
        JustifyContent::Center,
        // 覆盖主轴末端分布。
        JustifyContent::End,
        // 覆盖两端分布。
        JustifyContent::SpaceBetween,
        // 覆盖环绕分布。
        JustifyContent::SpaceAround,
        // 覆盖均匀分布。
        JustifyContent::SpaceEvenly,
        // 覆盖溢出路径下的 Stretch 分布。
        JustifyContent::Stretch,
    ];
    // 枚举容器级交叉轴对齐的完整集合。
    let alignments = [
        // 覆盖交叉轴起点对齐。
        AlignItems::Start,
        // 覆盖交叉轴居中对齐。
        AlignItems::Center,
        // 覆盖交叉轴末端对齐。
        AlignItems::End,
        // 覆盖交叉轴拉伸对齐。
        AlignItems::Stretch,
    ];
    // 枚举交叉轴宽裕、溢出和零尺寸 bootstrap。
    let row_frames = [
        // 三十像素主轴保留负 gap 的所有分布余量，交叉轴提供宽裕空间。
        Rect::new(10.0, 20.0, 30.0, 40.0),
        // 三十像素主轴保留负 gap，十五像素交叉轴覆盖自然行组溢出。
        Rect::new(10.0, 20.0, 30.0, 15.0),
        // 三十像素主轴保留负 gap，零交叉轴覆盖固有尺寸 bootstrap。
        Rect::new(10.0, 20.0, 30.0, 0.0),
    ];
    // 逐种主轴分布、交叉轴对齐和 frame 执行全方向比较。
    for justify in justifications {
        // 逐种验证容器级交叉轴对齐。
        for align in alignments {
            // 逐种验证交叉轴空间状态。
            for row_frame in row_frames {
                // 构造水平正向联合矩阵布局。
                let row_layout = make_layout(FlexDirection::Row, justify, align);
                // 执行水平正向参考布局。
                let row_output = row_layout.layout(row_frame, &row_children);
                // 构造轴转置后的垂直正向布局。
                let column_layout = make_layout(FlexDirection::Column, justify, align);
                // 交换 frame 两轴以保持相同的主轴约束。
                let column_frame = transpose_rect(row_frame);
                // 执行垂直正向转置布局。
                let column_output = column_layout.layout(column_frame, &column_children);
                // 把垂直输出的每个矩形转置回水平坐标。
                let transposed_positions: Vec<_> = column_output
                    .positions
                    .iter()
                    .copied()
                    .map(transpose_rect)
                    .collect();
                // 轴转置不得改变任一子项的物理矩形。
                assert_eq!(
                    transposed_positions, row_output.positions,
                    "positions diverged for {justify:?}, {align:?}, {row_frame:?}"
                );
                // 轴转置后的自然尺寸账本必须完全一致。
                assert_eq!(
                    transpose_size(column_output.total_size),
                    row_output.total_size,
                    "total size diverged for {justify:?}, {align:?}, {row_frame:?}"
                );
                // 构造水平反向布局并使用交换后的主轴边距。
                let row_reverse_layout = make_layout(FlexDirection::RowReverse, justify, align);
                // 执行水平反向布局。
                let row_reverse_output =
                    row_reverse_layout.layout(row_frame, &row_reverse_children);
                // 计算水平正向结果围绕 frame 主轴的镜像。
                let mirrored_row_positions: Vec<_> = row_output
                    .positions
                    .iter()
                    .copied()
                    .map(|rect| mirror_horizontal(rect, row_frame))
                    .collect();
                // 水平反向布局必须等于正向物理镜像。
                assert_eq!(
                    row_reverse_output.positions, mirrored_row_positions,
                    "horizontal mirror diverged for {justify:?}, {align:?}, {row_frame:?}"
                );
                // 水平反向不得改变自然尺寸账本。
                assert_eq!(
                    row_reverse_output.total_size, row_output.total_size,
                    "horizontal total size diverged for {justify:?}, {align:?}, {row_frame:?}"
                );
                // 构造垂直反向布局并使用轴转置后的反向输入。
                let column_reverse_layout =
                    make_layout(FlexDirection::ColumnReverse, justify, align);
                // 执行垂直反向布局。
                let column_reverse_output =
                    column_reverse_layout.layout(column_frame, &column_reverse_children);
                // 计算垂直正向结果围绕 frame 主轴的镜像。
                let mirrored_column_positions: Vec<_> = column_output
                    .positions
                    .iter()
                    .copied()
                    .map(|rect| mirror_vertical(rect, column_frame))
                    .collect();
                // 垂直反向布局必须等于正向物理镜像。
                assert_eq!(
                    column_reverse_output.positions, mirrored_column_positions,
                    "vertical mirror diverged for {justify:?}, {align:?}, {row_frame:?}"
                );
                // 垂直反向不得改变自然尺寸账本。
                assert_eq!(
                    column_reverse_output.total_size, column_output.total_size,
                    "vertical total size diverged for {justify:?}, {align:?}, {row_frame:?}"
                );
                // 把垂直反向输出转置回水平坐标。
                let transposed_reverse_positions: Vec<_> = column_reverse_output
                    .positions
                    .iter()
                    .copied()
                    .map(transpose_rect)
                    .collect();
                // 垂直反向与水平反向必须保持完整轴转置。
                assert_eq!(
                    transposed_reverse_positions, row_reverse_output.positions,
                    "reverse transpose diverged for {justify:?}, {align:?}, {row_frame:?}"
                );
                // 垂直反向的自然尺寸账本必须同步转置。
                assert_eq!(
                    transpose_size(column_reverse_output.total_size),
                    row_reverse_output.total_size,
                    "reverse total size diverged for {justify:?}, {align:?}, {row_frame:?}"
                );
                // 读取首行的前两个矩形，确认负 gap 仍产生可见重叠。
                let first_rect = row_output.positions[0];
                // 读取负 gap 后的第二个矩形。
                let second_rect = row_output.positions[1];
                // 读取第三项，确认负起始边距叠加后仍实际换行。
                let third_rect = row_output.positions[2];
                // 前两个子项必须在水平主轴上保持可见重叠。
                assert!(
                    second_rect.x < first_rect.x + first_rect.w
                        && first_rect.x < second_rect.x + second_rect.w,
                    "negative gap did not overlap for {justify:?}, {align:?}, {row_frame:?}"
                );
                // 第三项必须位于不同的实际行。
                assert_ne!(
                    third_rect.y, first_rect.y,
                    "negative margin/gap case did not wrap for {justify:?}, {align:?}, {row_frame:?}"
                );
            }
        }
    }
}
