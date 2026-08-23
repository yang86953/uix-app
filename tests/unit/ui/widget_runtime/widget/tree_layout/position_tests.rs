// 引入逻辑点和矩形用于固定验收表面与屏幕坐标。
use crate::core::{Point, Rect};
// 引入声明树到实际树的唯一适配边界。
use crate::ui::adapter::ViewAdapter;
// 引入实际组件 frame 的只读和测试写入契约。
use crate::ui::widget_runtime::widget::WidgetCore;
// 引入定位声明、基础组件与声明节点。
use crate::ui::{Container, PositionMode, ScrollView, Space, ViewNode};
// 引入纵向滚动模式。
use crate::platform::windowing::ScrollDirection;

// 用确定尺寸的空组件创建最小布局子项。
fn box_view(width: f32, height: f32) -> ViewNode {
    // 返回没有额外绘制策略的固定尺寸叶节点。
    ViewNode::leaf(Space::new().width(width).height(height))
}

// 给根节点写入确定客户区并执行一次真实布局。
fn layout_root(tree: &mut crate::ui::WidgetTree, frame: Rect) {
    // 取得已经发布的根身份。
    let root = tree.root_id().expect("定位测试根节点必须存在");
    // 根节点在布局前必须仍然存活。
    tree.get_mut(root)
        // 发布失败应立即终止测试。
        .expect("定位测试根节点必须可写")
        // 使用窗口等价的逻辑客户区隔离 bootstrap 测量。
        .set_frame(frame);
    // 执行组件树正式多阶段布局。
    tree.layout();
}

// ScrollView 必须以 flex-grow 内容的自然高度建立滚动范围，而不是读取其零 basis。
#[test]
fn scroll_view_measures_flex_grow_child_by_natural_content_size() {
    // 构造与 UIX <ScrollView><Column> 相同的唯一 flex-grow 内容节点。
    let content = ViewNode::new(
        // Column 默认扩张，但滚动轴仍应读取其自然内容高度。
        Container::new().flex_grow(1.0),
        // 两个固定高度子项共同超过视口。
        vec![box_view(100.0, 40.0), box_view(100.0, 50.0)],
    );
    // 使用五十像素高的纵向滚动视口承载九十像素自然内容。
    let root = ViewNode::new(
        ScrollView::new(ScrollDirection::Vertical).size(100.0, 50.0),
        vec![content],
    );
    // 发布并执行真实多阶段布局。
    let mut tree = ViewAdapter::build_nodes(root);
    layout_root(&mut tree, Rect::new(0.0, 0.0, 100.0, 50.0));
    // 读取根滚动组件的运行态范围。
    let scroll = tree
        .root()
        .expect("滚动根必须存在")
        .widget()
        .as_any()
        .downcast_ref::<ScrollView>()
        .expect("滚动根类型必须保持不变");
    // 九十像素内容相对五十像素视口必须产生四十像素滚动范围。
    assert_eq!(scroll.max_scroll_y(), 40.0);
}

// absolute 必须退出正常流，并按根包含块求解右上定位。
#[test]
fn position_runtime_absolute_leaves_flow_and_uses_root_block() {
    // 构造正常项、脱流项和后续正常项的垂直序列。
    let root = ViewNode::new(
        // 固定根尺寸使右侧 inset 具有确定结果。
        Container::new().size(200.0, 120.0),
        // 保存三个声明序子项。
        vec![
            // 首项占用十像素正常流高度。
            box_view(20.0, 10.0),
            // 中项相对根右上角定位且不应占槽。
            box_view(30.0, 15.0)
                // 退出父级正常流。
                .position(PositionMode::Absolute)
                // 距离根顶部十像素。
                .top(10.0)
                // 距离根右侧二十像素。
                .right(20.0),
            // 后项应紧随首项而非 absolute 项。
            box_view(20.0, 10.0),
        ],
    );
    // 通过真实适配器发布组件树。
    let mut tree = ViewAdapter::build_nodes(root);
    // 取得三个稳定实际身份。
    let children = tree
        // 根必须已经发布。
        .root()
        // 缺失根表示适配失败。
        .expect("absolute 根节点必须存在")
        // 复制直接子节点以跨越可变布局。
        .children()
        // 测试拥有身份集合。
        .to_vec();
    // 在确定客户区执行布局。
    layout_root(&mut tree, Rect::new(0.0, 0.0, 200.0, 120.0));
    // 读取脱流节点最终 border-box。
    let absolute = tree
        .get(children[1])
        .expect("absolute 节点必须存在")
        .frame();
    // 右侧二十像素与三十像素自然宽度应得水平坐标一百五十。
    assert_eq!(absolute, Rect::new(150.0, 10.0, 30.0, 15.0));
    // 后续正常项只消费首项十像素高度。
    assert_eq!(
        tree.get(children[2]).expect("后续节点必须存在").frame().y,
        10.0
    );
}

