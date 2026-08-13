// 导入被测的声明树适配器。
use super::ViewAdapter;
// 导入构造布局几何变化、滚动偏移与固有尺寸断言所需的核心类型。
use crate::core::{Constraints, EdgeInsets, Point, Rect, Size};
// 导入构造绘制变化所需的颜色常量。
use crate::draw::Color;
// 导入建立最小组件树、缓存门禁与保守回退门禁所需的组件类型。
use crate::ui::widgets::{
    Affix, Button, Card, Carousel, Container, Grid, Label, ScrollView, Space,
};
// 导入构造纵向滚动视口所需的方向类型。
use crate::ui::{ScrollDirection, State};
// 导入构造 Grid 轨道几何变化所需的轨道类型。
use crate::ui::GridTrack;
// 导入把叶组件包装成声明节点所需的节点类型。
use crate::ui::view::ViewNode;
// 导入读取节点 frame 与直接子节点顺序所需的核心组件接口。
use crate::ui::component::widget::WidgetCore;
// 导入并发计数器以观察 Effect 的运行次数。
use std::sync::atomic::{AtomicUsize, Ordering};
// 导入跨 Effect 闭包共享观察值的所有权句柄。
use std::sync::Arc;
// 导入直接调用定制子布局与测量入口所需的布局接口。
use crate::ui::WidgetLayout;
// 导入读取共享失效队列所需的组件树类型。
use crate::ui::WidgetTree;

// 完成初始布局并清除建树阶段产生的失效信号。
fn clear_initial_invalidations(tree: &mut WidgetTree) {
    // 为根组件建立正尺寸 frame，使后续 Paint 能生成精确脏区。
    tree.layout();
    // 克隆共享句柄，避免队列锁借用影响后续树操作。
    let invalidation = tree.invalidation().clone();
    // 即使测试线程曾恐慌，也恢复队列锁并继续给出精确断言。
    let mut queue = invalidation
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    // 丢弃建树与首轮布局产生的基线工作。
    queue.clear();
}

// 读取当前队列是否分别包含布局工作和绘制工作。
fn invalidation_flags(tree: &WidgetTree) -> (bool, bool) {
    // 锁定共享队列以取得同一时刻的两个失效分类。
    let queue = tree
        .invalidation()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    // 按布局、绘制的固定顺序返回门禁结果。
    (queue.has_layout(), queue.has_paint_or_composite())
}

// 在宽松约束下读取根组件当前暴露的固有尺寸。
fn root_measure(tree: &WidgetTree) -> Size {
    // 构造足够容纳测试子项的有限宽松约束。
    let constraints = Constraints::loose(Size::new(1_000.0, 1_000.0));
    // 根节点在声明树建成后必须存在，直接读取其组件测量结果。
    tree.root()
        .expect("测试声明树必须存在根节点")
        .measure(constraints)
}

// 验证标签颜色变化只需要重绘而不重新布局。
#[test]
fn label_color_change_only_invalidates_paint() {
    // 构建具有稳定文本和初始颜色的标签树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Label::new("stable").color(Color::RED)));
    // 清除初始建树与布局失效。
    clear_initial_invalidations(&mut tree);

    // 只改变不会影响文本度量的标签颜色。
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Label::new("stable").color(Color::BLUE)),
    );

    // 颜色变化必须产生 Paint，且不得升级为 Layout。
    assert_eq!(invalidation_flags(&tree), (false, true));
}

// 验证容器背景变化只需要重绘而不重新布局。
#[test]
fn container_background_change_only_invalidates_paint() {
    // 构建具有固定几何和初始背景的容器树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Container::new().size(80.0, 40.0).bg(Color::RED),
    ));
    // 清除初始建树与布局失效。
    clear_initial_invalidations(&mut tree);

    // 只改变不参与盒模型计算的背景颜色。
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Container::new().size(80.0, 40.0).bg(Color::BLUE)),
    );

    // 背景变化必须产生 Paint，且不得升级为 Layout。
    assert_eq!(invalidation_flags(&tree), (false, true));
}

// 验证文本内容变化仍会使文本度量和布局失效。
#[test]
fn label_text_change_invalidates_layout() {
    // 构建初始短文本标签树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Label::new("short")));
    // 清除初始建树与布局失效。
    clear_initial_invalidations(&mut tree);

    // 改变参与固有尺寸测量的文本内容。
    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Label::new("a much longer label")));

    // 文本变化必须同时请求 Layout 与 Paint。
    assert_eq!(invalidation_flags(&tree), (true, true));
}

