// 导入布局契约测试使用的核心几何类型。
use uix::core::{ComponentId, EdgeInsets, Rect, Size};
// 导入三个共享布局入口及其公开输出类型。
use uix::ui::{
    AlignItems, BoxModel, FlexDirection, FlexLayout, GridLayout, GridTrack, JustifyContent,
    LayoutChild, LayoutEngine, LayoutOutput,
};

// 断言实际布局坐标不是测量阶段的无界哨兵或非有限值。
fn assert_actual_coordinate(value: f32, label: &str) {
    // 实际坐标必须是有限浮点值。
    assert!(value.is_finite(), "{label} must be finite, got {value}");
    // 实际坐标不得保留 f32::MAX 无界哨兵。
    assert!(
        value.abs() < f32::MAX,
        "{label} must not contain the unbounded sentinel"
    );
}

// 断言实际布局尺寸有限且非负。
fn assert_actual_extent(value: f32, label: &str) {
    // 尺寸先满足实际坐标的有限性要求。
    assert_actual_coordinate(value, label);
    // 实际尺寸不得为负数。
    assert!(value >= 0.0, "{label} must be nonnegative, got {value}");
}

// 断言共享布局输出中的总尺寸和全部子 frame 满足架构不变量。
fn assert_finite_layout_output(output: &LayoutOutput) {
    // 核对布局总宽度。
    assert_actual_extent(output.total_size.w, "total width");
    // 核对布局总高度。
    assert_actual_extent(output.total_size.h, "total height");
    // 逐个核对最终子 frame。
    for (index, frame) in output.positions.iter().enumerate() {
        // 核对子 frame 的横坐标。
        assert_actual_coordinate(frame.x, &format!("frame {index} x"));
        // 核对子 frame 的纵坐标。
        assert_actual_coordinate(frame.y, &format!("frame {index} y"));
        // 核对子 frame 的宽度。
        assert_actual_extent(frame.w, &format!("frame {index} width"));
        // 核对子 frame 的高度。
        assert_actual_extent(frame.h, &format!("frame {index} height"));
    }
}

// 验证盒模型忽略外边距，并拒绝把无界值写入内容 frame。
#[test]
// 覆盖 border-box 到 content-box 的共享收敛边界。
fn box_model_content_rect_is_margin_free_and_finite() {
    // 构造包含外边距、边框与内边距的标准盒模型。
    let model = BoxModel {
        // 外边距应由父布局消费，不参与当前 content rect 收缩。
        margin: EdgeInsets::uniform(40.0),
        // 每边保留两像素边框。
        border_width: EdgeInsets::uniform(2.0),
        // 每边保留四像素内边距。
        padding: EdgeInsets::uniform(4.0),
    };
    // 从稳定 border-box 计算内容区域。
    let content = model.content_rect(Rect::new(10.0, 20.0, 100.0, 80.0));
    // 横坐标只扣边框与内边距，不重复扣外边距。
    assert_eq!(content.x, 16.0);
    // 纵坐标只扣边框与内边距，不重复扣外边距。
    assert_eq!(content.y, 26.0);
    // 宽度从 border-box 两侧各扣六像素。
    assert_eq!(content.w, 88.0);
    // 高度从 border-box 两侧各扣六像素。
    assert_eq!(content.h, 68.0);

    // 构造来自无界约束或非法样式值的病理盒模型。
    let pathological = BoxModel {
        // 外边距仍不参与内容区域计算。
        margin: EdgeInsets::uniform(f32::INFINITY),
        // 非有限边框不得传播到实际 frame。
        border_width: EdgeInsets::new(f32::INFINITY, f32::NAN, f32::MAX, f32::NEG_INFINITY),
        // 非有限内边距不得传播到实际 frame。
        padding: EdgeInsets::new(f32::MAX, f32::INFINITY, f32::NAN, f32::NEG_INFINITY),
    };
    // 使用测量阶段的无界哨兵模拟未归一输入。
    let content = pathological.content_rect(Rect::new(
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MAX,
        f32::INFINITY,
    ));
    // 病理输入的内容横坐标仍须有限。
    assert_actual_coordinate(content.x, "content x");
    // 病理输入的内容纵坐标仍须有限。
    assert_actual_coordinate(content.y, "content y");
    // 病理输入的内容宽度仍须有限非负。
    assert_actual_extent(content.w, "content width");
    // 病理输入的内容高度仍须有限非负。
    assert_actual_extent(content.h, "content height");
}