// relative 必须保留正常流槽位，同时移动自身视觉、命中与子树坐标。
#[test]
fn position_runtime_relative_preserves_slot_and_moves_visual_hit_box() {
    // 构造一个 relative 子项和一个正常流兄弟。
    let root = ViewNode::new(
        // 使用固定尺寸垂直容器。
        Container::new().size(100.0, 60.0),
        // 保存两个正常流参与者。
        vec![
            // 首项声明视觉偏移但继续占据原槽位。
            box_view(20.0, 10.0)
                // 开启 relative 语义。
                .position(PositionMode::Relative)
                // 向右移动十五像素。
                .left(15.0)
                // 向下移动七像素。
                .top(7.0),
            // 后项应继续从十像素处开始。
            box_view(20.0, 10.0),
        ],
    );
    // 发布实际组件树。
    let mut tree = ViewAdapter::build_nodes(root);
    // 复制两个子节点身份。
    let children = tree
        .root()
        .expect("relative 根必须存在")
        .children()
        .to_vec();
    // 执行真实布局。
    layout_root(&mut tree, Rect::new(0.0, 0.0, 100.0, 60.0));
    // 读取 relative 节点未移动的布局槽位。
    let raw = tree
        .get(children[0])
        .expect("relative 节点必须存在")
        .frame();
    // relative 不得改写正常流 frame。
    assert_eq!(raw, Rect::new(0.0, 0.0, 100.0, 10.0));
    // 兄弟继续使用原始槽位高度。
    assert_eq!(
        tree.get(children[1])
            .expect("relative 兄弟必须存在")
            .frame()
            .y,
        10.0
    );
    // 视觉矩形必须包含声明偏移。
    assert_eq!(
        // 通过合成器共用的树级视觉变换读取结果。
        tree.node_visual_rect(children[0], raw),
        // 期望视觉盒向右十五、向下七。
        Some(Rect::new(15.0, 7.0, 100.0, 10.0))
    );
    // 移动后的屏幕坐标必须命中 relative 节点。
    assert_eq!(tree.hit_test(Point::new(16.0, 8.0)), Some(children[0]));
}

// absolute 必须选择最近非 static 祖先，并继承该祖先的视觉偏移。
#[test]
fn position_runtime_absolute_uses_nearest_positioned_ancestor() {
    // 创建内部 absolute 子项。
    let absolute = box_view(20.0, 10.0)
        // 退出中间容器的正常流。
        .position(PositionMode::Absolute)
        // 相对包含块左侧十像素。
        .left(10.0)
        // 相对包含块顶部十二像素。
        .top(12.0);
    // 创建建立包含块且自身具有视觉偏移的中间容器。
    let positioned_parent = ViewNode::new(
        // 固定包含块尺寸。
        Container::new().size(100.0, 80.0),
        // 保存唯一 absolute 子节点。
        vec![absolute],
    )
    // relative 祖先建立 absolute 包含块。
    .position(PositionMode::Relative)
    // 包含块视觉上向右移动五十像素。
    .left(50.0)
    // 包含块视觉上向下移动二十像素。
    .top(20.0);
    // 构造更大的 static 根。
    let root = ViewNode::new(
        // 根尺寸故意不同于最近包含块。
        Container::new().size(300.0, 200.0),
        // 挂载定位祖先。
        vec![positioned_parent],
    );
    // 发布实际树。
    let mut tree = ViewAdapter::build_nodes(root);
    // 取得中间祖先身份。
    let parent = tree.root().expect("包含块根必须存在").children()[0];
    // 取得 absolute 后代身份。
    let child = tree.get(parent).expect("定位祖先必须存在").children()[0];
    // 执行真实布局。
    layout_root(&mut tree, Rect::new(0.0, 0.0, 300.0, 200.0));
    // absolute 的布局 frame 应以最近祖先原始 border-box 为坐标系。
    let raw = tree.get(child).expect("absolute 后代必须存在").frame();
    // 原始包含块位于零点，因此 inset 直接成为 frame 起点。
    assert_eq!(raw, Rect::new(10.0, 12.0, 20.0, 10.0));
    // 屏幕视觉矩形还必须继承 relative 祖先偏移。
    assert_eq!(
        // 读取实际合成路径结果。
        tree.node_visual_rect(child, raw),
        // 五十和二十像素祖先偏移叠加到子项坐标。
        Some(Rect::new(60.0, 32.0, 20.0, 10.0))
    );
}

