// 导入溢出弹性冻结契约使用的核心几何类型。
use uix::core::{ComponentId, EdgeInsets, Rect, Size};
// 导入溢出弹性冻结契约使用的共享布局入口。
use uix::ui::{AlignItems, FlexDirection, FlexLayout, JustifyContent, LayoutChild, LayoutEngine};

// 验证实际分行溢出始终忽略子项的增长与压缩声明。
#[test]
// 同时覆盖全部方向、分布、对齐、异常弹性因子与交叉轴范围。
fn wrapped_overflow_multiline_freezes_flexibility_matrix() {
    // 构造首行第一个具有非对称尺寸与 margin 的子项。
    let mut first = LayoutChild::new(ComponentId::new(76), Size::new(15.0, 7.0));
    // 首项的水平外尺寸为二十像素。
    first.margin = EdgeInsets::new(2.0, 1.0, 3.0, 4.0);
    // 首项固定在各自行盒交叉轴起点。
    first.align_self = Some(AlignItems::Start);
    // 构造首行第二个自然尺寸不同的子项。
    let mut second = LayoutChild::new(ComponentId::new(77), Size::new(14.0, 9.0));
    // 次项的水平外尺寸为十七像素。
    second.margin = EdgeInsets::new(1.0, 3.0, 2.0, 1.0);
    // 构造第二行的超宽单项。
    let mut third = LayoutChild::new(ComponentId::new(78), Size::new(38.0, 8.0));
    // 第三项的水平外尺寸为四十三像素。
    third.margin = EdgeInsets::new(4.0, 2.0, 1.0, 5.0);
    // 第三项固定在各自行盒交叉轴末端。
    third.align_self = Some(AlignItems::End);
    // 构造第三行的独立窄项。
    let mut fourth = LayoutChild::new(ComponentId::new(79), Size::new(18.0, 6.0));
    // 第四项的水平外尺寸为二十二像素。
    fourth.margin = EdgeInsets::new(1.0, 4.0, 3.0, 2.0);
    // 第四项显式拉伸以覆盖逐项 Stretch。
    fourth.align_self = Some(AlignItems::Stretch);
    // 四十五像素主轴与四像素 gap 会稳定形成二加一加一三行。
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
    // 枚举两条主轴及其反向形式。
    let directions = [
        // 水平正向覆盖普通行流。
        FlexDirection::Row,
        // 水平反向覆盖真实宽度镜像。
        FlexDirection::RowReverse,
        // 垂直正向覆盖普通列流。
        FlexDirection::Column,
        // 垂直反向覆盖真实高度镜像。
        FlexDirection::ColumnReverse,
    ];
    // 枚举公开主轴分布的完整集合。
    let justifications = [
        // 起点分布验证自然游标。
        JustifyContent::Start,
        // 居中分布验证每行独立偏移。
        JustifyContent::Center,
        // 末端分布验证每行独立末端贴合。
        JustifyContent::End,
        // 两端分布验证动态项间距。
        JustifyContent::SpaceBetween,
        // 环绕分布验证行首与项间空间。
        JustifyContent::SpaceAround,
        // 均匀分布验证两端与项间空间。
        JustifyContent::SpaceEvenly,
        // 溢出模式中的 Stretch 也必须保持自然主轴尺寸。
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
    // 逐种方向比较自然因子与异常因子输入。
    for direction in directions {
        // 当前方向是否使用水平主轴。
        let is_row = matches!(direction, FlexDirection::Row | FlexDirection::RowReverse);
        // 水平使用原始子项，垂直使用轴转置子项。
        let baseline_children = if is_row {
            // 水平分支借用原始子项。
            &row_children
        } else {
            // 垂直分支借用转置子项。
            &column_children
        };
        // 构造包含有限、负值、无穷与非数因子的受测输入。
        let flexible_children = baseline_children.clone().map(|mut child| {
            // 按身份为每项写入不同增长因子。
            child.flex_grow = match child.id {
                // 首项声明普通正增长。
                id if id == ComponentId::new(76) => 1.0,
                // 次项声明大权重增长。
                id if id == ComponentId::new(77) => 100.0,
                // 第三项声明有限上界哨兵。
                id if id == ComponentId::new(78) => f32::MAX,
                // 第四项声明负增长。
                _ => -7.0,
            };
            // 按身份为每项写入不同压缩因子。
            child.flex_shrink = match child.id {
                // 首项声明普通正压缩。
                id if id == ComponentId::new(76) => 1.0,
                // 次项声明零压缩。
                id if id == ComponentId::new(77) => 0.0,
                // 第三项声明正无穷压缩。
                id if id == ComponentId::new(78) => f32::INFINITY,
                // 第四项声明非数压缩。
                _ => f32::NAN,
            };
            // 返回只改变弹性因子的受测子项。
            child
        });
        // 逐种主轴分布核验自然尺寸冻结。
        for justify in justifications {
            // 逐种交叉轴对齐核验弹性因子不泄漏到行组。
            for align in alignments {
                // 逐种交叉轴范围执行等价比较。
                for row_frame in row_frames {
                    // 垂直分支交换 frame 两轴以保留相同主轴上限。
                    let frame = if is_row {
                        // 水平分支直接使用参考 frame。
                        row_frame
                    } else {
                        // 垂直分支交换原点与尺寸两轴。
                        Rect::new(row_frame.y, row_frame.x, row_frame.h, row_frame.w)
                    };
                    // 构造实际分行的溢出布局。
                    let mut layout = FlexLayout::new()
                        // 应用当前主轴方向。
                        .with_direction(direction)
                        // 四像素同时作为项间距与行间距。
                        .with_gap(4.0)
                        // 应用当前逐行主轴分布。
                        .with_justify(justify)
                        // 应用当前容器级交叉轴对齐。
                        .with_align(align);
                    // 开启实际换行路径。
                    layout.wrap = true;
                    // 冻结增长与压缩并保留自然主轴尺寸。
                    layout.overflow_content = true;
                    // 执行零弹性因子的自然基准布局。
                    let baseline = layout.layout(frame, baseline_children);
                    // 执行包含异常弹性声明的受测布局。
                    let output = layout.layout(frame, &flexible_children);
                    // 弹性声明不得改变分行或任一子项矩形。
                    assert_eq!(
                        output.positions, baseline.positions,
                        // 失败消息标识当前方向、分布、对齐与 frame。
                        "positions diverged for {direction:?}, {justify:?}, {align:?}, {frame:?}"
                    );
                    // 弹性声明不得改变自然内容总尺寸账本。
                    assert_eq!(
                        output.total_size, baseline.total_size,
                        // 失败消息沿用同一组合标识。
                        "total size diverged for {direction:?}, {justify:?}, {align:?}, {frame:?}"
                    );
                }
            }
        }
    }
}