// 验证盒模型几何变化仍会使布局失效。
#[test]
fn container_padding_change_invalidates_layout() {
    // 构建无内边距的固定尺寸容器树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new().size(80.0, 40.0)));
    // 清除初始建树与布局失效。
    clear_initial_invalidations(&mut tree);

    // 改变参与内容区域计算的容器内边距。
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Container::new()
                .size(80.0, 40.0)
                .padding(EdgeInsets::uniform(4.0)),
        ),
    );

    // 内边距变化必须同时请求 Layout 与 Paint。
    assert_eq!(invalidation_flags(&tree), (true, true));
}

// 验证 Grid 绘制字段与轨道字段进入不同失效通道。
#[test]
fn grid_style_change_distinguishes_paint_from_tracks() {
    // 构建具有固定几何和初始背景的空 Grid 树。
    let mut tree =
        ViewAdapter::build_nodes(ViewNode::leaf(Grid::new().size(80.0, 40.0).bg(Color::RED)));
    // 清除初始建树与布局失效。
    clear_initial_invalidations(&mut tree);

    // 只改变不参与轨道求解的 Grid 背景。
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Grid::new().size(80.0, 40.0).bg(Color::BLUE)),
    );
    // 背景变化必须只产生 Paint。
    assert_eq!(invalidation_flags(&tree), (false, true));
    // 清除背景变化产生的 Paint，以隔离下一步轨道断言。
    tree.invalidation()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear();

    // 增加参与 Grid 求解的显式列轨道。
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Grid::new()
                .size(80.0, 40.0)
                .bg(Color::BLUE)
                .columns(vec![GridTrack::Px(80.0)]),
        ),
    );
    // 轨道变化必须同时请求 Layout 与 Paint。
    assert_eq!(invalidation_flags(&tree), (true, true));
}

// 验证其余组件的显式保守分类继续请求布局。
#[test]
fn conservative_widget_change_invalidates_layout() {
    // 构建采用保守分类的按钮树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Button::new("short")));
    // 清除初始建树与布局失效。
    clear_initial_invalidations(&mut tree);

    // 改变当前未纳入精细字段分类的按钮文本。
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Button::new("a much longer button")),
    );

    // 保守配置变化必须同时请求 Layout 与 Paint，不能误吞布局工作。
    assert_eq!(invalidation_flags(&tree), (true, true));
}

// 验证 Container 移除最后一个子节点时立即清除内容尺寸缓存。
#[test]
fn container_last_child_removal_clears_intrinsic_cache() {
    // 构建由固定尺寸标签撑开的无显式尺寸容器。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        Container::new(),
        vec![ViewNode::leaf(Label::new("content").size(120.0, 30.0))],
    ));
    // 完成首轮布局，让容器记录真实子内容尺寸。
    clear_initial_invalidations(&mut tree);
    // 读取首轮布局后由子内容撑开的容器尺寸。
    let measured = root_measure(&tree);
    // 前置条件：至少一个轴必须已经记录非零子内容范围。
    assert!(measured.w > 0.0 || measured.h > 0.0);

    // 原位协调为同类型空容器，触发最后一个子节点移除路径。
    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Container::new()));

    // 子节点移除后不得继续暴露上一轮内容尺寸。
    assert_eq!(root_measure(&tree), Size::zero());
}

// 验证 Space 移除最后一个子节点时立即清除内容尺寸缓存。
#[test]
fn space_last_child_removal_clears_intrinsic_cache() {
    // 构建由固定尺寸标签撑开的无显式尺寸间距容器。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        Space::new(),
        vec![ViewNode::leaf(Label::new("content").size(90.0, 24.0))],
    ));
    // 完成首轮布局，让 Space 记录真实子内容尺寸。
    clear_initial_invalidations(&mut tree);
    // 读取首轮布局后由子内容撑开的 Space 尺寸。
    let measured = root_measure(&tree);
    // 前置条件：至少一个轴必须已经记录非零子内容范围。
    assert!(measured.w > 0.0 || measured.h > 0.0);

    // 原位协调为同类型空 Space，触发最后一个子节点移除路径。
    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Space::new()));

    // 子节点移除后不得继续暴露上一轮内容尺寸。
    assert_eq!(root_measure(&tree), Size::zero());
}

// 验证 Affix 移除最后一个子节点时立即清除占位尺寸缓存。
#[test]
fn affix_last_child_removal_clears_intrinsic_cache() {
    // 构建由固定尺寸标签撑开的吸顶占位容器。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        Affix::new(8.0),
        vec![ViewNode::leaf(Label::new("content").size(80.0, 28.0))],
    ));
    // 完成首轮布局，让 Affix 记录真实子项占位尺寸。
    clear_initial_invalidations(&mut tree);
    // 读取首轮布局后由子项撑开的 Affix 占位尺寸。
    let measured = root_measure(&tree);
    // 前置条件：至少一个轴必须已经记录非零子项占位。
    assert!(measured.w > 0.0 || measured.h > 0.0);

    // 原位协调为同类型空 Affix，触发最后一个子节点移除路径。
    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Affix::new(8.0)));

    // 子节点移除后不得继续保留上一轮吸顶占位。
    assert_eq!(root_measure(&tree), Size::zero());
}

