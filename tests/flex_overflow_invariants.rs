// 导入 Flex 溢出契约使用的核心几何类型。
use uix::core::{ComponentId, EdgeInsets, Rect, Size};
// 导入 Flex 溢出契约使用的共享布局入口。
use uix::ui::{AlignItems, FlexDirection, FlexLayout, JustifyContent, LayoutChild, LayoutEngine};
// 验证溢出 Flex 在交叉轴对齐时先扣除两侧 margin。
#[test]
// 同时覆盖居中与末端对齐，防止非对称 margin 重复偏移。
fn overflow_flex_cross_alignment_respects_asymmetric_margins() {
    // 构造十乘十且带二像素上边距和八像素下边距的子项。
    let mut child = LayoutChild::new(ComponentId::new(33), Size::new(10.0, 10.0));
    // 非对称 margin 将交叉轴可用区从四十缩减到三十像素。
    child.margin = EdgeInsets::new(0.0, 2.0, 0.0, 8.0);
    // 构造保留自然主轴尺寸的居中布局。
    let mut centered_layout = FlexLayout::row().with_align(AlignItems::Center);
    // 开启溢出内容路径。
    centered_layout.overflow_content = true;
    // 在四十像素高的内容区执行居中布局。
    let centered = centered_layout.layout(Rect::new(0.0, 0.0, 100.0, 40.0), &[child.clone()]);
    // 子项应在扣除两侧 margin 后的三十像素区域内居中。
    assert_eq!(centered.positions[0], Rect::new(0.0, 12.0, 10.0, 10.0));
    // 构造保留自然主轴尺寸的末端布局。
    let mut ended_layout = FlexLayout::row().with_align(AlignItems::End);
    // 开启溢出内容路径。
    ended_layout.overflow_content = true;
    // 在同一内容区执行末端布局。
    let ended = ended_layout.layout(Rect::new(0.0, 0.0, 100.0, 40.0), &[child]);
    // 子项底边应停在八像素下边距之前。
    assert_eq!(ended.positions[0], Rect::new(0.0, 22.0, 10.0, 10.0));
}

// 验证溢出 Flex 的反向主轴与标准 Flex 使用相同镜像语义。
#[test]
// 两个自然宽度子项应从容器右端向左排列。
fn overflow_flex_reverse_starts_from_the_main_end() {
    // 构造两个十乘八的自然尺寸子项。
    let children = vec![
        LayoutChild::new(ComponentId::new(34), Size::new(10.0, 8.0)),
        LayoutChild::new(ComponentId::new(35), Size::new(10.0, 8.0)),
    ];
    // 构造五像素间距的反向水平布局。
    let mut layout = FlexLayout::row()
        .with_direction(FlexDirection::RowReverse)
        .with_gap(5.0)
        .with_justify(JustifyContent::Start)
        .with_align(AlignItems::Start);
    // 开启溢出内容路径以保留自然主轴尺寸。
    layout.overflow_content = true;
    // 在一百像素宽的内容区执行反向布局。
    let output = layout.layout(Rect::new(0.0, 0.0, 100.0, 20.0), &children);
    // 首项应贴住反向主轴起点，也就是容器右端。
    assert_eq!(output.positions[0], Rect::new(90.0, 0.0, 10.0, 8.0));
    // 次项应位于首项左侧并保留五像素间距。
    assert_eq!(output.positions[1], Rect::new(75.0, 0.0, 10.0, 8.0));
}

