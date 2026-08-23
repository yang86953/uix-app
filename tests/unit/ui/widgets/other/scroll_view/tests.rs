// 导入 ScrollView 私有布局状态与核心几何类型。
use super::*;
// 导入构造子项外边距所需的边集类型。
use crate::core::EdgeInsets;
// 导入直接调用组件布局入口所需的 trait。
use crate::ui::{WidgetLayout, WidgetRender};

// 验证滚动条覆盖绘制阶段会被场景树实际调度。
#[test]
fn scrollbar_declares_after_children_paint_capability() {
    // 滚动条必须越过内容裁剪后叠加，不能只在未调度的 render 分支中实现。
    let scroll = ScrollView::new(ScrollDirection::Vertical);
    // 场景桥接只依据此能力决定是否执行 AfterChildren。
    assert!(WidgetRender::paint_after_children(&scroll));
}

// 验证内容缩短后，视口与公开偏移立即收敛到新的内容末端。
#[test]
fn content_shrink_clamps_effective_scroll_offset() {
    // 先构造可向下滚动八十像素的视口并移动到底部。
    let mut scroll = ScrollView::new(ScrollDirection::Vertical);
    scroll
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 100.0, 80.0)));
    scroll.content_bounds.set(Some(Size::new(100.0, 160.0)));
    scroll.set_scroll_y(80.0);
    // 下一轮内容只有四十像素高，布局应把有效偏移同步回零。
    let children = [child(1, Size::new(100.0, 40.0), EdgeInsets::zero())];
    let tree = WidgetTree::new();
    scroll.layout_children(Rect::new(0.0, 0.0, 100.0, 80.0), &children, &tree);
    // 公开位置与场景视口变换必须共同观察零偏移。
    assert_eq!(scroll.scroll_y(), 0.0);
    assert_eq!(
        crate::ui::EventHandler::viewport_scroll_offset(&scroll),
        Some((0.0, 0.0))
    );
}

// 构造带自然尺寸与外边距的布局子项。
fn child(id: usize, size: Size, margin: EdgeInsets) -> LayoutChild {
    // 创建指定标识与自然尺寸的子项描述。
    let mut child = LayoutChild::new(WidgetId::new(id), size);
    // 写入应由父滚动容器消费的外边距。
    child.margin = margin;
    // 返回完整子项描述。
    child
}

// 验证纵向滚动把四侧 margin 纳入放置、沟槽判断与内容范围。
#[test]
fn vertical_flow_consumes_child_margins() {
    // 构造显示滚动条的纵向视口。
    let scroll = ScrollView::new(ScrollDirection::Vertical);
    // 使用非零原点核对内容坐标不会退回局部原点。
    let frame = Rect::new(10.0, 20.0, 100.0, 70.0);
    // 模拟渲染阶段记录的局部视口尺寸，供最大滚动距离计算。
    scroll
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
    // 两个子项的自然高度恰好等于视口，只有 margin 会触发纵向溢出。
    let children = [
        // 首项外尺寸高度为五十像素。
        child(
            1,
            Size::new(20.0, 40.0),
            EdgeInsets::new(2.0, 3.0, 8.0, 7.0),
        ),
        // 次项外尺寸高度为四十四像素。
        child(
            2,
            Size::new(20.0, 30.0),
            EdgeInsets::new(4.0, 5.0, 6.0, 9.0),
        ),
    ];
    // 空树足以覆盖所有子项自然高度为正的布局路径。
    let tree = WidgetTree::new();
    // 执行纵向滚动内容排布。
    let positions = scroll.layout_children(frame, &children, &tree);
    // 纵向滚动条占八像素，首项再扣左右 margin 后宽八十二像素。
    assert_eq!(positions[0].1, Rect::new(12.0, 23.0, 82.0, 40.0));
    // 次项必须被首项完整外尺寸推开，并应用自身起始 margin。
    assert_eq!(positions[1].1, Rect::new(14.0, 75.0, 82.0, 30.0));
    // 九十四像素外高度相对七十像素视口留下二十四像素滚动范围。
    assert_eq!(scroll.max_scroll_y(), 24.0);
}

// 验证横向滚动对 margin 的处理与纵向流保持轴对称。
#[test]
fn horizontal_flow_consumes_child_margins() {
    // 构造显示滚动条的横向视口。
    let scroll = ScrollView::new(ScrollDirection::Horizontal);
    // 使用非零原点与七十像素宽视口。
    let frame = Rect::new(10.0, 20.0, 70.0, 100.0);
    // 模拟渲染阶段记录的局部视口尺寸。
    scroll
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
    // 两个子项的自然宽度恰好等于视口，只有 margin 会触发横向溢出。
    let children = [
        // 首项外尺寸宽度为五十像素。
        child(
            1,
            Size::new(40.0, 20.0),
            EdgeInsets::new(3.0, 2.0, 7.0, 8.0),
        ),
        // 次项外尺寸宽度为四十四像素。
        child(
            2,
            Size::new(30.0, 20.0),
            EdgeInsets::new(5.0, 4.0, 9.0, 6.0),
        ),
    ];
    // 空树足以覆盖所有自然尺寸为正的布局路径。
    let tree = WidgetTree::new();
    // 执行横向滚动内容排布。
    let positions = scroll.layout_children(frame, &children, &tree);
    // 横向滚动条占八像素，首项再扣上下 margin 后高八十二像素。
    assert_eq!(positions[0].1, Rect::new(13.0, 22.0, 40.0, 82.0));
    // 次项必须被首项完整外尺寸推开，并应用自身起始 margin。
    assert_eq!(positions[1].1, Rect::new(65.0, 24.0, 30.0, 82.0));
    // 九十四像素外宽相对七十像素视口留下二十四像素滚动范围。
    assert_eq!(scroll.max_scroll_x(), 24.0);
}