// 验证 ScrollView 移除最后一个子节点时立即清除滚动范围与偏移。
#[test]
fn scroll_view_last_child_removal_clears_scroll_state() {
    // 建立双向绑定的滚动偏移，验证结构归零会同步到声明状态。
    let offset = State::new(Point::new(0.0, 0.0));
    // 构建内容高度大于固定视口的纵向滚动树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        ScrollView::new(ScrollDirection::Vertical)
            .size(80.0, 40.0)
            .scroll_offset(&offset),
        vec![ViewNode::leaf(Label::new("content").size(80.0, 200.0))],
    ));
    // 完成首轮布局，让滚动视口记录内容范围。
    clear_initial_invalidations(&mut tree);
    // 取得可变根组件并写入一个有效的非零滚动偏移。
    let scroll = tree
        .root_mut()
        .expect("测试声明树必须存在根节点")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<ScrollView>()
        .expect("测试根组件必须是 ScrollView");
    // 前置条件：高内容必须产生纵向滚动范围。
    assert!(scroll.max_scroll_y() > 0.0);
    // 将滚动位置推进到范围内部，验证移除时会归零运行态偏移。
    scroll.scroll_to_xy(0.0, 20.0);
    // 前置条件：运行态偏移必须成功更新。
    assert_eq!(scroll.scroll_y(), 20.0);
    // 双向绑定必须接收同一非零运行态偏移。
    assert_eq!(offset.get(), Point::new(0.0, 20.0));

    // 原位协调为同尺寸空视口，触发最后一个子节点移除路径。
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            ScrollView::new(ScrollDirection::Vertical)
                .size(80.0, 40.0)
                .scroll_offset(&offset),
        ),
    );

    // 重新取得协调后保留的同一 ScrollView 实例。
    let scroll = tree
        .root()
        .expect("测试声明树必须存在根节点")
        .component()
        .as_any()
        .downcast_ref::<ScrollView>()
        .expect("测试根组件必须是 ScrollView");
    // 空视口不得继续暴露已移除内容的滚动范围。
    assert_eq!(scroll.max_scroll_y(), 0.0);
    // 空视口不得继续保留超出当前范围的滚动偏移。
    assert_eq!(scroll.scroll_y(), 0.0);
    // 受控偏移也必须同步归零，不能在下一次协调时恢复陈旧位置。
    assert_eq!(offset.get(), Point::new(0.0, 0.0));
}

// 验证 Carousel 移除最后一组幻灯片时立即清除直接子节点计数。
#[test]
fn carousel_last_slide_removal_clears_runtime_count() {
    // 构建含两个幻灯片的轮播声明树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        Carousel::new().size(120.0, 60.0),
        vec![
            ViewNode::leaf(Label::new("first").size(120.0, 60.0)),
            ViewNode::leaf(Label::new("second").size(120.0, 60.0)),
        ],
    ));
    // 完成首轮布局，让运行态记录精确幻灯片数量。
    clear_initial_invalidations(&mut tree);
    // 取得布局后的轮播组件。
    let carousel = tree
        .root()
        .expect("测试声明树必须存在根节点")
        .component()
        .as_any()
        .downcast_ref::<Carousel>()
        .expect("测试根组件必须是 Carousel");
    // 前置条件：轮播运行态必须已经记录两个幻灯片。
    assert_eq!(carousel.slide_count(), 2);

    // 原位协调为同尺寸空轮播，触发最后一组幻灯片移除路径。
    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Carousel::new().size(120.0, 60.0)));

    // 重新取得协调后保留的同一 Carousel 实例。
    let carousel = tree
        .root()
        .expect("测试声明树必须存在根节点")
        .component()
        .as_any()
        .downcast_ref::<Carousel>()
        .expect("测试根组件必须是 Carousel");
    // 空轮播不得继续暴露已移除幻灯片的数量。
    assert_eq!(carousel.slide_count(), 0);
    // 空轮播的活动索引必须归一到零。
    assert_eq!(carousel.current_index(), 0);
}