// 验证溢出 Flex 保留自然尺寸时仍遵守主轴分布配置。
#[test]
// 同时覆盖整体居中与剩余空间均分到项目间隙。
fn overflow_flex_honors_justify_content() {
    // 构造两个十乘八的自然尺寸子项。
    let children = vec![
        LayoutChild::new(ComponentId::new(36), Size::new(10.0, 8.0)),
        LayoutChild::new(ComponentId::new(37), Size::new(10.0, 8.0)),
    ];
    // 构造五像素间距的居中溢出布局。
    let mut centered_layout = FlexLayout::row()
        .with_gap(5.0)
        .with_justify(JustifyContent::Center)
        .with_align(AlignItems::Start);
    // 开启溢出内容路径。
    centered_layout.overflow_content = true;
    // 在一百像素宽的内容区执行居中布局。
    let centered = centered_layout.layout(Rect::new(0.0, 0.0, 100.0, 20.0), &children);
    // 二十五像素自然内容应整体位于容器中央。
    assert_eq!(centered.positions[0], Rect::new(37.5, 0.0, 10.0, 8.0));
    // 第二项应保留五像素基础间距。
    assert_eq!(centered.positions[1], Rect::new(52.5, 0.0, 10.0, 8.0));
    // 构造把剩余空间分配到项目间隙的溢出布局。
    let mut distributed_layout = FlexLayout::row()
        .with_gap(5.0)
        .with_justify(JustifyContent::SpaceBetween)
        .with_align(AlignItems::Start);
    // 开启溢出内容路径。
    distributed_layout.overflow_content = true;
    // 在同一内容区执行两端分布布局。
    let distributed = distributed_layout.layout(Rect::new(0.0, 0.0, 100.0, 20.0), &children);
    // 首项应贴住主轴起点。
    assert_eq!(distributed.positions[0], Rect::new(0.0, 0.0, 10.0, 8.0));
    // 次项应贴住主轴末端。
    assert_eq!(distributed.positions[1], Rect::new(90.0, 0.0, 10.0, 8.0));
}

// 验证固有主轴的反向溢出布局按自然内容长度镜像。
#[test]
// 零高 ColumnReverse bootstrap 不得把全部子项镜像到负坐标。
fn overflow_flex_intrinsic_reverse_uses_natural_main_extent() {
    // 构造两个十乘十的自然尺寸子项。
    let children = vec![
        LayoutChild::new(ComponentId::new(38), Size::new(10.0, 10.0)),
        LayoutChild::new(ComponentId::new(39), Size::new(10.0, 10.0)),
    ];
    // 构造五像素间距的反向垂直布局。
    let mut layout = FlexLayout::column()
        .with_direction(FlexDirection::ColumnReverse)
        .with_gap(5.0)
        .with_align(AlignItems::Start);
    // 开启自然尺寸溢出路径。
    layout.overflow_content = true;
    // 声明主轴无显式尺寸，应由二十五像素自然内容撑开。
    layout.intrinsic_main = true;
    // 使用带非零原点的零高 frame 覆盖 bootstrap 镜像。
    let output = layout.layout(Rect::new(4.0, 6.0, 20.0, 0.0), &children);
    // 首项应位于自然内容主轴末端。
    assert_eq!(output.positions[0], Rect::new(4.0, 21.0, 10.0, 10.0));
    // 次项应位于自然内容主轴起点。
    assert_eq!(output.positions[1], Rect::new(4.0, 6.0, 10.0, 10.0));
    // 固有总高必须包含两个子项与中间 gap。
    assert_eq!(output.total_size, Size::new(20.0, 25.0));
}

