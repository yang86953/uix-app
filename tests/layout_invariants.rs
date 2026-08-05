// 导入布局契约测试使用的核心几何类型。
use uix::core::{ComponentId, EdgeInsets, Rect, Size};
// 导入三个共享布局入口及其公开输出类型。
use uix::ui::{
    AlignItems, BoxModel, FlexLayout, GridLayout, GridTrack, JustifyContent, LayoutChild,
    LayoutEngine, LayoutOutput,
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