// 验证双向溢出时滚动范围使用扣除对侧滚动条后的真实视口。
#[test]
fn dual_axis_scroll_range_accounts_for_opposite_gutters() {
    // 构造默认显示双向滚动条的视口。
    let scroll = ScrollView::new(ScrollDirection::Both);
    // 使用一百乘一百的稳定视口。
    let frame = Rect::new(0.0, 0.0, 100.0, 100.0);
    // 模拟渲染阶段记录的局部视口尺寸。
    scroll.last_frame.set(Some(frame));
    // 单个一百五十像素方形子项同时触发两个方向溢出。
    let children = [child(1, Size::new(150.0, 150.0), EdgeInsets::zero())];
    // 空树足以覆盖自然尺寸为正的布局路径。
    let tree = WidgetTree::new();
    // 执行双向滚动内容排布。
    let positions = scroll.layout_children(frame, &children, &tree);
    // 双向滚动必须保留子项的自然内容尺寸。
    assert_eq!(positions[0].1, Rect::new(0.0, 0.0, 150.0, 150.0));
    // 横向范围应按扣除纵向沟槽后的九十二像素视口计算。
    assert_eq!(scroll.max_scroll_x(), 50.0 + ScrollBar::gutter());
    // 纵向范围应按扣除横向沟槽后的九十二像素视口计算。
    assert_eq!(scroll.max_scroll_y(), 50.0 + ScrollBar::gutter());
}

// 验证病理视口、自然尺寸与 margin 不会污染最终滚动几何。
#[test]
fn pathological_geometry_is_normalized_before_layout_output() {
    // 构造双向滚动视口以覆盖两个轴的有限化路径。
    let scroll = ScrollView::new(ScrollDirection::Both);
    // 注入非有限坐标、无界哨兵和 NaN 尺寸。
    let frame = Rect::new(f32::INFINITY, f32::NEG_INFINITY, f32::MAX, f32::NAN);
    // 最大滚动范围也必须防御渲染阶段保存的病理 frame。
    scroll.last_frame.set(Some(frame));
    // 构造同时携带非法自然尺寸与四侧 margin 的子项。
    let children = [child(
        1,
        Size::new(f32::INFINITY, f32::MAX),
        EdgeInsets::new(f32::NAN, f32::INFINITY, f32::MAX, f32::NEG_INFINITY),
    )];
    // 空树足以覆盖自然高度被归零后的稳定回退路径。
    let tree = WidgetTree::new();
    // 执行病理输入布局。
    let positions = scroll.layout_children(frame, &children, &tree);
    // 读取唯一子项的最终 frame。
    let child_frame = positions[0].1;
    // 坐标与尺寸都必须有限。
    for value in [child_frame.x, child_frame.y, child_frame.w, child_frame.h] {
        // 禁止任何非有限值进入布局树。
        assert!(value.is_finite());
    }
    // 实际尺寸还必须保持非负。
    assert!(child_frame.w >= 0.0 && child_frame.h >= 0.0);
    // 读取组件缓存的最终内容范围。
    let bounds = scroll.content_bounds.get().expect("layout stores bounds");
    // 内容范围两个轴都必须有限非负。
    assert!(bounds.w.is_finite() && bounds.w >= 0.0);
    // 纵向内容范围同样必须有限非负。
    assert!(bounds.h.is_finite() && bounds.h >= 0.0);
    // 横向最大滚动距离不得传播病理值。
    assert!(scroll.max_scroll_x().is_finite());
    // 纵向最大滚动距离不得传播病理值。
    assert!(scroll.max_scroll_y().is_finite());
}

// 验证受控偏移同步会产生一次可由树级合成器消费的滚动增量。
#[test]
// 使用稳定测试名锁定声明式 State 与局部纹理搬移之间的组件契约。
fn controlled_offset_sync_records_one_scroll_delta() {
    // 构造已经完成首帧布局的纵向滚动视口。
    let mut current = ScrollView::new(ScrollDirection::Vertical);
    // 固定一百乘八十的真实视口，使最大滚动范围可确定。
    current
        // 写入渲染阶段保存的局部视口。
        .last_frame
        // 模拟首帧已经完成布局。
        .set(Some(Rect::new(0.0, 0.0, 100.0, 80.0)));
    // 模拟一百六十像素高的内容，允许向下滚动八十像素。
    current.content_bounds.set(Some(Size::new(100.0, 160.0)));
    // 创建由声明式视图拥有的受控滚动位置。
    let offset = State::new(Point::new(0.0, 40.0));
    // 构造下一轮声明组件并绑定新的受控偏移。
    let next = ScrollView::new(ScrollDirection::Vertical).scroll_offset(&offset);
    // 执行与 ViewAdapter 原位协调相同的组件同步入口。
    current.sync_from(next);
    // live 组件必须接纳经过内容范围夹取的受控垂直偏移。
    assert_eq!(current.scroll_y(), 40.0);
    // 第一次消费必须得到从旧位置到新位置的精确增量。
    assert_eq!(
        // 通过事件能力契约读取树级脏区合成将要消费的位移。
        crate::ui::EventHandler::scroll_delta_for_dirty(&current),
        // 垂直向下四十像素对应同值逻辑增量。
        Some((0.0, 40.0)),
    );
    // 增量是一次性事务事实，不能在下一帧重复搬移旧像素。
    assert_eq!(
        // 再次读取同一能力必须已经为空。
        crate::ui::EventHandler::scroll_delta_for_dirty(&current),
        // 已消费状态不再产生滚动记录。
        None,
    );
}