// 验证固有主轴在获得实际宽度后仍以该宽度完成反向镜像。
#[test]
// 居中布局不得先按容器定位、再按更短的自然内容长度镜像到负坐标。
fn overflow_flex_intrinsic_reverse_with_allocated_main_uses_container_extent() {
    // 构造两个十乘八的自然尺寸子项。
    let children = vec![
        // 第一个子项用于验证反向后的右侧位置。
        LayoutChild::new(ComponentId::new(41), Size::new(10.0, 8.0)),
        // 第二个子项用于验证反向后的左侧位置。
        LayoutChild::new(ComponentId::new(42), Size::new(10.0, 8.0)),
    ];
    // 构造带五像素间距的水平反向布局。
    let mut layout = FlexLayout::row()
        // 切换为水平反向主轴。
        .with_direction(FlexDirection::RowReverse)
        // 保留两个子项之间的固定间距。
        .with_gap(5.0)
        // 让自然内容先按实际容器宽度居中。
        .with_justify(JustifyContent::Center)
        // 交叉轴从顶部开始，隔离主轴镜像结果。
        .with_align(AlignItems::Start);
    // 开启自然尺寸溢出路径。
    layout.overflow_content = true;
    // 声明主轴固有尺寸仍由内容决定。
    layout.intrinsic_main = true;
    // 为布局提供带非零原点的一百像素实际宽度。
    let output = layout.layout(Rect::new(4.0, 6.0, 100.0, 20.0), &children);
    // 第一项应位于居中内容的右侧，而不是被镜像到负坐标。
    assert_eq!(output.positions[0], Rect::new(56.5, 6.0, 10.0, 8.0));
    // 第二项应位于第一项左侧并保留五像素间距。
    assert_eq!(output.positions[1], Rect::new(41.5, 6.0, 10.0, 8.0));
    // 固有尺寸账本仍只记录两个子项与基础间距。
    assert_eq!(output.total_size, Size::new(25.0, 20.0));
}

// 验证标准 Flex 的固有主轴同样使用已分配 frame 完成反向镜像。
#[test]
// 非溢出路径不得因固有尺寸账本较短而破坏实际容器内的居中位置。
fn flex_intrinsic_reverse_with_allocated_main_uses_container_extent() {
    // 构造两个十乘八的自然尺寸子项。
    let children = vec![
        // 第一项用于验证反向后的右侧位置。
        LayoutChild::new(ComponentId::new(46), Size::new(10.0, 8.0)),
        // 第二项用于验证反向后的左侧位置。
        LayoutChild::new(ComponentId::new(47), Size::new(10.0, 8.0)),
    ];
    // 构造标准水平反向布局。
    let mut layout = FlexLayout::row()
        // 切换为水平反向主轴。
        .with_direction(FlexDirection::RowReverse)
        // 保留两个子项之间的五像素间距。
        .with_gap(5.0)
        // 让自然内容按实际容器宽度居中。
        .with_justify(JustifyContent::Center)
        // 交叉轴从顶部开始。
        .with_align(AlignItems::Start);
    // 声明主轴固有尺寸由内容决定，但不启用溢出专用路径。
    layout.intrinsic_main = true;
    // 为标准求解器提供一百像素实际宽度。
    let output = layout.layout(Rect::new(4.0, 6.0, 100.0, 20.0), &children);
    // 第一项应位于居中内容的右侧。
    assert_eq!(output.positions[0], Rect::new(56.5, 6.0, 10.0, 8.0));
    // 第二项应位于第一项左侧并保留间距。
    assert_eq!(output.positions[1], Rect::new(41.5, 6.0, 10.0, 8.0));
    // 固有尺寸继续记录自然内容宽度。
    assert_eq!(output.total_size, Size::new(25.0, 20.0));
}

