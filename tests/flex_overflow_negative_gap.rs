// 导入负行间距契约使用的核心几何类型。
use uix::core::{ComponentId, Rect, Size};
// 导入负行间距契约使用的共享布局入口。
use uix::ui::{AlignItems, FlexLayout, LayoutChild, LayoutEngine};

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
