// 导入负主轴契约使用的核心几何类型。
use uix::core::{ComponentId, EdgeInsets, Rect, Size};
// 导入负主轴契约使用的共享布局入口。
use uix::ui::{AlignItems, FlexDirection, FlexLayout, JustifyContent, LayoutChild, LayoutEngine};

// 验证负主轴间距与负主轴起始边距在正向和反向布局中保持物理镜像。
#[test]
// 同时覆盖水平、垂直、实际分行和交叉轴 bootstrap 输入。
fn wrapped_overflow_negative_main_axis_mirrors_all_directions() {
    // 构造第一个子项，并让其正向主轴起始边距为负值。
    let mut first = LayoutChild::new(ComponentId::new(84), Size::new(20.0, 8.0));
    // 首项的水平主轴起始边距为负六像素。
    first.margin = EdgeInsets::new(-6.0, 1.0, 0.0, 2.0);
    // 构造第二个子项，使其与第一个子项在负主轴间距下发生可见重叠。
    let mut second = LayoutChild::new(ComponentId::new(85), Size::new(16.0, 7.0));
    // 次项保留非对称四侧边距以验证反向边距交换。
    second.margin = EdgeInsets::new(2.0, 1.0, 1.0, 3.0);
    // 构造第三个子项，使前两个子项同处首行而第三个子项进入下一行。
    let mut third = LayoutChild::new(ComponentId::new(86), Size::new(18.0, 6.0));
    // 末项保留非对称四侧边距以验证行列转置。
    third.margin = EdgeInsets::new(1.0, 2.0, 2.0, 1.0);
    // 保存水平正向布局的子项矩阵。
    let row_children = [first, second, third];
    // 定义尺寸的轴转置，水平分量与垂直分量互换。
    let transpose_size = |size: Size| Size::new(size.h, size.w);
    // 定义物理边距的轴转置。
    let transpose_margin = |margin: EdgeInsets| {
        // 返回保持几何转置语义的四侧外边距。
        EdgeInsets::new(margin.top, margin.left, margin.bottom, margin.right)
    };
    // 从水平输入逐项构造完全转置的垂直输入。
    let column_children = row_children.clone().map(|mut child| {
        // 交换子项自然尺寸的宽高。
        child.measured_size = transpose_size(child.measured_size);
        // 按相同坐标变换交换四侧物理边距。
        child.margin = transpose_margin(child.margin);
        // 返回保留身份的转置子项。
        child
    });
    // 定义正向到反向布局的水平主轴边距镜像。
    let mirror_horizontal_margin = |margin: EdgeInsets| {
        // 交换水平主轴的左右边距并保持交叉轴边距。
        EdgeInsets::new(margin.right, margin.top, margin.left, margin.bottom)
    };
    // 定义正向到反向布局的垂直主轴边距镜像。
    let mirror_vertical_margin = |margin: EdgeInsets| {
        // 交换垂直主轴的上下边距并保持交叉轴边距。
        EdgeInsets::new(margin.left, margin.bottom, margin.right, margin.top)
    };
    // 枚举两条主轴的正向与反向配对。
    let direction_pairs = [
        // 水平正向应围绕真实宽度镜像为水平反向。
        (FlexDirection::Row, FlexDirection::RowReverse),
        // 垂直正向应围绕真实高度镜像为垂直反向。
        (FlexDirection::Column, FlexDirection::ColumnReverse),
    ];
    // 枚举实际交叉轴尺寸与零交叉轴启动尺寸。
    let row_frames = [
        // 四十像素交叉轴提供正常自然行组空间。
        Rect::new(4.0, 6.0, 35.0, 40.0),
        // 零交叉轴触发自然行组 bootstrap。
        Rect::new(4.0, 6.0, 35.0, 0.0),
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
        // 逐种交叉轴范围执行正反向比较。
        for (frame_index, row_frame) in row_frames.into_iter().enumerate() {
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
                // 负八像素同时作为项间距与行间距。
                .with_gap(-8.0)
                // 从主轴起点排列以保留负间距的直接几何效果。
                .with_justify(JustifyContent::Start)
                // 从交叉轴起点排列以隔离自然内容账本。
                .with_align(AlignItems::Start);
            // 开启实际换行路径。
            forward_layout.wrap = true;
            // 冻结增长与压缩并保留自然主轴尺寸。
            forward_layout.overflow_content = true;
            // 执行正向参考布局。
            let forward_output = forward_layout.layout(frame, forward_children);
            // 反向主轴的物理镜像必须交换每项的主轴起止边距。
            let reverse_children = forward_children.clone().map(|mut child| {
                // 根据当前主轴交换对应的起止边距。
                child.margin = if pair_index == 0 {
                    // 水平主轴交换左右边距。
                    mirror_horizontal_margin(child.margin)
                } else {
                    // 垂直主轴交换上下边距。
                    mirror_vertical_margin(child.margin)
                };
                // 返回只交换主轴边距的反向输入。
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
                // 转为复制迭代以保留正向自然尺寸断言。
                .iter()
                .copied()
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
                // 失败消息标识当前方向与 frame。
                "positions diverged for {reverse_direction:?}, {frame:?}"
            );
            // 反向排列不得改变自然内容尺寸账本。
            assert_eq!(
                reverse_output.total_size, forward_output.total_size,
                // 失败消息沿用当前方向与 frame 标识。
                "total size diverged for {reverse_direction:?}, {frame:?}"
            );
            // 负主轴 gap 与边距的自然主轴账本应保持首行的二十五像素占用。
            let expected_main = 25.0;
            // 按当前主轴方向读取自然总尺寸。
            let actual_main = if pair_index == 0 {
                // 水平布局从总宽度读取主轴账本。
                forward_output.total_size.w
            } else {
                // 垂直布局从总高度读取主轴账本。
                forward_output.total_size.h
            };
            // 要求固定主轴不被容器的三十五像素分配尺寸替代。
            assert_eq!(
                actual_main, expected_main,
                // 失败消息标识自然主轴账本异常的组合。
                "natural main extent diverged: pair={pair_index}, frame={frame_index}"
            );
            // 要求第三个子项确实进入了不同的物理交叉轴位置。
            if pair_index == 0 {
                // 水平布局应在纵轴上产生第二行。
                assert_ne!(
                    forward_output.positions[2].y, forward_output.positions[0].y,
                    // 失败消息标识未发生真实换行的组合。
                    "row negative main-axis case did not wrap: frame={frame_index}"
                );
            } else {
                // 垂直布局应在横轴上产生第二列。
                assert_ne!(
                    forward_output.positions[2].x, forward_output.positions[0].x,
                    // 失败消息标识未发生真实换列的组合。
                    "column negative main-axis case did not wrap: frame={frame_index}"
                );
            }
            // 要求前两个子项在负主轴间距下保持可见重叠。
            if pair_index == 0 {
                // 水平布局比较两个可见矩形的横向交集。
                assert!(
                    forward_output.positions[1].x
                        < forward_output.positions[0].x + forward_output.positions[0].w
                        && forward_output.positions[0].x
                            < forward_output.positions[1].x + forward_output.positions[1].w,
                    // 失败消息标识负主轴间距未产生水平重叠。
                    "row negative main-axis gap did not overlap: frame={frame_index}"
                );
            } else {
                // 垂直布局比较两个可见矩形的纵向交集。
                assert!(
                    forward_output.positions[1].y
                        < forward_output.positions[0].y + forward_output.positions[0].h
                        && forward_output.positions[0].y
                            < forward_output.positions[1].y + forward_output.positions[1].h,
                    // 失败消息标识负主轴间距未产生垂直重叠。
                    "column negative main-axis gap did not overlap: frame={frame_index}"
                );
            }
        }
    }
}