// 验证换行固有主轴在非零 frame 内逐行保持反向分布。
#[test]
// 不同行宽不得因按最大自然行长镜像而偏离各自的容器内对齐位置。
fn wrapped_overflow_flex_reverse_uses_container_extent() {
    // 构造三个二十乘八的自然尺寸子项。
    let children = vec![
        // 第一项与第二项共同占据首行。
        LayoutChild::new(ComponentId::new(43), Size::new(20.0, 8.0)),
        // 第二项验证首行反向顺序。
        LayoutChild::new(ComponentId::new(44), Size::new(20.0, 8.0)),
        // 第三项单独换到第二行并保持居中。
        LayoutChild::new(ComponentId::new(45), Size::new(20.0, 8.0)),
    ];
    // 构造带五像素间距的水平反向布局。
    let mut layout = FlexLayout::row()
        // 切换为水平反向主轴。
        .with_direction(FlexDirection::RowReverse)
        // 同时作为行内和行间基础间距。
        .with_gap(5.0)
        // 让每一行在实际容器主轴内居中。
        .with_justify(JustifyContent::Center)
        // 让各行从交叉轴起点依次排列。
        .with_align(AlignItems::Start);
    // 开启多行换行路径。
    layout.wrap = true;
    // 开启自然尺寸溢出路径。
    layout.overflow_content = true;
    // 五十像素主轴使前两项同处首行、第三项换到次行。
    let output = layout.layout(Rect::new(0.0, 0.0, 50.0, 30.0), &children);
    // 首项应位于首行居中内容的右侧。
    assert_eq!(output.positions[0], Rect::new(27.5, 0.0, 20.0, 8.0));
    // 次项应位于首行左侧并保留五像素间距。
    assert_eq!(output.positions[1], Rect::new(2.5, 0.0, 20.0, 8.0));
    // 单独换行的第三项应继续位于实际容器中央。
    assert_eq!(output.positions[2], Rect::new(15.0, 13.0, 20.0, 8.0));
    // 固有主轴尺寸取首行自然宽，交叉轴沿用实际容器高度。
    assert_eq!(output.total_size, Size::new(45.0, 30.0));
}

// 验证溢出 Flex 的零交叉轴 bootstrap 保留自然外尺寸。
#[test]
// 默认 Stretch 不得把未约束交叉轴压成零。
fn overflow_flex_bootstrap_cross_axis_uses_natural_outer_size() {
    // 构造二十乘十且四侧 margin 总计十像素的子项。
    let mut child = LayoutChild::new(ComponentId::new(40), Size::new(20.0, 10.0));
    // 左二、上三、右八、下七构成三十乘二十的自然外尺寸。
    child.margin = EdgeInsets::new(2.0, 3.0, 8.0, 7.0);
    // 构造默认 Stretch 的垂直自然尺寸布局。
    let mut layout = FlexLayout::column();
    // 开启自然尺寸溢出路径。
    layout.overflow_content = true;
    // 声明垂直主轴无显式尺寸。
    layout.intrinsic_main = true;
    // 使用双轴为零且带非零原点的 bootstrap frame。
    let output = layout.layout(Rect::new(4.0, 6.0, 0.0, 0.0), &[child]);
    // 子项应保留自然尺寸并从左上 margin 后开始。
    assert_eq!(output.positions[0], Rect::new(6.0, 9.0, 20.0, 10.0));
    // 总尺寸必须包含交叉轴与主轴两侧 margin。
    assert_eq!(output.total_size, Size::new(30.0, 20.0));
}

// 验证单行溢出内容在主轴空间不足时仍执行居中分布。
#[test]
// 负剩余空间必须转化为向主轴起点外溢的半量偏移。
fn overflow_flex_centers_negative_main_space() {
    // 构造宽八十像素的单个自然尺寸子项。
    let child = LayoutChild::new(ComponentId::new(52), Size::new(80.0, 8.0));
    // 构造主轴居中的水平布局。
    let mut layout = FlexLayout::row()
        // 声明内容在主轴居中。
        .with_justify(JustifyContent::Center)
        // 固定交叉轴从起点对齐以隔离主轴结果。
        .with_align(AlignItems::Start);
    // 开启自然尺寸溢出路径。
    layout.overflow_content = true;
    // 在带非零原点且仅四十像素宽的容器内布局。
    let output = layout.layout(Rect::new(10.0, 6.0, 40.0, 20.0), &[child]);
    // 八十像素内容应在四十像素容器内居中并向两侧各溢出二十像素。
    assert_eq!(output.positions[0], Rect::new(-10.0, 6.0, 80.0, 8.0));
    // 内容尺寸账本继续保留自然主轴长度。
    assert_eq!(output.total_size, Size::new(80.0, 20.0));
}

