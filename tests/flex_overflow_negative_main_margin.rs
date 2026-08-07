// 导入负主轴外边距契约使用的几何类型。
use uix::core::{ComponentId, EdgeInsets, Rect, Size};
// 导入负主轴外边距契约使用的布局入口。
use uix::ui::{AlignItems, FlexDirection, FlexLayout, LayoutChild, LayoutEngine};

// 验证水平多行布局不会用负右外边距裁掉子项可见末端。
#[test]
// 较窄第二行不得让自然总宽度停在首行可见矩形之前。
fn wrapped_overflow_negative_right_main_margin_keeps_visible_width() {
    // 构造三十像素宽、右外边距为负十像素的首行子项。
    let mut first = LayoutChild::new(ComponentId::new(88), Size::new(30.0, 20.0));
    // 负尾侧主轴外边距只把首行占用缩短到二十像素。
    first.margin = EdgeInsets::new(0.0, 0.0, -10.0, 0.0);
    // 构造会落入第二行的十像素宽子项。
    let second = LayoutChild::new(ComponentId::new(89), Size::new(10.0, 5.0));
    // 二十五像素主轴空间使二十加十的外占用形成两行。
    let children = [first, second];
    // 构造从交叉轴起点排列的水平布局。
    let mut layout = FlexLayout::row().with_align(AlignItems::Start);
    // 开启真实换行路径。
    layout.wrap = true;
    // 保留两个子项的自然主轴尺寸。
    layout.overflow_content = true;
    // 零交叉轴 frame 让输出完全由自然行组撑开。
    let output = layout.layout(Rect::new(4.0, 6.0, 25.0, 0.0), &children);
    // 首行子项保持完整的三十像素可见宽度。
    assert_eq!(output.positions[0], Rect::new(4.0, 6.0, 30.0, 20.0));
    // 第二行仍从内容原点开始并保留自然尺寸。
    assert_eq!(output.positions[1], Rect::new(4.0, 26.0, 10.0, 5.0));
    // 总宽度必须覆盖首项结束于三十像素处的可见矩形。
    assert_eq!(output.total_size, Size::new(30.0, 25.0));
}

// 验证垂直多列布局保持相同的轴转置可见范围语义。
#[test]
// 较矮第二列不得让自然总高度停在首列可见矩形之前。
fn wrapped_overflow_negative_bottom_main_margin_keeps_visible_height() {
    // 构造三十像素高、下外边距为负十像素的首列子项。
    let mut first = LayoutChild::new(ComponentId::new(90), Size::new(20.0, 30.0));
    // 负尾侧主轴外边距只把首列占用缩短到二十像素。
    first.margin = EdgeInsets::new(0.0, 0.0, 0.0, -10.0);
    // 构造会落入第二列的十像素高子项。
    let second = LayoutChild::new(ComponentId::new(91), Size::new(5.0, 10.0));
    // 二十五像素主轴空间使二十加十的外占用形成两列。
    let children = [first, second];
    // 构造从交叉轴起点排列的垂直布局。
    let mut layout = FlexLayout::column().with_align(AlignItems::Start);
    // 开启真实换行路径。
    layout.wrap = true;
    // 保留两个子项的自然主轴尺寸。
    layout.overflow_content = true;
    // 零交叉轴 frame 让输出完全由自然列组撑开。
    let output = layout.layout(Rect::new(6.0, 4.0, 0.0, 25.0), &children);
    // 首列子项保持完整的三十像素可见高度。
    assert_eq!(output.positions[0], Rect::new(6.0, 4.0, 20.0, 30.0));
    // 第二列仍从内容原点开始并保留自然尺寸。
    assert_eq!(output.positions[1], Rect::new(26.0, 4.0, 5.0, 10.0));
    // 总高度必须覆盖首项结束于三十像素处的可见矩形。
    assert_eq!(output.total_size, Size::new(25.0, 30.0));
}

// 验证负起始侧主轴外边距在正间距真实分行中仍参与行占用并保持轴转置。
#[test]
// 该契约独立于负 gap，避免把起始边距语义隐藏在负间距重叠中。
fn wrapped_overflow_negative_leading_margin_uses_margin_for_line_break() {
    // 构造首项并为水平主轴起点设置负六像素外边距。
    let mut first = LayoutChild::new(ComponentId::new(92), Size::new(20.0, 8.0));
    // 首项的水平起始边距为负值，尾侧仍保留正占用。
    first.margin = EdgeInsets::new(-6.0, 1.0, 2.0, 2.0);
    // 构造第二项并保留非对称的正主轴外边距。
    let mut second = LayoutChild::new(ComponentId::new(93), Size::new(12.0, 6.0));
    // 第二项的左右外边距用于确认换行后仍按完整外尺寸放置。
    second.margin = EdgeInsets::new(1.0, 2.0, 3.0, 1.0);
    // 保存水平参考输入。
    let row_children = [first, second];
    // 将水平输入逐项转置为垂直输入。
    let column_children = row_children.clone().map(|mut child| {
        // 读取待转置的物理外边距。
        let margin = child.margin;
        // 交换子项自然尺寸的宽高。
        child.measured_size = Size::new(child.measured_size.h, child.measured_size.w);
        // 按轴转置规则交换四侧外边距。
        child.margin = EdgeInsets::new(margin.top, margin.left, margin.bottom, margin.right);
        // 返回完成转置的子项。
        child
    });
    // 使用非零原点和零交叉轴构造水平 frame。
    let row_frame = Rect::new(10.0, 20.0, 30.0, 0.0);
    // 交换原点与尺寸得到垂直轴转置 frame。
    let column_frame = Rect::new(20.0, 10.0, 0.0, 30.0);
    // 构造带正 gap 的水平溢出换行布局。
    let mut row_layout = FlexLayout::row()
        .with_gap(4.0)
        .with_align(AlignItems::Start);
    // 开启真实多行求解路径。
    row_layout.wrap = true;
    // 保留自然尺寸，禁止弹性收缩掩盖换行边界。
    row_layout.overflow_content = true;
    // 执行水平参考布局。
    let row_output = row_layout.layout(row_frame, &row_children);
    // 复用全部参数并切换到垂直主轴。
    let mut column_layout = row_layout.clone();
    // 应用垂直主轴方向。
    column_layout.direction = FlexDirection::Column;
    // 执行垂直轴转置布局。
    let column_output = column_layout.layout(column_frame, &column_children);
    // 将水平结果的每个矩形转置为垂直参考值。
    let transposed_positions: Vec<_> = row_output
        .positions
        .iter()
        .copied()
        .map(|rect| Rect::new(rect.y, rect.x, rect.h, rect.w))
        .collect();
    // 负起始边距必须把首项移到内容原点之前而不改变自身尺寸。
    assert_eq!(row_output.positions[0], Rect::new(4.0, 21.0, 20.0, 8.0));
    // 第一项的外占用不足以容纳第二项与正 gap，因此第二项必须换行。
    assert_eq!(row_output.positions[1], Rect::new(11.0, 37.0, 12.0, 6.0));
    // 固有主轴账本保留每行十六像素的最大外占用。
    assert_eq!(row_output.total_size, Size::new(16.0, 24.0));
    // 垂直结果必须与水平结果保持完整轴转置。
    assert_eq!(column_output.positions, transposed_positions);
    // 垂直自然尺寸账本必须同步交换宽高。
    assert_eq!(column_output.total_size, Size::new(24.0, 16.0));
}