// 验证空 Flex/Grid 不会把无界父尺寸直接写入布局输出。
#[test]
// 覆盖共享布局引擎的空子集快速返回分支。
fn empty_layouts_do_not_materialize_unbounded_sentinels() {
    // 构造包含无界宽高与非有限原点的父内容 frame。
    let unbounded = Rect::new(f32::INFINITY, f32::NEG_INFINITY, f32::MAX, f32::INFINITY);
    // 执行空 Flex 布局。
    let flex = FlexLayout::row().layout(unbounded, &[]);
    // 空 Flex 输出必须保持实际几何不变量。
    assert_finite_layout_output(&flex);
    // 执行空 Grid 布局。
    let grid = GridLayout::new()
        .with_columns(vec![GridTrack::Auto])
        .layout(unbounded, &[]);
    // 空 Grid 输出必须保持实际几何不变量。
    assert_finite_layout_output(&grid);
}

// 验证 Flex 对病理尺寸、弹性因子、间距和外边距进行最终收敛。
#[test]
// 覆盖标准 Flex 求解和输出 frame 的有限化边界。
fn flex_layout_normalizes_pathological_inputs() {
    // 构造第一个包含非有限尺寸与弹性因子的子项。
    let mut first = LayoutChild::new(ComponentId::new(1), Size::new(f32::MAX, f32::INFINITY));
    // 非有限 grow 不得进入实际 frame 算术。
    first.flex_grow = f32::INFINITY;
    // NaN shrink 不得进入实际 frame 算术。
    first.flex_shrink = f32::NAN;
    // 非有限外边距不得进入实际 frame 算术。
    first.margin = EdgeInsets::new(f32::INFINITY, f32::NAN, f32::MAX, f32::NEG_INFINITY);
    // 构造第二个包含负尺寸与有限基线尺寸的子项。
    let mut second = LayoutChild::new(ComponentId::new(2), Size::new(-10.0, 24.0));
    // 使用有限 grow 继续覆盖正常分配分支。
    second.flex_grow = 1.0;
    // 使用非有限外边距覆盖累加溢出分支。
    second.margin = EdgeInsets::uniform(f32::MAX);
    // 运行带非有限 gap 的水平 Flex 布局。
    let output = FlexLayout::row()
        .with_gap(f32::INFINITY)
        .with_align(AlignItems::Center)
        .layout(
            Rect::new(4.0, 8.0, f32::MAX, f32::INFINITY),
            &[first, second],
        );
    // 全部最终几何必须有限且尺寸非负。
    assert_finite_layout_output(&output);
    // 求解器必须为每个输入子项保留一个输出 frame。
    assert_eq!(output.positions.len(), 2);
}

// 验证 Grid 对病理 track、间距、尺寸和外边距进行最终收敛。
#[test]
// 覆盖 Grid track 解析、span 与输出 frame 的有限化边界。
fn grid_layout_normalizes_pathological_inputs() {
    // 构造显式放置且跨两列的病理子项。
    let mut first = LayoutChild::new(ComponentId::new(1), Size::new(f32::INFINITY, f32::MAX));
    // 把子项放入第一个显式单元格。
    first.grid_cell = Some(0);
    // 覆盖多列 span 路径。
    first.grid_column_span = 2;
    // 非有限外边距不得传播到结果。
    first.margin = EdgeInsets::new(f32::MAX, f32::INFINITY, f32::NAN, f32::NEG_INFINITY);
    // 构造自动放置的负尺寸子项。
    let second = LayoutChild::new(ComponentId::new(2), Size::new(-20.0, -30.0));
    // 运行混合固定、弹性和自动 track 的 Grid 布局。
    let output = GridLayout::new()
        .with_columns(vec![
            GridTrack::Px(f32::MAX),
            GridTrack::Fr(f32::INFINITY),
            GridTrack::Auto,
        ])
        .with_rows(vec![GridTrack::Auto])
        .with_gap(f32::INFINITY, f32::NAN)
        .with_align(AlignItems::Center)
        .with_justify(JustifyContent::Center)
        .layout(
            Rect::new(f32::INFINITY, 3.0, f32::MAX, f32::INFINITY),
            &[first, second],
        );
    // 全部最终几何必须有限且尺寸非负。
    assert_finite_layout_output(&output);
    // 求解器必须为每个输入子项保留一个输出 frame。
    assert_eq!(output.positions.len(), 2);
}