// 验证标准固有主轴在空间不足时仍执行末端分布。
#[test]
// End 必须把全部负剩余空间转化为向主轴起点外溢的偏移。
fn intrinsic_flex_ends_negative_main_space() {
    // 构造宽八十像素的单个自然尺寸子项。
    let child = LayoutChild::new(ComponentId::new(53), Size::new(80.0, 8.0));
    // 构造主轴末端对齐的标准水平布局。
    let mut layout = FlexLayout::row()
        // 声明内容贴住主轴末端。
        .with_justify(JustifyContent::End)
        // 固定交叉轴从起点对齐以隔离主轴结果。
        .with_align(AlignItems::Start);
    // 声明主轴尺寸由自然内容决定并跳过压缩。
    layout.intrinsic_main = true;
    // 在带非零原点且仅四十像素宽的实际容器内布局。
    let output = layout.layout(Rect::new(10.0, 6.0, 40.0, 20.0), &[child]);
    // 子项末端应贴住容器末端并向起点外溢四十像素。
    assert_eq!(output.positions[0], Rect::new(-30.0, 6.0, 80.0, 8.0));
    // 固有尺寸账本继续记录八十像素自然宽度。
    assert_eq!(output.total_size, Size::new(80.0, 20.0));
}

// 验证换行溢出中的超宽单项仍执行逐行居中分布。
#[test]
// 单项超过换行上限时不得把负剩余空间钳零并退化为 Start。
fn wrapped_overflow_flex_centers_oversized_line() {
    // 构造宽八十像素且无法继续拆分的单个自然尺寸子项。
    let child = LayoutChild::new(ComponentId::new(54), Size::new(80.0, 8.0));
    // 构造逐行居中的水平布局。
    let mut layout = FlexLayout::row()
        // 声明每一行的内容在主轴居中。
        .with_justify(JustifyContent::Center)
        // 固定交叉轴从起点排列以隔离逐行主轴结果。
        .with_align(AlignItems::Start);
    // 开启换行求解路径。
    layout.wrap = true;
    // 开启自然尺寸溢出路径并冻结增长与压缩。
    layout.overflow_content = true;
    // 在带非零原点且仅四十像素宽的容器内布局。
    let output = layout.layout(Rect::new(10.0, 6.0, 40.0, 20.0), &[child]);
    // 超宽行应相对实际容器居中并向两侧各溢出二十像素。
    assert_eq!(output.positions[0], Rect::new(-10.0, 6.0, 80.0, 8.0));
    // 换行固有尺寸账本继续记录最长自然行宽。
    assert_eq!(output.total_size, Size::new(80.0, 20.0));
}

// 验证启用换行但实际只有一行时保持普通 Flex 的交叉轴 Stretch 语义。
#[test]
// wrap 开关本身不得让默认 Stretch 子项退回自然高度。
fn wrapped_single_line_stretches_across_the_container() {
    // 构造带非对称交叉轴 margin 的自然尺寸子项。
    let mut child = LayoutChild::new(ComponentId::new(55), Size::new(20.0, 10.0));
    // 上二下八的 margin 为四十像素容器留下三十像素可拉伸高度。
    child.margin = EdgeInsets::new(0.0, 2.0, 0.0, 8.0);
    // 使用默认 Stretch 构造水平 Flex。
    let mut layout = FlexLayout::row();
    // 开启换行求解路径，但单项不会真正换行。
    layout.wrap = true;
    // 在已知主轴与交叉轴尺寸的容器内执行布局。
    let output = layout.layout(Rect::new(4.0, 6.0, 100.0, 40.0), &[child]);
    // 子项应扣除上下 margin 后填满整条单行交叉轴。
    assert_eq!(output.positions[0], Rect::new(4.0, 8.0, 20.0, 30.0));
    // 总尺寸继续等于父级分配的实际容器尺寸。
    assert_eq!(output.total_size, Size::new(100.0, 40.0));
}

