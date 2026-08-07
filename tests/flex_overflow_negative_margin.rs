// 导入负交叉轴外边距契约使用的几何类型。
use uix::core::{ComponentId, EdgeInsets, Rect, Size};
// 导入负交叉轴外边距契约使用的布局入口。
use uix::ui::{AlignItems, FlexLayout, LayoutChild, LayoutEngine};

// 验证水平多行布局不会用负下外边距裁掉子项可见末端。
#[test]
// 后续矮行不得让总高度停在首行可见矩形的末端之前。
fn wrapped_overflow_negative_bottom_margin_keeps_visible_height() {
    // 构造三十像素高、下外边距为负十像素的首行子项。
    let mut first = LayoutChild::new(ComponentId::new(84), Size::new(20.0, 30.0));
    // 负尾侧外边距只改变下一行推进量。
    first.margin = EdgeInsets::new(0.0, 0.0, 0.0, -10.0);
    // 构造落入第二行的五像素高子项。
    let second = LayoutChild::new(ComponentId::new(85), Size::new(20.0, 5.0));
    // 二十五像素主轴空间会把两个二十像素宽子项拆成两行。
    let children = [first, second];
    // 构造从交叉轴起点排列的水平布局。
    let mut layout = FlexLayout::row().with_align(AlignItems::Start);
    // 开启真实换行路径。
    layout.wrap = true;
    // 保留两个子项的自然主轴尺寸。
    layout.overflow_content = true;
    // 零交叉轴 frame 让输出完全由自然行组撑开。
    let output = layout.layout(Rect::new(4.0, 6.0, 25.0, 0.0), &children);
    // 首行子项保持完整的三十像素可见高度。
    assert_eq!(output.positions[0], Rect::new(4.0, 6.0, 20.0, 30.0));
    // 第二行按扣除负尾侧外边距后的二十像素行盒推进。
    assert_eq!(output.positions[1], Rect::new(4.0, 26.0, 20.0, 5.0));
    // 总高度仍须覆盖首行子项结束于三十像素处的可见矩形。
    assert_eq!(output.total_size, Size::new(20.0, 30.0));
}

// 验证垂直多列布局保持相同的轴转置可见范围语义。
#[test]
// 后续窄列不得让总宽度停在首列可见矩形的末端之前。
fn wrapped_overflow_negative_right_margin_keeps_visible_width() {
    // 构造三十像素宽、右外边距为负十像素的首列子项。
    let mut first = LayoutChild::new(ComponentId::new(86), Size::new(30.0, 20.0));
    // 负尾侧外边距只改变下一列推进量。
    first.margin = EdgeInsets::new(0.0, 0.0, -10.0, 0.0);
    // 构造落入第二列的五像素宽子项。
    let second = LayoutChild::new(ComponentId::new(87), Size::new(5.0, 20.0));
    // 二十五像素主轴空间会把两个二十像素高子项拆成两列。
    let children = [first, second];
    // 构造从交叉轴起点排列的垂直布局。
    let mut layout = FlexLayout::column().with_align(AlignItems::Start);
    // 开启真实换行路径。
    layout.wrap = true;
    // 保留两个子项的自然主轴尺寸。
    layout.overflow_content = true;
    // 零交叉轴 frame 让输出完全由自然列组撑开。
    let output = layout.layout(Rect::new(6.0, 4.0, 0.0, 25.0), &children);
    // 首列子项保持完整的三十像素可见宽度。
    assert_eq!(output.positions[0], Rect::new(6.0, 4.0, 30.0, 20.0));
    // 第二列按扣除负尾侧外边距后的二十像素列盒推进。
    assert_eq!(output.positions[1], Rect::new(26.0, 4.0, 5.0, 20.0));
    // 总宽度仍须覆盖首列子项结束于三十像素处的可见矩形。
    assert_eq!(output.total_size, Size::new(30.0, 20.0));
}
