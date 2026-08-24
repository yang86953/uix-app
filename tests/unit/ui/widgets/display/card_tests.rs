// 导入 Card 私有实现与布局辅助类型。
use super::*;
// 导入调用组件布局契约所需的 trait。
use crate::ui::WidgetLayout;
// 导入声明视图到实际组件树的正式适配入口。
use crate::ui::adapter::ViewAdapter;
// 导入读取与写入测试布局 frame 的组件核心接口。
use crate::ui::widget_runtime::widget::WidgetCore;
// 导入构造真实 Card 子树所需的公开组件与声明节点。
use crate::ui::{Container, Space, ViewNode};
// 导入公开 View 构建入口以验证同目录 UIX 根。
use crate::ui::view::View;

// 验证 UIX 根桥接保持 Card 动态类型与拥有型 View 子树形状。
#[test]
fn uix_root_preserves_card_kernel_and_children() {
    // 构造一个真实 View 子节点并经代码生成桥接进入 Card UIX 根。
    let child = ViewNode::leaf(Space::new().width(40.0).height(20.0));
    let node = Card::new()
        .title("概览")
        .build_view_with_children(vec![child]);
    // UIX 声明不得增加额外包装或复制子节点。
    assert_eq!(node.children.len(), 1);
    // 根动态类型必须继续是拥有布局、交互与绘制机制的 Card。
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Card>()
        .expect("UIX 根必须保留 Card 内核");
    assert_eq!(kernel.title.as_deref(), Some("概览"));
    // UIX 声明必须进入真实绘制与布局内核。
    assert_eq!(
        kernel.visual_contract_for_test(),
        (200.0, 120.0, 16.0, 56.0, 15.0, 40.0, 13.0, 0.05)
    );
    // 公开 View 入口的空卡片同样保持零子节点形状。
    assert!(View::build(Card::new()).children.is_empty());
}

// UIX 默认外观只填充未显式覆写字段，且实例共享静态视觉表。
#[test]
fn uix_defaults_preserve_authored_card_appearance_and_share_visuals() {
    let authored = View::build(Card::new().bordered(false).padding(24.0).elevation(3));
    let defaults = View::build(Card::new());
    let authored = authored
        .widget
        .as_any()
        .downcast_ref::<Card>()
        .expect("作者 Card 必须保留内核");
    let defaults = defaults
        .widget
        .as_any()
        .downcast_ref::<Card>()
        .expect("默认 Card 必须保留内核");
    assert_eq!(
        (authored.bordered, authored.padding, authored.elevation),
        (false, 24.0, 3)
    );
    assert_eq!(
        (defaults.bordered, defaults.padding, defaults.elevation),
        (true, 16.0, 1)
    );
    assert!(authored.shares_visual_with_for_test(defaults));
}

// 验证自动高度由未压缩的 body 子树撑开，而显式高度仍是硬约束。
#[test]
fn auto_height_tracks_body_content_but_fixed_height_does_not() {
    // 创建与首页卡片相同的定宽、自动高度配置。
    let card = Card::new().title("快捷导航").size(220.0, 0.0);
    // 构造高度超过默认 body 可用高度的自然尺寸子项。
    let child = LayoutChild::new(WidgetId::new(1), Size::new(188.0, 102.0));
    // 布局入口不读取树节点，因此空树足以承载本次组件契约测试。
    let tree = WidgetTree::new();
    // 用旧默认高度执行 bootstrap 布局，模拟首次展示。
    let positions = WidgetLayout::layout_children(
        // 布局目标是待验证的自动高度卡片。
        &card,
        // 二百二十宽卡片扣除内边距后恰好得到一百八十八宽 body。
        Rect::new(0.0, 0.0, 220.0, DEFAULT_CARD_VISUAL.defaults.height),
        // 单个子项代表首页卡片中的垂直 Column。
        &[child],
        // 本测试不需要访问运行时树内容。
        &tree,
    );
    // 自动高度主轴不得把一百零二高的子项压回四十八高 body。
    assert_eq!(positions[0].1.h, 102.0);
    // 下一轮测量必须包含标题区、真实 body 高度和底部内边距。
    assert_eq!(
        // 通过公开布局契约测量，而不是绕过组件责任读取私有 helper。
        WidgetLayout::measure(&card, Constraints::unconstrained()),
        // 五十六加一百零二加十六得到一百七十四逻辑像素。
        Size::new(220.0, 174.0),
    );

    // 创建同尺寸但显式锁定一百二十高度的卡片。
    let fixed = Card::new().title("固定高度").size(220.0, 120.0);
    // 注入相同内容缓存，验证内容不能改写显式产品契约。
    fixed
        // 直接设置组件拥有的布局缓存。
        .cached_content_size
        // 使用与自动高度分支相同的 body 内容尺寸。
        .set(Size::new(188.0, 102.0));
    // 固定高度继续保持一百二十，不被溢出内容反向撑开。
    assert_eq!(
        // 沿用相同的无界父约束以隔离 Card 自身行为。
        WidgetLayout::measure(&fixed, Constraints::unconstrained()),
        // 显式宽高必须原样保留。
        Size::new(220.0, 120.0),
    );
}