// 验证启用换行但实际仍为单行时复用非换行路径的真实交叉轴行盒。
#[test]
// 同时覆盖较小容器内的 Stretch 收缩与逐项 End 对齐。
fn wrapped_unbroken_line_matches_single_line_cross_alignment() {
    // 构造自然外高度超过容器、并要求在行盒内 Stretch 的首项。
    let mut stretched = LayoutChild::new(ComponentId::new(64), Size::new(20.0, 10.0));
    // 上下各两像素 margin 让八像素容器只留下四像素可拉伸高度。
    stretched.margin = EdgeInsets::new(0.0, 2.0, 0.0, 2.0);
    // 首项显式覆盖容器级 Start，以验证逐项 Stretch 仍使用真实行盒。
    stretched.align_self = Some(AlignItems::Stretch);
    // 构造在同一真实行盒末端对齐的较矮次项。
    let mut ended = LayoutChild::new(ComponentId::new(65), Size::new(20.0, 3.0));
    // 次项显式覆盖为 End，以验证定位不再局限于自然行高。
    ended.align_self = Some(AlignItems::End);
    // 两个二十像素宽子项会稳定留在同一百像素主轴行内。
    let children = [stretched, ended];
    // 关闭换行建立同一公开布局契约的单行基准。
    let baseline = FlexLayout::row()
        // 容器级 Start 隔离逐项 align_self 的结果。
        .with_align(AlignItems::Start)
        // 在带非零原点的八像素交叉轴内执行基准布局。
        .layout(Rect::new(4.0, 6.0, 100.0, 8.0), &children);
    // 基准首项应扣除上下 margin 后收缩为四像素高。
    assert_eq!(baseline.positions[0], Rect::new(4.0, 8.0, 20.0, 4.0));
    // 基准次项应贴住八像素真实行盒的末端。
    assert_eq!(baseline.positions[1], Rect::new(24.0, 11.0, 20.0, 3.0));
    // 构造除 wrap 开关外与基准完全相同的布局。
    let mut wrapped = FlexLayout::row().with_align(AlignItems::Start);
    // 开启换行求解路径，但两个子项仍不会实际分行。
    wrapped.wrap = true;
    // 执行受测的未分行 wrapped 布局。
    let output = wrapped.layout(Rect::new(4.0, 6.0, 100.0, 8.0), &children);
    // wrap 开关不得改变未分行子项的最终矩形。
    assert_eq!(output.positions, baseline.positions);
    // wrap 开关也不得改变未分行布局的内容尺寸账本。
    assert_eq!(output.total_size, baseline.total_size);
}

// 验证多行默认 Stretch 把交叉轴剩余空间均分到各行。
#[test]
// 逐项 align_self 只覆盖项内对齐，不得阻止其他行消费扩展后的行高。
fn wrapped_lines_stretch_cross_space_and_preserve_align_self() {
    // 构造首行自然尺寸子项。
    let mut first = LayoutChild::new(ComponentId::new(56), Size::new(60.0, 10.0));
    // 首项覆盖为 Start，验证扩展行内仍保留自然高度。
    first.align_self = Some(AlignItems::Start);
    // 构造第二行沿用容器默认 Stretch 的自然尺寸子项。
    let second = LayoutChild::new(ComponentId::new(57), Size::new(60.0, 10.0));
    // 使用四像素行间距构造默认 Stretch 的水平 Flex。
    let mut layout = FlexLayout::row().with_gap(4.0);
    // 开启换行，使两个六十像素子项在百像素主轴内分成两行。
    layout.wrap = true;
    // 四十像素交叉轴扣除行间距后应为两行各分配十八像素。
    let output = layout.layout(Rect::new(4.0, 6.0, 100.0, 40.0), &[first, second]);
    // 首项按 align_self Start 保留十像素自然高度并位于扩展首行起点。
    assert_eq!(output.positions[0], Rect::new(4.0, 6.0, 60.0, 10.0));
    // 第二项应从二十二像素行起点开始并填满十八像素行高。
    assert_eq!(output.positions[1], Rect::new(4.0, 28.0, 60.0, 18.0));
    // 多行账本必须完整占用四十像素实际交叉轴。
    assert_eq!(output.total_size, Size::new(100.0, 40.0));
}