// 验证有限输入的既有 Flex/Grid 几何语义没有被安全边界改写。
#[test]
// 覆盖正常 gap、margin、track 与子项顺序的精确结果。
fn valid_layout_geometry_remains_stable() {
    // 构造带右外边距的第一个 Flex 子项。
    let mut first = LayoutChild::new(ComponentId::new(1), Size::new(10.0, 8.0));
    // 右外边距应继续推开后续兄弟。
    first.margin = EdgeInsets::new(0.0, 0.0, 3.0, 0.0);
    // 构造带左外边距的第二个 Flex 子项。
    let mut second = LayoutChild::new(ComponentId::new(2), Size::new(20.0, 12.0));
    // 左外边距应继续参与自身起点偏移。
    second.margin = EdgeInsets::new(2.0, 0.0, 0.0, 0.0);
    // 运行有限水平 Flex 布局。
    let flex = FlexLayout::row()
        .with_gap(5.0)
        .with_align(AlignItems::Start)
        .layout(Rect::new(0.0, 0.0, 100.0, 40.0), &[first, second]);
    // 第一个子项仍从内容区起点开始。
    assert_eq!(flex.positions[0], Rect::new(0.0, 0.0, 10.0, 8.0));
    // 第二个子项仍由前项宽度、两侧 margin 与 gap 共同推到 x=20。
    assert_eq!(flex.positions[1], Rect::new(20.0, 0.0, 20.0, 12.0));
    // 有限 Flex 总尺寸继续占满父级内容区。
    assert_eq!(flex.total_size, Size::new(100.0, 40.0));

    // 构造两个自动放置的有限 Grid 子项。
    let grid_children = [
        // 第一个子项进入首列。
        LayoutChild::new(ComponentId::new(3), Size::new(10.0, 8.0)),
        // 第二个子项进入次列。
        LayoutChild::new(ComponentId::new(4), Size::new(20.0, 12.0)),
    ];
    // 运行两列固定 track 与五像素列间距的 Grid 布局。
    let grid = GridLayout::new()
        .with_columns(vec![GridTrack::Px(30.0), GridTrack::Px(40.0)])
        .with_rows(vec![GridTrack::Px(20.0)])
        .with_gap(5.0, 0.0)
        .with_align(AlignItems::Start)
        .with_justify(JustifyContent::Start)
        .layout(Rect::new(0.0, 0.0, 100.0, 40.0), &grid_children);
    // 第一个 Grid 子项保持首列原点与测量尺寸。
    assert_eq!(grid.positions[0], Rect::new(0.0, 0.0, 10.0, 8.0));
    // 第二个 Grid 子项保持固定首列宽度加 gap 后的起点。
    assert_eq!(grid.positions[1], Rect::new(35.0, 0.0, 20.0, 12.0));
    // Grid 总宽度继续等于两列宽度与间距总和。
    assert_eq!(grid.total_size, Size::new(75.0, 20.0));
}

// 验证带 wrap 的固有主轴在零尺寸 bootstrap 阶段仍由子项自然尺寸撑开。
#[test]
// 覆盖 Space 等无显式主轴尺寸容器的首次布局边界。
fn wrapped_intrinsic_flex_preserves_natural_main_extent() {
    // 构造两个具有不同自然宽度的水平子项。
    let children = [
        // 第一个子项贡献三十像素自然宽度。
        LayoutChild::new(ComponentId::new(10), Size::new(30.0, 10.0)),
        // 第二个子项贡献四十像素自然宽度。
        LayoutChild::new(ComponentId::new(11), Size::new(40.0, 12.0)),
    ];
    // 从水平 Flex 默认配置开始构造可换行布局。
    let mut layout = FlexLayout::row()
        .with_gap(5.0)
        .with_align(AlignItems::Start);
    // 开启换行路径以覆盖 wrapped 求解器。
    layout.wrap = true;
    // 声明主轴没有显式尺寸，应由子项自然尺寸撑开。
    layout.intrinsic_main = true;
    // 使用零宽 bootstrap frame 模拟父树首次布局。
    let output = layout.layout(Rect::new(0.0, 0.0, 0.0, 0.0), &children);
    // 首项不得因零宽 bootstrap 被压缩。
    assert_eq!(output.positions[0], Rect::new(0.0, 0.0, 30.0, 10.0));
    // 次项应在首项与 gap 之后继续自然排列。
    assert_eq!(output.positions[1], Rect::new(35.0, 0.0, 40.0, 12.0));
    // 固有总尺寸应由两项宽度、gap 与最大交叉尺寸共同决定。
    assert_eq!(output.total_size, Size::new(75.0, 12.0));
}