// 验证 UIX 等价声明中的增长型 Column 能通过自然测量撑开 Card。
#[test]
fn view_card_expands_for_growing_column_content() {
    // 构造三个确定高度的内容块，隔离字体度量差异。
    let content = crate::ui::column(vec![
        // 第一项模拟首页按钮或标题行。
        ViewNode::leaf(Space::new().width(188.0).height(30.0)),
        // 第二项模拟首页按钮或说明行。
        ViewNode::leaf(Space::new().width(188.0).height(30.0)),
        // 第三项模拟首页按钮或标签行。
        ViewNode::leaf(Space::new().width(188.0).height(30.0)),
    ])
    // 两个八像素间距把 Column 自然高度扩展到一百零六。
    .gap(8.0);
    // 使用与 UIX Card 代码生成相同的 ViewNode 尺寸样式入口。
    let card = ViewNode::new(
        // Card 自身只声明标题，不声明固定高度。
        Card::new().title("快捷导航"),
        // 默认 Column 带 flex-grow，正是本次回归目标。
        vec![content],
    )
    // UIX 的 width 属性必须由 Adapter 传递给 Card。
    .width(220.0);
    // 首页实际把两个 Card 放在自动高度 Row 中，本测试保留相同交叉轴 Stretch。
    let cards = crate::ui::row(vec![card]);
    // 固定根客户区，避免 bootstrap 根尺寸参与断言。
    let root = ViewNode::new(
        // 垂直根容器提供足够的确定布局空间。
        Container::new()
            // 使用垂直正常流放置 Card。
            .dir(FlexDirection::Column)
            // 锁定测试客户区尺寸。
            .size(600.0, 400.0),
        // 根只承载一个自动高度 Row。
        vec![cards],
    );
    // 通过正式 ViewAdapter 建立实际组件树。
    let mut tree = ViewAdapter::build_nodes(root);
    // 读取已经发布的根节点身份。
    let root_id = tree.root_id().expect("Card 布局测试必须存在根节点");
    // 给根节点写入与声明一致的窗口客户区。
    tree.get_mut(root_id)
        // 根节点发布失败应立即终止测试。
        .expect("Card 布局测试根节点必须可写")
        // 使用窗口等价的确定 frame。
        .set_frame(Rect::new(0.0, 0.0, 600.0, 400.0));
    // 执行正式多阶段布局收敛。
    tree.layout();
    // 根的唯一直接子节点是首页等价的自动高度 Row。
    let row_id = tree
        // 根节点必须继续存活。
        .get(root_id)
        // 缺失根表示布局错误。
        .expect("Card 布局测试根节点必须存在")
        // 读取唯一 Row 子项身份。
        .children()[0];
    // Row 的唯一直接子节点才是待测 Card。
    let card_id = tree
        // 自动高度 Row 必须继续存活。
        .get(row_id)
        // 缺失 Row 表示声明适配失败。
        .expect("Card 布局测试 Row 必须存在")
        // 读取唯一 Card 子项身份。
        .children()[0];
    // 根级 Stretch 应把自动 Row 扩展到完整客户区，而不是收缩成自然内容宽度。
    assert_eq!(
        // 读取自动 Row 在最终布局阶段获得的父级分配宽度。
        tree.get(row_id)
            // 缺失 Row 表示布局树身份发生漂移。
            .expect("Card 布局测试 Row 必须存在")
            // 最终 frame 才是绘制与命中的共同几何事实。
            .frame()
            // 本断言只关心交叉轴宽度。
            .w,
        // 根客户区的六百宽必须完整保留。
        600.0,
    );
    // 读取 Card 最终 border-box。
    let card_frame = tree
        // Card 必须由 ViewAdapter 实际物化。
        .get(card_id)
        // 缺失 Card 表示声明适配失败。
        .expect("Card 节点必须存在")
        // 读取布局收敛后的最终几何。
        .frame();
    // ViewNode 的 width 属性必须应用为二百二十逻辑像素。
    assert_eq!(card_frame.w, 220.0);
    // 标题五十六、内容一百零六和底边距十六合计一百七十八。
    assert_eq!(card_frame.h, 178.0);
}
