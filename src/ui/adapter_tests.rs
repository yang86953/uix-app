// 导入被测的声明树适配器。
use super::ViewAdapter;
// 导入构造布局几何变化所需的边距类型。
use crate::core::EdgeInsets;
// 导入构造绘制变化所需的颜色常量。
use crate::draw::Color;
// 导入建立最小组件树与保守回退门禁所需的组件类型。
use crate::ui::widgets::{Button, Container, Grid, Label};
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