// 验证 Carousel 的结构通知不会把自定义箭头误计为幻灯片。
#[test]
fn carousel_structure_count_excludes_custom_arrow() {
    // 构建带两个幻灯片和一个自定义箭头子树的轮播。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        Carousel::new()
            .size(120.0, 60.0)
            .arrows(|_, _| ViewNode::leaf(Label::new("arrows"))),
        vec![
            ViewNode::leaf(Label::new("first").size(120.0, 60.0)),
            ViewNode::leaf(Label::new("second").size(120.0, 60.0)),
        ],
    ));
    // 取得建树后尚未依赖布局修正的轮播组件。
    let carousel = tree
        .root()
        .expect("测试声明树必须存在根节点")
        .component()
        .as_any()
        .downcast_ref::<Carousel>()
        .expect("测试根组件必须是 Carousel");
    // 两个幻灯片加一个箭头仍只能发布两个幻灯片。
    assert_eq!(carousel.slide_count(), 2);

    // 保留自定义箭头但移除全部幻灯片，且暂不执行下一轮布局。
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Carousel::new()
                .size(120.0, 60.0)
                .arrows(|_, _| ViewNode::leaf(Label::new("arrows"))),
        ),
    );

    // 重新取得结构协调后保留的轮播组件。
    let carousel = tree
        .root()
        .expect("测试声明树必须存在根节点")
        .component()
        .as_any()
        .downcast_ref::<Carousel>()
        .expect("测试根组件必须是 Carousel");
    // 唯一剩余的箭头子树不得形成虚假的幻灯片计数。
    assert_eq!(carousel.slide_count(), 0);
}

// 验证组件测量与公开 frame 写入都不会把无界哨兵或负尺寸写入布局树。
#[test]
fn widget_tree_normalizes_materialized_frames() {
    // 构造会从组件测量边界返回无界宽度的根容器。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        // 显式使用测量哨兵覆盖根节点 bootstrap 路径。
        Container::new().size(f32::MAX, 20.0),
    ));
    // 运行布局，使根节点从组件测量结果建立实际 frame。
    tree.layout();
    // 读取首轮布局写入的根节点 frame。
    let measured_frame = tree.root().expect("测试声明树必须存在根节点").frame();
    // 实际宽度不得保留测量阶段的无界哨兵。
    assert!(measured_frame.w.is_finite() && measured_frame.w < f32::MAX);
    // 实际高度必须保持有限非负。
    assert!(measured_frame.h.is_finite() && measured_frame.h >= 0.0);

    // 取得根节点标识以覆盖公开脏 frame 写入路径。
    let root_id = tree.root_id().expect("测试声明树必须存在根节点标识");
    // 尝试写入非有限坐标、负宽度和无界高度。
    tree.set_frame_dirty(
        root_id,
        // 组合覆盖坐标与尺寸的全部非法类别。
        Rect::new(f32::INFINITY, f32::NEG_INFINITY, -10.0, f32::MAX),
    );
    // 读取统一写入边界保存的最终 frame。
    let assigned_frame = tree.root().expect("测试声明树必须存在根节点").frame();
    // 横坐标必须收敛到有限值。
    assert!(assigned_frame.x.is_finite());
    // 纵坐标必须收敛到有限值。
    assert!(assigned_frame.y.is_finite());
    // 宽度必须收敛到有限非负值。
    assert!(assigned_frame.w.is_finite() && assigned_frame.w >= 0.0);
    // 高度不得保留无界哨兵。
    assert!(assigned_frame.h.is_finite() && assigned_frame.h < f32::MAX);
}

// 验证保留身份的隐藏子节点完全退出父容器固有尺寸计算。
#[test]
fn hidden_space_child_does_not_affect_intrinsic_size() {
    // 构造一项可见内容和一项更宽的隐藏内容。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        // 使用无显式尺寸的 Space 观察真实子内容范围。
        Space::new(),
        // 声明两个尺寸差异明显的直接子节点。
        vec![
            // 可见子节点决定最终固有尺寸。
            ViewNode::leaf(Label::new("visible").size(20.0, 12.0)),
            // 隐藏子节点保留身份但不得占用布局空间。
            ViewNode::leaf(Label::new("hidden").size(80.0, 12.0)).visible(false),
        ],
    ));
    // 完成布局并让 Space 缓存实际可见子内容范围。
    clear_initial_invalidations(&mut tree);
    // 读取由可见子树决定的固有尺寸。
    let hidden_measure = root_measure(&tree);
    // 隐藏项及其前置 gap 均不得增加横向固有尺寸。
    assert_eq!(hidden_measure.w, 20.0);
    // 可见项必须继续产生正的纵向固有尺寸。
    assert!(hidden_measure.h > 0.0);

    // 原位协调为同一结构并恢复第二项可见。
    ViewAdapter::reconcile_nodes(
        &mut tree,
        // 保持父组件类型和布局参数不变。
        ViewNode::new(
            // 复用无显式尺寸的 Space。
            Space::new(),
            // 第二项恢复为普通可见声明。
            vec![
                // 第一项保持原尺寸。
                ViewNode::leaf(Label::new("visible").size(20.0, 12.0)),
                // 恢复的第二项应重新进入布局。
                ViewNode::leaf(Label::new("hidden").size(80.0, 12.0)),
            ],
        ),
    );
    // 完成恢复可见后的布局收敛。
    clear_initial_invalidations(&mut tree);
    // 读取包含两个可见项及默认 gap 的固有尺寸。
    let visible_measure = root_measure(&tree);
    // 20 + 8 gap + 80 应完整恢复。
    assert_eq!(visible_measure.w, 108.0);
    // 同高子项恢复可见不得改变父级纵向固有尺寸。
    assert_eq!(visible_measure.h, hidden_measure.h);
}