// 验证 Flex 交叉轴居中和末端对齐先扣除两侧 margin。
#[test]
// 覆盖非对称交叉轴 margin 的对齐公式。
fn flex_cross_alignment_respects_asymmetric_margins() {
    // 构造十像素高并带非对称上下外边距的子项。
    let mut child = LayoutChild::new(ComponentId::new(12), Size::new(10.0, 10.0));
    // 上边距为二、下边距为八，交叉轴可用区因此为三十像素。
    child.margin = EdgeInsets::new(0.0, 2.0, 0.0, 8.0);
    // 在四十像素高容器中执行居中对齐。
    let centered = FlexLayout::row()
        .with_align(AlignItems::Center)
        .layout(Rect::new(0.0, 0.0, 100.0, 40.0), &[child.clone()]);
    // 子项应在扣除 margin 后的三十像素可用区内居中。
    assert_eq!(centered.positions[0], Rect::new(0.0, 12.0, 10.0, 10.0));
    // 在相同容器中执行交叉轴末端对齐。
    let ended = FlexLayout::row()
        .with_align(AlignItems::End)
        .layout(Rect::new(0.0, 0.0, 100.0, 40.0), &[child]);
    // 子项底边应停在下边距之前。
    assert_eq!(ended.positions[0], Rect::new(0.0, 22.0, 10.0, 10.0));
}

// 验证 wrapped 单行的固有交叉尺寸包含两侧 margin。
#[test]
// 覆盖单行与多行共用行尺寸账本的边界。
fn wrapped_single_line_total_cross_includes_margins() {
    // 构造带二像素上边距和八像素下边距的子项。
    let mut child = LayoutChild::new(ComponentId::new(13), Size::new(20.0, 10.0));
    // 两侧 margin 应使单行总高从十增加到二十像素。
    child.margin = EdgeInsets::new(0.0, 2.0, 0.0, 8.0);
    // 从水平 Flex 默认配置开始构造 wrapped 路径。
    let mut layout = FlexLayout::row().with_align(AlignItems::Start);
    // 即使只有一项，也必须走 wrapped 行账本。
    layout.wrap = true;
    // 在已知主轴宽度和零交叉轴高度下执行布局。
    let output = layout.layout(Rect::new(0.0, 0.0, 100.0, 0.0), &[child]);
    // 子项从上边距之后开始绘制。
    assert_eq!(output.positions[0], Rect::new(0.0, 2.0, 20.0, 10.0));
    // 总高必须包含上下 margin，而不是只返回子项自身高度。
    assert_eq!(output.total_size, Size::new(100.0, 20.0));
}

// 验证普通单行 Flex 的固有交叉尺寸同样包含两侧 margin。
#[test]
// 覆盖 non-wrap 与 wrapped 共用交叉轴占位语义的边界。
fn single_line_total_cross_includes_margins() {
    // 构造带二像素上边距和八像素下边距的子项。
    let mut child = LayoutChild::new(ComponentId::new(14), Size::new(20.0, 10.0));
    // 两侧 margin 应使单行自然总高达到二十像素。
    child.margin = EdgeInsets::new(0.0, 2.0, 0.0, 8.0);
    // 在已知主轴宽度和零交叉轴高度下执行普通单行布局。
    let output = FlexLayout::row()
        .with_align(AlignItems::Start)
        .layout(Rect::new(0.0, 0.0, 100.0, 0.0), &[child]);
    // 子项从上边距之后开始绘制。
    assert_eq!(output.positions[0], Rect::new(0.0, 2.0, 20.0, 10.0));
    // 总高必须包含上下 margin，而不是只返回子项自身高度。
    assert_eq!(output.total_size, Size::new(100.0, 20.0));
}