// fixed 必须以根视口定位，并截断滚动和祖先裁剪。
#[test]
fn position_runtime_fixed_escapes_scroll_and_remains_hittable() {
    // 构造可滚动根及一个普通长内容和一个 fixed 子项。
    let root = ViewNode::new(
        // 初始滚动四十像素并隐藏滚动条以固定客户区宽度。
        ScrollView::new(ScrollDirection::Vertical)
            // 固定根视口。
            .size(100.0, 50.0)
            // 保留完整内容宽度。
            .show_scrollbar(false)
            // 建立非零纵向偏移。
            .scroll_to(0.0, 40.0),
        // 保存正常内容和 fixed 兄弟。
        vec![
            // 长内容建立有效滚动范围。
            box_view(100.0, 120.0),
            // fixed 项锚定根视口右上角。
            box_view(20.0, 10.0)
                // 提升到根合成坐标系。
                .position(PositionMode::Fixed)
                // 距离顶部五像素。
                .top(5.0)
                // 距离右侧五像素。
                .right(5.0),
        ],
    );
    // 发布实际树。
    let mut tree = ViewAdapter::build_nodes(root);
    // 复制普通和 fixed 身份。
    let children = tree.root().expect("fixed 根必须存在").children().to_vec();
    // 执行滚动视口布局。
    layout_root(&mut tree, Rect::new(0.0, 0.0, 100.0, 50.0));
    // 读取 fixed 原始 frame。
    let fixed = tree.get(children[1]).expect("fixed 节点必须存在").frame();
    // 根视口右五、上五应得确定 frame。
    assert_eq!(fixed, Rect::new(75.0, 5.0, 20.0, 10.0));
    // fixed 的视觉矩形不得减去四十像素滚动量。
    assert_eq!(tree.node_visual_rect(children[1], fixed), Some(fixed));
    // fixed 区域必须绕过滚动父级裁剪并参与命中。
    assert_eq!(tree.hit_test(Point::new(80.0, 8.0)), Some(children[1]));
}

// sticky 必须保留正常流槽位并在滚动后停靠最近视口 inset。
#[test]
fn position_runtime_sticky_tracks_nearest_scroll_viewport() {
    // 构造纵向滚动视口。
    let root = ViewNode::new(
        // 固定视口并建立三十像素滚动量。
        ScrollView::new(ScrollDirection::Vertical)
            // 固定验收表面。
            .size(100.0, 50.0)
            // 隐藏滚动条以避免沟槽影响断言。
            .show_scrollbar(false)
            // 让首项滚出正常位置。
            .scroll_to(0.0, 30.0),
        // 保存 sticky 头部和长内容。
        vec![
            // 头部继续占用十像素正常流高度。
            box_view(100.0, 10.0)
                // 开启 sticky 语义。
                .position(PositionMode::Sticky)
                // 停靠视口顶部五像素。
                .top(5.0),
            // 长内容保证滚动范围成立。
            box_view(100.0, 120.0),
        ],
    );
    // 发布实际树。
    let mut tree = ViewAdapter::build_nodes(root);
    // 取得 sticky 身份。
    let sticky = tree.root().expect("sticky 根必须存在").children()[0];
    // 执行滚动布局。
    layout_root(&mut tree, Rect::new(0.0, 0.0, 100.0, 50.0));
    // sticky 的原始 frame 仍位于正常流顶部。
    let raw = tree.get(sticky).expect("sticky 节点必须存在").frame();
    // 正常流槽位不得被视觉停靠改写。
    assert_eq!(raw.y, 0.0);
    // 滚动后的视觉盒应稳定停靠视口顶部五像素。
    assert_eq!(
        // 读取与绘制及命中共用的最终矩形。
        tree.node_visual_rect(sticky, raw),
        // 期望纵坐标为五。
        Some(Rect::new(0.0, 5.0, 100.0, 10.0))
    );
}