// 验证零主轴尺寸子项仍会占据换行收集器中的一个项目位置。
#[test]
// 行内是否已有项目必须由索引判断，不能由当前行占用是否大于零判断。
fn wrapped_zero_sized_item_still_forces_the_next_item_to_wrap() {
    // 构造主轴宽度为零但交叉轴仍有高度的首项。
    let first = LayoutChild::new(ComponentId::new(58), Size::new(0.0, 8.0));
    // 构造恰好填满四十像素主轴的第二项。
    let second = LayoutChild::new(ComponentId::new(59), Size::new(40.0, 8.0));
    // 使用五像素间距并从两个轴的起点对齐。
    let mut layout = FlexLayout::row()
        // 间距会使两个项目的行占用超过四十像素。
        .with_gap(5.0)
        // 固定主轴从起点排列以直接观察换行结果。
        .with_justify(JustifyContent::Start)
        // 固定交叉轴从起点排列以隔离行高。
        .with_align(AlignItems::Start);
    // 开启换行求解路径。
    layout.wrap = true;
    // 保留自然主轴尺寸，避免第二项被压缩而掩盖分行错误。
    layout.overflow_content = true;
    // 在四十像素主轴与三十像素交叉轴的容器内执行布局。
    let output = layout.layout(Rect::new(4.0, 6.0, 40.0, 30.0), &[first, second]);
    // 零宽首项仍应留在第一行起点。
    assert_eq!(output.positions[0], Rect::new(4.0, 6.0, 0.0, 8.0));
    // 第二项必须在八像素行高与五像素行距之后另起一行。
    assert_eq!(output.positions[1], Rect::new(4.0, 19.0, 40.0, 8.0));
    // 固有主轴账本应记录最长的四十像素自然行宽。
    assert_eq!(output.total_size, Size::new(40.0, 30.0));
}

// 验证换行行组在交叉轴溢出时仍能执行末端对齐。
#[test]
// End 必须保留负剩余空间，让最后一行末端贴住实际容器末端。
fn wrapped_overflow_flex_end_aligns_negative_cross_space() {
    // 构造两个会各自占据一行的三十乘十子项。
    let children = vec![
        // 首项用于观察行组向交叉轴起点外溢后的负坐标。
        LayoutChild::new(ComponentId::new(60), Size::new(30.0, 10.0)),
        // 次项用于验证行组末端仍贴住容器末端。
        LayoutChild::new(ComponentId::new(61), Size::new(30.0, 10.0)),
    ];
    // 构造交叉轴末端对齐的水平 Flex。
    let mut layout = FlexLayout::row()
        // 五像素间距同时形成两行之间的固定行距。
        .with_gap(5.0)
        // 每行主轴从起点排列，隔离交叉轴结果。
        .with_justify(JustifyContent::Start)
        // 整个自然行组贴住实际交叉轴末端。
        .with_align(AlignItems::End);
    // 开启换行，使两个三十像素子项在五十像素主轴内分成两行。
    layout.wrap = true;
    // 保留自然尺寸，明确覆盖换行溢出路径。
    layout.overflow_content = true;
    // 十五像素交叉轴小于两行和行距组成的二十五像素自然高度。
    let output = layout.layout(Rect::new(4.0, 6.0, 50.0, 15.0), &children);
    // 首行应向容器顶部之外溢出十像素。
    assert_eq!(output.positions[0], Rect::new(4.0, -4.0, 30.0, 10.0));
    // 第二行底边应精确贴住二十一像素的容器底边。
    assert_eq!(output.positions[1], Rect::new(4.0, 11.0, 30.0, 10.0));
    // 内容尺寸账本保留三十像素最长自然行与二十五像素自然交叉轴高度。
    assert_eq!(output.total_size, Size::new(30.0, 25.0));
}