// 验证 Auto track 先采用子项外尺寸，再把剩余空间交给 Fr track。
#[test]
// 覆盖 mixed Auto+Fr 列与内容定高 Auto 行。
fn grid_auto_tracks_resolve_intrinsic_size_before_fraction_space() {
    // 构造位于首列且带非对称 margin 的内容子项。
    let mut auto_child = LayoutChild::new(ComponentId::new(20), Size::new(30.0, 10.0));
    // 显式放入首个 Auto 单元格。
    auto_child.grid_cell = Some(0);
    // 水平外尺寸为三十五，垂直外尺寸为十五。
    auto_child.margin = EdgeInsets::new(2.0, 1.0, 3.0, 4.0);
    // 构造位于 Fr 列且高度更大的第二个子项。
    let mut fraction_child = LayoutChild::new(ComponentId::new(21), Size::new(10.0, 20.0));
    // 显式放入第二列。
    fraction_child.grid_cell = Some(1);
    // 运行 Auto+1fr 两列、Auto 单行和五像素列 gap 的 Grid。
    let output = GridLayout::new()
        .with_columns(vec![GridTrack::Auto, GridTrack::Fr(1.0)])
        .with_rows(vec![GridTrack::Auto])
        .with_gap(5.0, 0.0)
        .with_align(AlignItems::Start)
        .with_justify(JustifyContent::Start)
        .layout(
            Rect::new(0.0, 0.0, 100.0, 40.0),
            &[auto_child, fraction_child],
        );
    // 首项在三十五像素 Auto 列内扣除 margin 后保持三十像素宽。
    assert_eq!(output.positions[0], Rect::new(2.0, 1.0, 30.0, 10.0));
    // Fr 列应从 Auto 外宽三十五加五像素 gap 后开始。
    assert_eq!(output.positions[1], Rect::new(40.0, 0.0, 10.0, 20.0));
    // 列总宽占满容器，Auto 行总高只取最大子项外高二十。
    assert_eq!(output.total_size, Size::new(100.0, 20.0));
}

// 验证没有 Fr 时 Auto track 保持内容尺寸而不吸收全部剩余空间。
#[test]
// 覆盖纯 Auto 列的固有宽度与列 gap 账本。
fn grid_auto_tracks_remain_content_sized_without_fraction_tracks() {
    // 构造首列二十乘十的内容子项。
    let mut first = LayoutChild::new(ComponentId::new(22), Size::new(20.0, 10.0));
    // 显式放入首列。
    first.grid_cell = Some(0);
    // 构造次列三十乘十二的内容子项。
    let mut second = LayoutChild::new(ComponentId::new(23), Size::new(30.0, 12.0));
    // 显式放入次列。
    second.grid_cell = Some(1);
    // 运行两列 Auto 与五像素列 gap 的 Grid。
    let output = GridLayout::new()
        .with_columns(vec![GridTrack::Auto, GridTrack::Auto])
        .with_rows(vec![GridTrack::Auto])
        .with_gap(5.0, 0.0)
        .with_align(AlignItems::Start)
        .with_justify(JustifyContent::Start)
        .layout(Rect::new(0.0, 0.0, 100.0, 40.0), &[first, second]);
    // 首项保持首列自然宽度。
    assert_eq!(output.positions[0], Rect::new(0.0, 0.0, 20.0, 10.0));
    // 次列从二十像素首列加五像素 gap 后开始。
    assert_eq!(output.positions[1], Rect::new(25.0, 0.0, 30.0, 12.0));
    // 总尺寸只包含两列内容宽、gap 与最大行高。
    assert_eq!(output.total_size, Size::new(55.0, 12.0));
}

// 验证跨多个 Auto track 的内容贡献会扩展整个 span。
#[test]
// 覆盖 spanning 子项在两个 Auto 列间均分缺口的规则。
fn grid_spanning_child_expands_auto_tracks() {
    // 构造跨两列且自然宽七十的主子项。
    let mut spanning = LayoutChild::new(ComponentId::new(24), Size::new(70.0, 12.0));
    // 主子项从首格开始。
    spanning.grid_cell = Some(0);
    // 主子项横跨两个 Auto 列。
    spanning.grid_column_span = 2;
    // 构造零尺寸探针以观察第二列起点。
    let mut probe = LayoutChild::new(ComponentId::new(25), Size::zero());
    // 探针与 spanning 子项显式重叠在第二列，Grid 允许显式叠放。
    probe.grid_cell = Some(1);
    // 运行两列 Auto 与十像素列 gap 的 Grid。
    let output = GridLayout::new()
        .with_columns(vec![GridTrack::Auto, GridTrack::Auto])
        .with_rows(vec![GridTrack::Auto])
        .with_gap(10.0, 0.0)
        .with_align(AlignItems::Start)
        .with_justify(JustifyContent::Start)
        .layout(Rect::new(0.0, 0.0, 100.0, 40.0), &[spanning, probe]);
    // 跨列子项获得两列各三十加十像素 gap 的完整七十像素宽度。
    assert_eq!(output.positions[0], Rect::new(0.0, 0.0, 70.0, 12.0));
    // 第二列从三十像素首列和十像素 gap 后开始。
    assert_eq!(output.positions[1], Rect::new(40.0, 0.0, 0.0, 0.0));
    // Grid 总尺寸收敛到跨列内容宽与 Auto 行内容高。
    assert_eq!(output.total_size, Size::new(70.0, 12.0));
}