// reconcile 切换定位模式必须保留身份并触发正常流重新求解。
#[test]
fn position_runtime_reconcile_preserves_identity_and_reflows_sibling() {
    // 构造可复用的初始声明树。
    let initial = ViewNode::new(
        // 固定根尺寸。
        Container::new().size(100.0, 80.0),
        // 保存带 key 的目标和普通兄弟。
        vec![
            // 稳定 key 允许协调原位更新。
            box_view(20.0, 10.0).key("moving"),
            // 第二项最初位于十像素纵坐标。
            box_view(20.0, 10.0).key("sibling"),
        ],
    );
    // 发布初始实际树。
    let mut tree = ViewAdapter::build_nodes(initial);
    // 复制协调前稳定身份。
    let children = tree.root().expect("协调根必须存在").children().to_vec();
    // 完成初始布局。
    layout_root(&mut tree, Rect::new(0.0, 0.0, 100.0, 80.0));
    // 初始兄弟应位于首项之后。
    assert_eq!(
        tree.get(children[1]).expect("初始兄弟必须存在").frame().y,
        10.0
    );
    // 构造相同身份但把首项切换为 absolute 的下一声明树。
    let next = ViewNode::new(
        // 保持根组件类型与尺寸不变。
        Container::new().size(100.0, 80.0),
        // 保持 key 顺序并只修改定位元数据。
        vec![
            // 目标原位切换为脱流定位。
            box_view(20.0, 10.0)
                // 保留稳定身份。
                .key("moving")
                // 切换到 absolute。
                .position(PositionMode::Absolute)
                // 明确新纵坐标。
                .top(20.0),
            // 未变化兄弟保持相同 key。
            box_view(20.0, 10.0).key("sibling"),
        ],
    );
    // 通过正式协调事务更新实际树。
    ViewAdapter::reconcile_nodes(&mut tree, next);
    // 根客户区在协调后继续模拟同一窗口。
    layout_root(&mut tree, Rect::new(0.0, 0.0, 100.0, 80.0));
    // 目标与兄弟身份都不得因结构元数据变化而重建。
    assert_eq!(
        tree.root().expect("协调后根必须存在").children(),
        children.as_slice()
    );
    // absolute 项应移动到声明位置。
    assert_eq!(
        tree.get(children[0]).expect("协调目标必须存在").frame().y,
        20.0
    );
    // 兄弟应回流到父级首个槽位。
    assert_eq!(
        tree.get(children[1]).expect("协调兄弟必须存在").frame().y,
        0.0
    );
    // 构造同一 absolute 身份把 top 单边恢复为 auto 的下一声明树。
    let cleared = ViewNode::new(
        // 保持根组件类型与尺寸不变。
        Container::new().size(100.0, 80.0),
        // 保持 key 顺序并只清除目标顶边。
        vec![
            // 目标继续脱流但不再拥有显式纵向 inset。
            box_view(20.0, 10.0)
                // 保留稳定身份。
                .key("moving")
                // 保持 absolute 模式。
                .position(PositionMode::Absolute)
                // 先建立像素值以验证 auto 确实清除当前边。
                .top(20.0)
                // 单边 auto 不应影响模式或其他边。
                .top_auto(),
            // 未变化兄弟继续保持身份。
            box_view(20.0, 10.0).key("sibling"),
        ],
    );
    // 通过正式协调事务发布单边 auto。
    ViewAdapter::reconcile_nodes(&mut tree, cleared);
    // 在同一窗口表面重新布局。
    layout_root(&mut tree, Rect::new(0.0, 0.0, 100.0, 80.0));
    // auto 轴应回退到根包含块起点。
    assert_eq!(
        tree.get(children[0])
            .expect("清除顶边后的目标必须存在")
            .frame()
            .y,
        0.0
    );
}