// 验证嵌套 Grid 在没有显式高度时可由 Auto 轨道和子内容完成首轮启动。
#[test]
fn nested_grid_bootstraps_from_auto_track_content() {
    // 构造具有确定视口但不替 Grid 声明显式尺寸的父容器。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        // 父容器提供有限可用空间。
        Container::new().size(200.0, 100.0),
        // 嵌套一个完全依赖 Auto 轨道固有尺寸的 Grid。
        vec![ViewNode::new(
            // 单列单行 Grid 不声明自身宽高。
            Grid::new()
                .columns(vec![GridTrack::Auto])
                .rows(vec![GridTrack::Auto]),
            // 固定尺寸内容应撑开 Grid 的 Auto 行。
            vec![ViewNode::leaf(Label::new("content").size(60.0, 24.0))],
        )],
    ));
    // 运行完整布局收敛循环。
    clear_initial_invalidations(&mut tree);
    // 定位嵌套 Grid 节点。
    let grid_id = tree
        .find_by_type::<Grid>()
        .expect("测试声明树必须包含 Grid 节点");
    // 复制 Grid 的唯一直接子节点标识，释放不可变借用。
    let content_id = tree
        .get(grid_id)
        .and_then(|grid| grid.children().first().copied())
        .expect("测试 Grid 必须包含内容节点");
    // 读取 Grid 收敛后的实际 frame。
    let grid_frame = tree.get(grid_id).expect("测试 Grid 节点必须存在").frame();
    // Auto 行必须由内容撑开到固定内容高度。
    assert!(
        grid_frame.h >= 24.0,
        "Grid 高度未由内容撑开: {grid_frame:?}"
    );
    // 读取 Grid 内容节点最终 frame。
    let content_frame = tree
        .get(content_id)
        .expect("测试 Grid 内容节点必须存在")
        .frame();
    // 内容必须获得非零且不小于其固有高度的布局 frame。
    assert!(
        content_frame.h >= 24.0,
        "Grid 内容未获得有效高度: {content_frame:?}"
    );
}

// 验证 Grid 移除最后一个子节点时立即清除固有轨道尺寸缓存。
#[test]
fn grid_last_child_removal_clears_intrinsic_cache() {
    // 构造由 Auto 轨道和固定内容撑开的无显式尺寸 Grid。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        // 单列单行都使用自然内容轨道。
        Grid::new()
            .columns(vec![GridTrack::Auto])
            .rows(vec![GridTrack::Auto]),
        // 唯一子项提供非零自然尺寸。
        vec![ViewNode::leaf(Label::new("content").size(60.0, 24.0))],
    ));
    // 完成首轮布局并记录 Grid 内容缓存。
    clear_initial_invalidations(&mut tree);
    // 读取有内容时的自然尺寸。
    let populated = root_measure(&tree);
    // Auto Grid 必须已被内容撑开。
    assert!(populated.w > 0.0 && populated.h > 0.0);

    // 原位协调为同轨道但没有任何子节点的 Grid。
    ViewAdapter::reconcile_nodes(
        &mut tree,
        // 保留轨道声明以隔离子节点移除路径。
        ViewNode::leaf(
            Grid::new()
                .columns(vec![GridTrack::Auto])
                .rows(vec![GridTrack::Auto]),
        ),
    );
    // 空 Grid 不得继续暴露旧内容尺寸。
    assert_eq!(root_measure(&tree), Size::zero());
}

