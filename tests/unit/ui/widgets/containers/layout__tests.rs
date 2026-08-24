// 引入当前模块组件与共享布局类型。
use super::*;
// 引入运行时布局 trait 以调用组件入口。
use crate::ui::WidgetLayout;
// 比较构建前后的视觉引用是否保持 UIX 生成单源。
use std::ptr;

// 验证五个布局公开组件的构建前默认值与构建后节点共享 UIX 视觉。
#[test]
fn layout_family_uses_colocated_uix_visuals() {
    // Layout 构造与 UIX 根构建都必须复用同一静态视觉。
    let layout = Layout::new();
    assert!(ptr::eq(layout.visual, LAYOUT_VISUAL_REF));
    let layout_node = View::build(layout);
    let layout = layout_node
        .widget
        .as_any()
        .downcast_ref::<Layout>()
        .expect("UIX 根应保留 Layout 内核");
    assert!(ptr::eq(layout.visual, LAYOUT_VISUAL_REF));

    // Header 默认高度与标题排版只能来自同目录 UIX 视觉。
    let header = Header::default();
    assert!(ptr::eq(header.visual, super::header::HEADER_VISUAL_REF));
    let header_node = View::build(header);
    let header = header_node
        .widget
        .as_any()
        .downcast_ref::<Header>()
        .expect("UIX 根应保留 Header 内核");
    assert!(ptr::eq(header.visual, super::header::HEADER_VISUAL_REF));

    // Sider 展开与折叠宽度只能来自同目录 UIX 视觉。
    let sider = Sider::default();
    assert!(ptr::eq(sider.visual, super::sider::SIDER_VISUAL_REF));
    let sider_node = View::build(sider);
    let sider = sider_node
        .widget
        .as_any()
        .downcast_ref::<Sider>()
        .expect("UIX 根应保留 Sider 内核");
    assert!(ptr::eq(sider.visual, super::sider::SIDER_VISUAL_REF));

    // Content 的弹性策略由同目录 UIX 视觉唯一拥有。
    let content = Content::default();
    assert!(ptr::eq(content.visual, super::content::CONTENT_VISUAL_REF));
    let content_node = View::build(content);
    let content = content_node
        .widget
        .as_any()
        .downcast_ref::<Content>()
        .expect("UIX 根应保留 Content 内核");
    assert!(ptr::eq(content.visual, super::content::CONTENT_VISUAL_REF));

    // Footer 默认高度与子树方向由同目录 UIX 视觉唯一拥有。
    let footer = Footer::default();
    assert!(ptr::eq(footer.visual, super::footer::FOOTER_VISUAL_REF));
    let footer_node = View::build(footer);
    let footer = footer_node
        .widget
        .as_any()
        .downcast_ref::<Footer>()
        .expect("UIX 根应保留 Footer 内核");
    assert!(ptr::eq(footer.visual, super::footer::FOOTER_VISUAL_REF));
}

// 验证默认纵向布局分配固定区域与可增长内容。
#[test]
fn layout_default_column_uses_shared_flex_distribution() {
    // 构造默认纵向布局壳。
    let layout = Layout::new();
    // 构造固定四十八像素 Header。
    let header = LayoutChild::new(WidgetId::new(1), Size::new(0.0, 48.0));
    // 构造可增长 Content。
    let mut content = LayoutChild::new(WidgetId::new(2), Size::zero());
    // 声明内容占用剩余空间。
    content.flex_grow = 1.0;
    // 构造固定四十八像素 Footer。
    let footer = LayoutChild::new(WidgetId::new(3), Size::new(0.0, 48.0));
    // 空树满足不读取树的排列签名。
    let tree = WidgetTree::new();
    // 在三百像素高区域中执行共享 Flex 排列。
    let positions = layout.layout_children(
        // 提供确定父区域。
        Rect::new(0.0, 0.0, 400.0, 300.0),
        // 保留 Header、Content、Footer 来源顺序。
        &[header, content, footer],
        // 传入空树。
        &tree,
    );
    // Header 保持固定高度并横向拉伸。
    assert_eq!(positions[0].1, Rect::new(0.0, 0.0, 400.0, 48.0));
    // Content 获得扣除上下固定区域后的剩余高度。
    assert_eq!(positions[1].1, Rect::new(0.0, 48.0, 400.0, 204.0));
    // Footer 位于底部并保持固定高度。
    assert_eq!(positions[2].1, Rect::new(0.0, 252.0, 400.0, 48.0));
}

// 验证横向 Layout 为 Sider 与嵌套 Layout 分配剩余宽度。
#[test]
fn layout_row_distributes_sider_and_nested_shell() {
    // 显式选择横向应用壳。
    let layout = Layout::new().direction(FlexDirection::Row);
    // 构造固定二百像素 Sider。
    let sider = LayoutChild::new(WidgetId::new(1), Size::new(200.0, 0.0));
    // 构造可增长的嵌套 Layout。
    let mut nested = LayoutChild::new(WidgetId::new(2), Size::zero());
    // 声明嵌套壳占用剩余空间。
    nested.flex_grow = 1.0;
    // 空树满足不读取树的排列签名。
    let tree = WidgetTree::new();
    // 在六百像素宽区域中执行共享 Flex 排列。
    let positions = layout.layout_children(
        // 提供确定父区域。
        Rect::new(0.0, 0.0, 600.0, 400.0),
        // 保持 Sider 在嵌套壳之前。
        &[sider, nested],
        // 传入空树。
        &tree,
    );
    // Sider 保持固定宽度并纵向拉伸。
    assert_eq!(positions[0].1, Rect::new(0.0, 0.0, 200.0, 400.0));
    // 嵌套壳获得其余四百像素宽度。
    assert_eq!(positions[1].1, Rect::new(200.0, 0.0, 400.0, 400.0));
}
