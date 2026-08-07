// 导入反向溢出契约使用的核心几何类型。
use uix::core::{ComponentId, EdgeInsets, Rect, Size};
// 导入反向溢出契约使用的共享布局入口。
use uix::ui::{AlignItems, FlexDirection, FlexLayout, JustifyContent, LayoutChild, LayoutEngine};

// 验证实际分行溢出在正向与反向主轴之间保持物理镜像语义。
#[test]
// 同时覆盖全部分布和对齐、非对称主轴 margin、逐项对齐与交叉轴溢出。
fn wrapped_overflow_multiline_reverse_mirrors_real_main_extent() {
    // 构造首行第一个具有非对称尺寸与 margin 的子项。
    let mut first = LayoutChild::new(ComponentId::new(72), Size::new(15.0, 7.0));
    // 首项的水平外尺寸为二十像素。
    first.margin = EdgeInsets::new(2.0, 1.0, 3.0, 4.0);
    // 首项固定在各自行盒交叉轴起点。
    first.align_self = Some(AlignItems::Start);
    // 构造首行第二个自然尺寸不同的子项。
    let mut second = LayoutChild::new(ComponentId::new(73), Size::new(14.0, 9.0));
    // 次项的水平外尺寸为十七像素。
    second.margin = EdgeInsets::new(1.0, 3.0, 2.0, 1.0);
    // 构造第二行的超宽单项。
    let mut third = LayoutChild::new(ComponentId::new(74), Size::new(38.0, 8.0));
    // 第三项的水平外尺寸为四十三像素。
    third.margin = EdgeInsets::new(4.0, 2.0, 1.0, 5.0);
    // 第三项固定在各自行盒交叉轴末端。
    third.align_self = Some(AlignItems::End);
    // 构造第三行的独立窄项。
    let mut fourth = LayoutChild::new(ComponentId::new(75), Size::new(18.0, 6.0));
    // 第四项的水平外尺寸为二十二像素。
    fourth.margin = EdgeInsets::new(1.0, 4.0, 3.0, 2.0);
    // 第四项显式拉伸以覆盖逐项 Stretch 的反向镜像。
    fourth.align_self = Some(AlignItems::Stretch);
    // 四十五像素主轴与四像素 gap 会稳定形成二加一加一的三行。
    let row_children = [first, second, third, fourth];
    // 定义尺寸的轴转置，水平分量与垂直分量互换。
    let transpose_size = |size: Size| Size::new(size.h, size.w);
    // 定义物理 margin 的轴转置。
    let transpose_margin = |margin: EdgeInsets| {
        // 返回保持几何转置语义的四侧外边距。
        EdgeInsets::new(margin.top, margin.left, margin.bottom, margin.right)
    };
    // 从水平输入逐项构造完全转置的垂直输入。
    let column_children = row_children.clone().map(|mut child| {
        // 子项自然尺寸交换宽高。
        child.measured_size = transpose_size(child.measured_size);
        // 子项物理 margin 按相同坐标变换交换四侧。
        child.margin = transpose_margin(child.margin);
        // 返回保留身份与 align_self 的转置子项。
        child
    });
    // 枚举两条主轴的正向与反向配对。
    let direction_pairs = [
        // 水平正向应围绕真实宽度镜像为水平反向。
        (FlexDirection::Row, FlexDirection::RowReverse),
        // 垂直正向应围绕真实高度镜像为垂直反向。
        (FlexDirection::Column, FlexDirection::ColumnReverse),
    ];
    // 枚举公开主轴分布的完整集合。
    let justifications = [
        // 起点分布验证反向主轴末端起步。
        JustifyContent::Start,
        // 居中分布验证每行独立的半量偏移。
        JustifyContent::Center,
        // 末端分布验证每行独立的完整偏移。
        JustifyContent::End,
        // 两端分布验证动态项间距的镜像。
        JustifyContent::SpaceBetween,
        // 环绕分布验证行首与项间空间的镜像。
        JustifyContent::SpaceAround,
        // 均匀分布验证两端空间的镜像。
        JustifyContent::SpaceEvenly,
        // 溢出模式中的 Stretch 保持自然主轴尺寸。
        JustifyContent::Stretch,
    ];
    // 枚举容器级交叉轴对齐的完整集合。
    let alignments = [
        // 起点对齐覆盖自然行组位置。
        AlignItems::Start,
        // 居中对齐覆盖正负交叉轴剩余空间。
        AlignItems::Center,
        // 末端对齐覆盖正负交叉轴剩余空间。
        AlignItems::End,
        // 拉伸对齐覆盖多行剩余空间分配。
        AlignItems::Stretch,
    ];
    // 枚举宽裕、溢出和 bootstrap 三种水平交叉轴范围。
    let row_frames = [
        // 六十像素交叉轴为三行提供正剩余空间。
        Rect::new(4.0, 6.0, 45.0, 60.0),
        // 二十像素交叉轴小于自然行组高度。
        Rect::new(4.0, 6.0, 45.0, 20.0),
        // 零交叉轴触发自然行组 bootstrap。
        Rect::new(4.0, 6.0, 45.0, 0.0),
    ];
    // 逐条主轴比较正向与反向结果。
    for (pair_index, (forward_direction, reverse_direction)) in
        direction_pairs.into_iter().enumerate()
    {
        // 水平使用原始子项，垂直使用轴转置子项。
        let forward_children = if pair_index == 0 {
            // 水平分支借用原始子项。
            &row_children
        } else {
            // 垂直分支借用转置子项。
            &column_children
        };
        // 逐种主轴分布核验每行独立镜像。
        for justify in justifications {
            // 逐种交叉轴对齐核验镜像不影响行组语义。
            for align in alignments {
                // 逐种交叉轴范围执行正反向比较。
                for row_frame in row_frames {
                    // 垂直分支交换 frame 两轴以保留相同主轴上限。
                    let frame = if pair_index == 0 {
                        // 水平分支直接使用参考 frame。
                        row_frame
                    } else {
                        // 垂直分支交换原点与尺寸两轴。
                        Rect::new(row_frame.y, row_frame.x, row_frame.h, row_frame.w)
                    };
                    // 构造实际分行的正向溢出布局。
                    let mut forward_layout = FlexLayout::new()
                        // 应用当前正向主轴。
                        .with_direction(forward_direction)
                        // 四像素同时作为项间距与行间距。
                        .with_gap(4.0)
                        // 应用当前逐行主轴分布。
                        .with_justify(justify)
                        // 应用当前容器级交叉轴对齐。
                        .with_align(align);
                    // 开启实际换行路径。
                    forward_layout.wrap = true;
                    // 冻结增长与压缩并保留自然主轴尺寸。
                    forward_layout.overflow_content = true;
                    // 执行正向参考布局。
                    let forward_output = forward_layout.layout(frame, forward_children);
                    // 反向主轴的物理镜像必须交换每项的主轴起止 margin。
                    let reverse_children = forward_children.clone().map(|mut child| {
                        // 读取待交换的四侧 margin。
                        let margin = child.margin;
                        // 按当前主轴交换物理起止侧，交叉轴两侧保持不变。
                        child.margin = if pair_index == 0 {
                            // 水平主轴交换左右 margin。
                            EdgeInsets::new(margin.right, margin.top, margin.left, margin.bottom)
                        } else {
                            // 垂直主轴交换上下 margin。
                            EdgeInsets::new(margin.left, margin.bottom, margin.right, margin.top)
                        };
                        // 返回只交换主轴 margin 的反向输入。
                        child
                    });
                    // 克隆全部布局参数后只切换主轴方向。
                    let mut reverse_layout = forward_layout.clone();
                    // 应用当前反向主轴。
                    reverse_layout.direction = reverse_direction;
                    // 执行受测的反向布局。
                    let reverse_output = reverse_layout.layout(frame, &reverse_children);
                    // 逐项把正向矩形围绕真实容器主轴镜像。
                    let mirrored_positions: Vec<_> = forward_output
                        // 读取正向子项矩形。
                        .positions
                        // 转为所有权迭代以避免额外克隆。
                        .into_iter()
                        // 围绕当前主轴的真实 frame 边界镜像。
                        .map(|rect| {
                            // 水平与垂直方向分别镜像主轴坐标。
                            if pair_index == 0 {
                                // 水平镜像保留纵轴几何。
                                Rect::new(
                                    frame.x + frame.w - (rect.x - frame.x) - rect.w,
                                    rect.y,
                                    rect.w,
                                    rect.h,
                                )
                            } else {
                                // 垂直镜像保留横轴几何。
                                Rect::new(
                                    rect.x,
                                    frame.y + frame.h - (rect.y - frame.y) - rect.h,
                                    rect.w,
                                    rect.h,
                                )
                            }
                        })
                        // 收集为与反向输出相同顺序的矩形数组。
                        .collect();
                    // 反向分支不得改变镜像以外的任一子项几何。
                    assert_eq!(
                        reverse_output.positions, mirrored_positions,
                        // 失败消息标识当前方向、分布、对齐与 frame。
                        "positions diverged for {reverse_direction:?}, {justify:?}, {align:?}, {frame:?}"
                    );
                    // 反向排列不得改变自然内容尺寸账本。
                    assert_eq!(
                        reverse_output.total_size, forward_output.total_size,
                        // 失败消息沿用同一组合标识。
                        "total size diverged for {reverse_direction:?}, {justify:?}, {align:?}, {frame:?}"
                    );
                }
            }
        }
    }
}
