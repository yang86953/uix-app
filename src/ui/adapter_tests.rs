// 导入被测的声明树适配器。
use super::ViewAdapter;
// 导入构造布局几何变化、滚动偏移与固有尺寸断言所需的核心类型。
use crate::core::{Constraints, EdgeInsets, Point, Size};
// 导入构造绘制变化所需的颜色常量。
use crate::draw::Color;
// 导入建立最小组件树、缓存门禁与保守回退门禁所需的组件类型。
use crate::ui::widgets::{Affix, Button, Carousel, Container, Grid, Label, ScrollView, Space};
// 导入构造纵向滚动视口所需的方向类型。
use crate::ui::{ScrollDirection, State};
// 导入构造 Grid 轨道几何变化所需的轨道类型。
use crate::ui::GridTrack;
// 导入把叶组件包装成声明节点所需的节点类型。
use crate::ui::view::ViewNode;
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

// 验证尚未精细审计的组件配置变化继续保守请求布局。
#[test]
fn unclassified_widget_change_conservatively_invalidates_layout() {
    // 构建仍走保守分类的初始按钮树。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Button::new("short")));
    // 清除初始建树与布局失效。
    clear_initial_invalidations(&mut tree);

    // 改变尚未纳入精细字段分类的按钮文本。
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Button::new("a much longer button")),
    );

    // 未分类配置变化必须同时请求 Layout 与 Paint，不能误吞布局工作。
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
