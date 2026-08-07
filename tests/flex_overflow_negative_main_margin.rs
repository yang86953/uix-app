// 导入负主轴外边距契约使用的几何类型。
use uix::core::{ComponentId, EdgeInsets, Rect, Size};
// 导入负主轴外边距契约使用的布局入口。
use uix::ui::{AlignItems, FlexLayout, LayoutChild, LayoutEngine};

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