// 验证零交叉轴 bootstrap 不会提前应用换行行组对齐偏移。
#[test]
// Center 与 End 都应从自然起点开始，等待父级下一轮分配实际尺寸。
fn wrapped_cross_bootstrap_starts_from_natural_origin() {
    // 构造两个会在五十像素主轴内各自占据一行的子项。
    let children = vec![
        // 首项提供十像素自然行高。
        LayoutChild::new(ComponentId::new(62), Size::new(30.0, 10.0)),
        // 次项提供第二个十像素自然行高。
        LayoutChild::new(ComponentId::new(63), Size::new(30.0, 10.0)),
    ];
    // 枚举会在实际负剩余空间下移动行组的两种对齐。
    let alignments = [AlignItems::Center, AlignItems::End];
    // 逐种验证 bootstrap 阶段都不生成负坐标。
    for alignment in alignments {
        // 构造带五像素固定行距的水平 Flex。
        let mut layout = FlexLayout::row()
            // 五像素间距使自然行组总高达到二十五像素。
            .with_gap(5.0)
            // 每行主轴从起点排列，隔离交叉轴结果。
            .with_justify(JustifyContent::Start)
            // 应用当前受测的交叉轴行组对齐。
            .with_align(alignment);
        // 开启换行，使两个三十像素子项分成两行。
        layout.wrap = true;
        // 保留自然内容尺寸以覆盖溢出布局入口。
        layout.overflow_content = true;
        // 使用非零原点和零交叉轴高度执行首次 bootstrap。
        let output = layout.layout(Rect::new(4.0, 6.0, 50.0, 0.0), &children);
        // 首行必须从父级交叉轴自然起点开始，不得向上产生负偏移。
        assert_eq!(output.positions[0], Rect::new(4.0, 6.0, 30.0, 10.0));
        // 次行在首行与固定行距之后继续自然排列。
        assert_eq!(output.positions[1], Rect::new(4.0, 21.0, 30.0, 10.0));
        // 输出账本由最长自然行与两行自然总高撑开。
        assert_eq!(output.total_size, Size::new(30.0, 25.0));
    }
}

// 验证空 Flex 的固有主轴由零个子项收敛为零，同时保留父级分配的交叉轴。
#[test]
// 水平、垂直和溢出入口必须共享相同的空内容自然尺寸语义。
fn empty_intrinsic_flex_collapses_only_the_main_axis() {
    // 构造具有非零原点与两条有限轴的父级内容区。
    let content_rect = Rect::new(4.0, 6.0, 100.0, 20.0);
    // 构造水平固有主轴布局。
    let mut row = FlexLayout::row();
    // 声明水平主轴由自然内容撑开。
    row.intrinsic_main = true;
    // 对空子集执行水平固有尺寸布局。
    let row_output = row.layout(content_rect, &[]);
    // 空子集不应生成任何位置。
    assert!(row_output.positions.is_empty());
    // 水平主轴应收敛为零，交叉轴继续占用父级分配高度。
    assert_eq!(row_output.total_size, Size::new(0.0, 20.0));

    // 构造垂直固有主轴布局。
    let mut column = FlexLayout::column();
    // 声明垂直主轴由自然内容撑开。
    column.intrinsic_main = true;
    // 对空子集执行垂直固有尺寸布局。
    let column_output = column.layout(content_rect, &[]);
    // 垂直主轴应收敛为零，交叉轴继续占用父级分配宽度。
    assert_eq!(column_output.total_size, Size::new(100.0, 0.0));

    // 构造以自然主轴记账的水平溢出布局。
    let mut overflow = FlexLayout::row();
    // 开启溢出内容路径，使其采用与固有主轴相同的自然尺寸语义。
    overflow.overflow_content = true;
    // 对空子集执行溢出布局。
    let overflow_output = overflow.layout(content_rect, &[]);
    // 空溢出内容也应只折叠主轴而保留分配的交叉轴。
    assert_eq!(overflow_output.total_size, Size::new(0.0, 20.0));
}