// 验证跨多个 Auto 行的内容贡献使用与列轴对称的规则。
#[test]
// 覆盖 spanning 子项在两个 Auto 行间均分缺口的垂直分支。
fn grid_row_spanning_child_expands_auto_tracks() {
    // 构造跨两行且自然高五十的主子项。
    let mut spanning = LayoutChild::new(ComponentId::new(26), Size::new(12.0, 50.0));
    // 主子项从首行开始。
    spanning.grid_cell = Some(0);
    // 主子项纵向跨越两个 Auto 行。
    spanning.grid_row_span = 2;
    // 构造零尺寸探针以观察第二行起点。
    let mut probe = LayoutChild::new(ComponentId::new(27), Size::zero());
    // 单列 Grid 中的第二个单元格对应第二行。
    probe.grid_cell = Some(1);
    // 运行单列 Auto、两行 Auto 与十像素行 gap 的 Grid。
    let output = GridLayout::new()
        .with_columns(vec![GridTrack::Auto])
        .with_rows(vec![GridTrack::Auto, GridTrack::Auto])
        .with_gap(0.0, 10.0)
        .with_align(AlignItems::Start)
        .with_justify(JustifyContent::Start)
        .layout(Rect::new(0.0, 0.0, 40.0, 100.0), &[spanning, probe]);
    // 跨行子项获得两行各二十加十像素 gap 的完整五十像素高度。
    assert_eq!(output.positions[0], Rect::new(0.0, 0.0, 12.0, 50.0));
    // 第二行从二十像素首行和十像素 gap 后开始。
    assert_eq!(output.positions[1], Rect::new(0.0, 30.0, 0.0, 0.0));
    // Grid 总尺寸同时收敛到单列内容宽与跨行内容高。
    assert_eq!(output.total_size, Size::new(12.0, 50.0));
}

// 验证超出 Grid 资源窗口的显式 cell 会收敛到最后一个可地址单元格。
#[test]
// 使用五千行级输入在旧实现上安全复现无界隐式行扩张。
fn grid_explicit_cell_is_clamped_to_the_placement_budget() {
    // 构造具有有限自然尺寸的显式放置子项。
    let mut child = LayoutChild::new(ComponentId::new(28), Size::new(10.0, 10.0));
    // 在两列 Grid 中请求第五千行的第一列。
    child.grid_cell = Some(10_000);
    // 运行两列 Auto 与单像素双轴 gap 的 Grid。
    let output = GridLayout::new()
        .with_columns(vec![GridTrack::Auto, GridTrack::Auto])
        .with_gap(1.0, 1.0)
        .with_align(AlignItems::Start)
        .with_justify(JustifyContent::Start)
        .layout(Rect::new(0.0, 0.0, 20.0, 20.0), &[child]);
    // 四千九十六行预算下的最后单元格位于次列与最后一行。
    assert_eq!(output.positions[0], Rect::new(1.0, 4095.0, 10.0, 10.0));
    // 总尺寸只包含有界轨道、gap 与子项内容。
    assert_eq!(output.total_size, Size::new(11.0, 4105.0));
}