// 验证 Card 的定制纵向布局继续遵守共享 LayoutChild 外边距契约。
#[test]
fn card_layout_consumes_child_margins() {
    // 构造无内部 padding 的固定尺寸 Card，隔离子项 margin 几何。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        // Card 的 body 与根 frame 重合，便于断言精确纵坐标。
        Card::new().size(200.0, 120.0).padding(0.0),
        // 两个固定高度子项分别声明不同的纵向外边距。
        vec![
            // 第一项应从顶部 margin 后开始。
            ViewNode::leaf(
                Container::new()
                    .size(20.0, 10.0)
                    .margin(EdgeInsets::new(0.0, 2.0, 0.0, 3.0)),
            ),
            // 第二项应同时被前一项底边距和自身顶边距推开。
            ViewNode::leaf(
                Container::new()
                    .size(20.0, 10.0)
                    .margin(EdgeInsets::new(0.0, 4.0, 0.0, 0.0)),
            ),
        ],
    ));
    // 运行 Card 定制布局。
    clear_initial_invalidations(&mut tree);
    // 复制根节点的直接子节点顺序。
    let children = tree
        .root()
        .expect("测试声明树必须存在 Card 根节点")
        .children()
        .to_vec();
    // 读取第一项最终 frame。
    let first = tree
        .get(children[0])
        .expect("测试 Card 第一项必须存在")
        .frame();
    // 读取第二项最终 frame。
    let second = tree
        .get(children[1])
        .expect("测试 Card 第二项必须存在")
        .frame();
    // 第一项起点必须包含自身顶部 margin。
    assert_eq!(first.y, 2.0);
    // 第二项起点必须包含前一项底部 margin 与自身顶部 margin。
    assert_eq!(second.y, 19.0);
}

// 验证 Affix 的手写堆叠同时消费子项 margin 并保留 border-box frame。
#[test]
fn affix_layout_consumes_child_margins() {
    // 构造不产生额外吸顶偏移的 Affix。
    let affix = Affix::new(0.0);
    // 构造第一项自然尺寸。
    let mut first = crate::ui::layout::LayoutChild::new(
        // 使用稳定测试标识。
        crate::core::ComponentId::new(1),
        // border-box 自然尺寸为二十乘十。
        Size::new(20.0, 10.0),
    );
    // 第一项声明左右与上下 margin。
    first.margin = EdgeInsets::new(1.0, 2.0, 2.0, 3.0);
    // 构造第二项相同自然尺寸。
    let mut second = crate::ui::layout::LayoutChild::new(
        // 使用不同稳定标识。
        crate::core::ComponentId::new(2),
        // 保持相同 border-box 尺寸以隔离 margin。
        Size::new(20.0, 10.0),
    );
    // 第二项只声明顶部 margin。
    second.margin = EdgeInsets::new(0.0, 4.0, 0.0, 0.0);
    // 空树足以覆盖不依赖真实祖先视口的自然布局分支。
    let tree = WidgetTree::new();
    // 在一百像素宽的父级 frame 中执行 Affix 定制布局。
    let positions = affix.layout_children(
        // 父级 frame 从原点开始且不触发吸顶补偿。
        Rect::new(0.0, 0.0, 100.0, 100.0),
        // 按声明顺序传入两个带 margin 子项。
        &[first, second],
        // 当前分支只在查找视口时读取空树并安全回退。
        &tree,
    );
    // 第一项横坐标包含左 margin。
    assert_eq!(positions[0].1.x, 1.0);
    // 第一项纵坐标包含顶部 margin。
    assert_eq!(positions[0].1.y, 2.0);
    // 第一项 border-box 宽度只扣除左右 margin。
    assert_eq!(positions[0].1.w, 97.0);
    // 第二项纵坐标包含第一项底 margin 与自身顶 margin。
    assert_eq!(positions[1].1.y, 19.0);
    // 缓存宽度使用最大自然外宽二十三像素。
    assert_eq!(
        affix.measure(Constraints::loose(Size::new(500.0, 500.0))).w,
        23.0
    );
    // 缓存高度包含两项 border-box 与全部纵向 margin。
    assert_eq!(
        affix.measure(Constraints::loose(Size::new(500.0, 500.0))).h,
        29.0
    );
}

// 验证嵌套根捕获只交接各自输出，且内层结束后外层继续捕获。
#[test]
// 执行嵌套捕获输出隔离回归。
fn capture_runtime_stack_isolates_nested_root_outputs() {
    // 创建外层构建开始前读取的独立状态。
    let outer_before = State::new(1_i32);
    // 创建内层构建读取的独立状态。
    let inner_state = State::new(2_i32);
    // 创建内层结束后外层继续读取的独立状态。
    let outer_after = State::new(3_i32);
    // 构建外层根并在其中嵌套构建另一个根。
    let outer = ViewAdapter::capture_root(|| {
        // 登记外层进入内层前的结构依赖。
        let _ = outer_before.get();
        // 登记外层进入内层前创建的副作用。
        let _ = crate::ui::Effect::new(|| {});
        // 构建并保留内层根以检查其独立输出。
        let inner = ViewAdapter::capture_root(|| {
            // 登记仅属于内层的结构依赖。
            let _ = inner_state.get();
            // 登记仅属于内层的副作用。
            let _ = crate::ui::Effect::new(|| {});
            // 返回最小内层声明根。
            ViewNode::leaf(Label::new("inner"))
        });
        // 内层必须只携带自己的一个 State 绑定。
        assert_eq!(inner.captured_state_binds.len(), 1);
        // 内层必须只携带自己的一个 Effect。
        assert_eq!(inner.captured_effects.len(), 1);
        // 内层完成后外层捕获帧必须仍处于活动状态。
        let _ = outer_after.get();
        // 登记外层恢复后创建的副作用。
        let _ = crate::ui::Effect::new(|| {});
        // 返回最小外层声明根。
        ViewNode::leaf(Label::new("outer"))
    });
    // 外层必须保留内层开始前后的两个 State 绑定。
    assert_eq!(outer.captured_state_binds.len(), 2);
    // 外层必须保留内层开始前后的两个 Effect。
    assert_eq!(outer.captured_effects.len(), 2);
}

