// 导入当前模块的 Grid 与布局类型。
use super::*;

// 验证直接构造与 View 构建共享同目录 UIX 的静态视觉地址。
#[test]
fn view_build_uses_uix_visual_without_a_rust_default_copy() {
    let grid = Grid::new();
    assert!(std::ptr::eq(grid.visual, GRID_VISUAL_REF));
    let node = crate::ui::view::View::build(grid);
    let grid = node
        .widget
        .as_any()
        .downcast_ref::<Grid>()
        .expect("UIX 根必须保留 Grid Rust 内核");
    assert!(std::ptr::eq(grid.visual, GRID_VISUAL_REF));
    assert_eq!(Breakpoints::antd(), GRID_VISUAL_REF.breakpoints);
}
// 导入调用组件布局 trait 所需的公开接口。
use crate::ui::WidgetLayout;

// 断言浮点布局坐标接近预期值。
fn assert_near(actual: f32, expected: f32) {
    // 布局误差必须保持在单精度计算可接受范围内。
    assert!(
        // 比较实际值与预期值的绝对差。
        (actual - expected).abs() < 0.001,
        // 失败时同时报告实际值与预期值。
        "expected {expected}, got {actual}"
    );
}

// 验证 Grid::justify 对齐整组固定列轨，而不是改写单元格内子项宽度。
#[test]
fn justify_aligns_fixed_tracks_as_grid_content() {
    // 两个十像素子项分别显式放入两列。
    let mut first = LayoutChild::new(WidgetId::new(1), Size::new(10.0, 10.0));
    // 首项进入第一格。
    first.grid_cell = Some(0);
    // 构造第二个十像素子项。
    let mut second = LayoutChild::new(WidgetId::new(2), Size::new(10.0, 10.0));
    // 次项进入第二格。
    second.grid_cell = Some(1);
    // 空树足以满足不读取子树的布局入口。
    let tree = WidgetTree::new();
    // 枚举六种不会改变固定轨道尺寸的内容对齐结果。
    let cases = [
        // Start 保持轨道组贴左。
        (JustifyContent::Start, 0.0, 30.0),
        // Center 把五十像素轨道组居中到一百像素容器。
        (JustifyContent::Center, 25.0, 55.0),
        // End 把轨道组贴到容器右侧。
        (JustifyContent::End, 50.0, 80.0),
        // SpaceBetween 把全部剩余空间加入两列之间。
        (JustifyContent::SpaceBetween, 0.0, 80.0),
        // SpaceAround 在两侧保留半份分布空间。
        (JustifyContent::SpaceAround, 12.5, 67.5),
        // SpaceEvenly 在两侧与列间保留等份空间。
        (JustifyContent::SpaceEvenly, 50.0 / 3.0, 190.0 / 3.0),
    ];
    // 逐种验证公开 Grid::justify 的端到端接线。
    for (justify, first_x, second_x) in cases {
        // 构造两条固定二十像素列和十像素基础 gap。
        let grid = Grid::new()
            // 固定轨道不会吸收内容对齐剩余空间。
            .columns(vec![GridTrack::Px(20.0), GridTrack::Px(20.0)])
            // 两列之间保留十像素作者间距。
            .gap(10.0)
            // 通过公开入口声明整组列轨的主轴对齐。
            .justify(justify)
            // 子项保持自然高度。
            .align(AlignItems::Start);
        // 在一百像素宽的实际 frame 中执行组件布局。
        let positions = grid.layout_children(
            // 父级提供一百乘二十的内容空间。
            Rect::new(0.0, 0.0, 100.0, 20.0),
            // 使用相同的两个显式子项。
            &[first.clone(), second.clone()],
            // 当前测试不依赖树内容。
            &tree,
        );
        // 首项横坐标应由整组列轨对齐决定。
        assert_near(positions[0].1.x, first_x);
        // 次项横坐标应同时包含首列宽度与分布后间距。
        assert_near(positions[1].1.x, second_x);
        // 内容对齐不能把首项拉伸到固定轨道宽度。
        assert_eq!(positions[0].1.w, 10.0);
        // 内容对齐不能把次项拉伸到固定轨道宽度。
        assert_eq!(positions[1].1.w, 10.0);
    }
}

// 验证 Stretch 只扩展 Auto 轨道，不拉伸单元格内子项。
#[test]
fn justify_stretch_expands_auto_tracks() {
    // 构造首个十像素自然宽度子项。
    let mut first = LayoutChild::new(WidgetId::new(3), Size::new(10.0, 10.0));
    // 首项进入第一格。
    first.grid_cell = Some(0);
    // 构造次个十像素自然宽度子项。
    let mut second = LayoutChild::new(WidgetId::new(4), Size::new(10.0, 10.0));
    // 次项进入第二格。
    second.grid_cell = Some(1);
    // 构造两条 Auto 列的 Stretch Grid。
    let grid = Grid::new()
        // 两条轨道初始都由十像素内容决定。
        .columns(vec![GridTrack::Auto, GridTrack::Auto])
        // 作者 gap 固定为十像素。
        .gap(10.0)
        // 剩余七十像素应均分给两条 Auto 轨道。
        .justify(JustifyContent::Stretch)
        // 子项保持自然高度。
        .align(AlignItems::Start);
    // 空树足以满足不读取子树的布局入口。
    let tree = WidgetTree::new();
    // 在一百像素宽容器中执行组件布局。
    let positions = grid.layout_children(
        // 父级提供一百乘二十的内容空间。
        Rect::new(0.0, 0.0, 100.0, 20.0),
        // 使用两个显式 Auto 轨道子项。
        &[first, second],
        // 当前测试不依赖树内容。
        &tree,
    );
    // 首项保持自然宽度并贴住第一条扩展后轨道起点。
    assert_eq!(positions[0].1, Rect::new(0.0, 0.0, 10.0, 10.0));
    // 第二条轨道从四十五像素首列与十像素 gap 后开始。
    assert_eq!(positions[1].1, Rect::new(55.0, 0.0, 10.0, 10.0));
}

// 验证 Grid 绘制不会在父级已经消费 margin 后再次缩小视觉矩形。
#[test]
fn visual_rect_keeps_parent_assigned_border_box() {
    // 构造带非零外边距的 Grid。
    let mut grid = Grid::new();
    // 直接声明四侧不同 margin，覆盖坐标和尺寸的旧二次扣除路径。
    grid.style.margin = EdgeInsets::new(3.0, 5.0, 7.0, 11.0);
    // 父布局分配的 frame 已是不含 margin 的 border-box。
    let frame = Rect::new(20.0, 30.0, 100.0, 60.0);
    // Grid 视觉区域必须完整保留父级 border-box。
    assert_eq!(grid.visual_rect(frame), frame);
}