// 验证超大行 span 不会让隐式行和占用矩阵无界增长。
#[test]
// 使用五千行 span 在旧实现上安全复现超出资源预算的扩容。
fn grid_row_span_is_clamped_to_the_track_budget() {
    // 构造一个自然高二十的跨行子项。
    let mut child = LayoutChild::new(ComponentId::new(29), Size::new(10.0, 20.0));
    // 从首格开始显式放置。
    child.grid_cell = Some(0);
    // 请求超出四千九十六行资源窗口的 span。
    child.grid_row_span = 5_000;
    // 运行单列 Auto 与单像素行 gap 的 Grid。
    let output = GridLayout::new()
        .with_columns(vec![GridTrack::Auto])
        .with_gap(0.0, 1.0)
        .with_align(AlignItems::Start)
        .with_justify(JustifyContent::Start)
        .layout(Rect::new(0.0, 0.0, 20.0, 20.0), &[child]);
    // 子项仍在有界 span 中保留自然尺寸。
    assert_eq!(output.positions[0], Rect::new(0.0, 0.0, 10.0, 20.0));
    // 四千九十六条零高 Auto 行之间只有四千九十五个 gap。
    assert_eq!(output.total_size, Size::new(10.0, 4095.0));
}

// 验证公开 GridLayout 入口能端到端收敛整数极值与耗尽的自动放置。
#[test]
// 覆盖 usize::MAX cell、u32::MAX 双轴 span 与无剩余矩形的组合。
fn grid_integer_extremes_remain_bounded_end_to_end() {
    // 构造使用极大 cell 与双轴 span 的显式子项。
    let mut explicit = LayoutChild::new(ComponentId::new(30), Size::new(10.0, 10.0));
    // 显式位置使用 usize 可表示的最大索引。
    explicit.grid_cell = Some(usize::MAX);
    // 列 span 使用 u32 极值并由最后一列剩余空间收敛。
    explicit.grid_column_span = u32::MAX;
    // 行 span 使用 u32 极值并由最后一行剩余空间收敛。
    explicit.grid_row_span = u32::MAX;
    // 构造请求占满整个有界网格的自动子项。
    let mut automatic = LayoutChild::new(ComponentId::new(31), Size::new(5.0, 5.0));
    // 自动子项横向请求极大 span。
    automatic.grid_column_span = u32::MAX;
    // 自动子项纵向请求极大 span。
    automatic.grid_row_span = u32::MAX;
    // 运行两列 Auto 与单像素双轴 gap 的 Grid。
    let output = GridLayout::new()
        .with_columns(vec![GridTrack::Auto, GridTrack::Auto])
        .with_gap(1.0, 1.0)
        .with_align(AlignItems::Start)
        .with_justify(JustifyContent::Start)
        .layout(Rect::new(0.0, 0.0, 20.0, 20.0), &[explicit, automatic]);
    // 全部公开输出仍须满足有限非负布局不变量。
    assert_finite_layout_output(&output);
    // 显式子项收敛到有界矩阵的最后一格。
    assert_eq!(output.positions[0], Rect::new(1.0, 4095.0, 10.0, 10.0));
    // 整个资源窗口内无法容纳自动矩形时，它保留零 frame。
    assert_eq!(output.positions[1], Rect::zero());
    // 总尺寸仅包含有界行列、gap 与成功放置的内容。
    assert_eq!(output.total_size, Size::new(11.0, 4105.0));
}

// 验证 Grid 子项的 align-self 能覆盖容器级交叉轴对齐。
#[test]
// 使用非对称 margin 区分末端对齐与容器默认拉伸。
fn grid_child_align_self_overrides_container_alignment() {
    // 构造十乘十且带非对称上下外边距的子项。
    let mut child = LayoutChild::new(ComponentId::new(32), Size::new(10.0, 10.0));
    // 上边距为二、下边距为八，单元格交叉轴可用区因此为三十像素。
    child.margin = EdgeInsets::new(0.0, 2.0, 0.0, 8.0);
    // 子项显式覆盖容器默认拉伸并贴近交叉轴末端。
    child.align_self = Some(AlignItems::End);
    // 在四十乘四十固定单元格中执行 Grid 布局。
    let output = GridLayout::new()
        .with_columns(vec![GridTrack::Px(40.0)])
        .with_rows(vec![GridTrack::Px(40.0)])
        .with_align(AlignItems::Stretch)
        .with_justify(JustifyContent::Start)
        .layout(Rect::new(0.0, 0.0, 40.0, 40.0), &[child]);
    // 子项底边应停在八像素下边距之前且保留自然高度。
    assert_eq!(output.positions[0], Rect::new(0.0, 22.0, 10.0, 10.0));
}

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