// 验证构建 panic 时守卫只丢弃当前捕获帧，后续捕获仍保持干净可用。
#[test]
// 执行捕获栈异常恢复回归。
fn capture_runtime_stack_recovers_after_panicking_root_build() {
    // 创建将由失败根读取的状态以填充即将丢弃的帧。
    let failed_state = State::new(1_i32);
    // 捕获失败根的 panic，允许其 Drop 守卫完成帧清理。
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 构建会在登记 State 和 Effect 后失败的根。
        ViewAdapter::capture_root(|| {
            // 登记仅属于失败根的结构依赖。
            let _ = failed_state.get();
            // 登记仅属于失败根的副作用。
            let _ = crate::ui::Effect::new(|| {});
            // 模拟用户根工厂异常退出。
            panic!("capture stack test panic");
        });
    }));
    // 失败路径必须确实触发 panic。
    assert!(panic.is_err());
    // 创建后续根读取的独立状态。
    let recovered_state = State::new(2_i32);
    // 构建后续根以验证不存在失败帧残留。
    let recovered = ViewAdapter::capture_root(|| {
        // 登记后续根唯一的结构依赖。
        let _ = recovered_state.get();
        // 登记后续根唯一的副作用。
        let _ = crate::ui::Effect::new(|| {});
        // 返回最小恢复后声明根。
        ViewNode::leaf(Label::new("recovered"))
    });
    // 后续根不得继承失败根的 State 绑定。
    assert_eq!(recovered.captured_state_binds.len(), 1);
    // 后续根不得继承失败根的 Effect。
    assert_eq!(recovered.captured_effects.len(), 1);
}

// 验证动态捕获的子 View 在建树和原位协调后仍把 State 绑定到所属树。
#[test]
// 执行动态 State 结构绑定交接回归。
fn capture_runtime_dynamic_child_state_requests_reconcile_after_build_and_reconcile() {
    // 创建将由动态子节点结构读取的共享状态。
    let state = State::new(0_i32);
    // 建立不含声明子节点的稳定父根。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Container::new()));
    // 读取父根标识以通过动态协调入口插入子节点。
    let root_id = tree.root_id().expect("测试树必须拥有根节点");
    // 克隆状态供第一次动态子节点捕获读取。
    let first_state = state.clone();
    // 捕获第一次动态子节点，使其输出必须交接给 WidgetNode。
    let first = ViewAdapter::capture_root(|| {
        // 读取状态以登记结构性 reconcile 绑定。
        let _ = first_state.get();
        // 返回可原位复用的稳定类型子节点。
        ViewNode::leaf(Label::new("dynamic"))
    });
    // 把首次捕获子节点挂载到现有根下。
    ViewAdapter::reconcile_dynamic_children(&mut tree, root_id, vec![first]);
    // 清除建树阶段可能遗留的请求以观察后续 set。
    let _ = tree.take_reconcile_requested();
    // 修改被动态子节点读取的状态。
    state.set(1);
    // 成功挂载后的捕获绑定必须请求所属树协调。
    assert!(tree.take_reconcile_requested());
    // 克隆状态供同节点的第二次动态捕获读取。
    let second_state = state.clone();
    // 捕获与已有节点同类型的替换声明以进入 reconcile_existing。
    let second = ViewAdapter::capture_root(|| {
        // 再次读取状态以交接协调阶段的新绑定输出。
        let _ = second_state.get();
        // 保持同类型以验证原位协调不会丢失输出。
        ViewNode::leaf(Label::new("dynamic"))
    });
    // 协调现有动态子节点而不新建父树。
    ViewAdapter::reconcile_dynamic_children(&mut tree, root_id, vec![second]);
    // 清除协调过程可能产生的旧请求。
    let _ = tree.take_reconcile_requested();
    // 再次改变状态以验证协调后的节点仍绑定到树。
    state.set(2);
    // 新旧绑定都必须指向当前树的唯一 reconcile 请求端口。
    assert!(tree.take_reconcile_requested());
}

// 验证节点私有 Effect 不会清空根 Effect，且节点真实移除后不再被调度。
#[test]
// 执行根与节点 Effect 独立生命周期回归。
fn capture_runtime_dynamic_effects_preserve_root_and_release_on_remove() {
    // 创建根 Effect 将读取的状态。
    let root_state = State::new(0_i32);
    // 创建动态节点 Effect 将读取的状态。
    let child_state = State::new(0_i32);
    // 创建根 Effect 的运行次数观察器。
    let root_runs = Arc::new(AtomicUsize::new(0));
    // 创建节点 Effect 的运行次数观察器。
    let child_runs = Arc::new(AtomicUsize::new(0));
    // 克隆根状态供根捕获内 Effect 使用。
    let captured_root_state = root_state.clone();
    // 克隆根观察器供根 Effect 闭包使用。
    let captured_root_runs = Arc::clone(&root_runs);
    // 捕获声明根及其树级 Effect。
    let root = ViewAdapter::capture_root(|| {
        // 创建依赖根状态的 Effect。
        let _ = crate::ui::Effect::new(move || {
            // 读取状态以建立 Effect 依赖。
            let _ = captured_root_state.get();
            // 记录 Effect 的首次与后续执行。
            captured_root_runs.fetch_add(1, Ordering::Relaxed);
        });
        // 返回没有动态子节点的父根。
        ViewNode::leaf(Container::new())
    });
    // 建立拥有根 Effect 的树。
    let mut tree = ViewAdapter::build_nodes(root);
    // 读取父根标识以插入动态节点。
    let root_id = tree.root_id().expect("测试树必须拥有根节点");
    // 克隆节点状态供动态捕获内 Effect 使用。
    let captured_child_state = child_state.clone();
    // 克隆节点观察器供动态 Effect 闭包使用。
    let captured_child_runs = Arc::clone(&child_runs);
    // 捕获应由实际子节点拥有的 Effect。
    let child = ViewAdapter::capture_root(|| {
        // 创建依赖节点状态的 Effect。
        let _ = crate::ui::Effect::new(move || {
            // 读取状态以建立节点 Effect 依赖。
            let _ = captured_child_state.get();
            // 记录节点 Effect 的首次与后续执行。
            captured_child_runs.fetch_add(1, Ordering::Relaxed);
        });
        // 返回可作为动态子节点挂载的最小声明。
        ViewNode::leaf(Label::new("effect child"))
    });
    // 插入携带节点 Effect 的动态 View。
    ViewAdapter::reconcile_dynamic_children(&mut tree, root_id, vec![child]);
    // 同时修改根与节点 Effect 的依赖状态。
    root_state.set(1);
    // 触发节点拥有的 Effect。
    child_state.set(1);
    // 根与节点任一 pending Effect 都应让树报告待处理工作。
    assert!(tree.has_pending_effects());
    // 单次 tick 必须完整遍历根和节点 Effect。
    assert!(tree.tick_effects());
    // 根 Effect 初次构造和本次 tick 都应执行。
    assert_eq!(root_runs.load(Ordering::Relaxed), 2);
    // 节点 Effect 初次构造和本次 tick 都应执行。
    assert_eq!(child_runs.load(Ordering::Relaxed), 2);
    // 真实移除动态子节点，使其 BoxedWidget 与 Effect 一同释放。
    ViewAdapter::reconcile_dynamic_children(&mut tree, root_id, Vec::new());
    // 仅修改已移除节点先前读取的状态。
    child_state.set(2);
    // 已移除节点 Effect 不得再使树报告 pending。
    assert!(!tree.has_pending_effects());
    // 已移除节点 Effect 不得在树 tick 中执行。
    assert!(!tree.tick_effects());
    // 节点运行次数必须保持在移除前的值。
    assert_eq!(child_runs.load(Ordering::Relaxed), 2);
}

// 拆分捕获输出合并回归，保持适配器测试主体低于规模上限。
#[path = "adapter_tests/capture_merge.rs"]
// 编译预捕获节点输出合并回归模块。
mod capture_merge;

// 拆分结构性 State 租约生命周期回归，保持适配器测试主体低于规模上限。
#[path = "adapter_tests/state_bind_lifecycle.rs"]
// 编译 State 租约与 shutdown 生命周期回归模块。
mod state_bind_lifecycle;

// 拆分树级动态命名空间与动画源所有权回归，保持适配器测试主体低于规模上限。
#[path = "adapter_tests/dynamic_capture.rs"]
// 编译延迟 View 动态捕获基础契约的行为测试模块。
mod dynamic_capture;

// 将 fail-stop 状态机与受控关闭行为放入独立文件，避免主测试文件超过规模上限。
#[path = "adapter_tests/fail_stop.rs"]
// 挂载协调 panic 前后不同恢复语义的行为门禁。
mod fail_stop;
