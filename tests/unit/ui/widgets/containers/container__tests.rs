// 导入当前模块的容器与布局类型。
use super::*;
// 导入调用布局 trait 所需的公开接口。
use crate::ui::WidgetLayout;
// 导入调用子树裁剪契约所需的渲染 trait。
use crate::ui::WidgetRender;
// 导入公开声明式样式扩展以验证完整阴影桥接。
use crate::ui::StyleExt;
// 导入声明视图展开为真实组件节点所需的转换接口。
use crate::ui::IntoWidgetNode;

// 验证内容缓存包含子项尾侧 margin。
#[test]
fn cached_content_size_includes_trailing_margins() {
    // 构造无固定尺寸的水平容器。
    let container = Container::new().dir(FlexDirection::Row);
    // 构造二十乘十的自然尺寸子项。
    let mut child = LayoutChild::new(WidgetId::new(1), Size::new(20.0, 10.0));
    // 四侧 margin 使自然外尺寸达到三十乘二十。
    child.margin = EdgeInsets::new(2.0, 3.0, 8.0, 7.0);
    // 空树足以满足无树读取的布局入口。
    let tree = WidgetTree::new();
    // 在零尺寸 bootstrap frame 中执行组件布局。
    let positions = container.layout_children(Rect::zero(), &[child], &tree);
    // 子项应从左上 margin 后开始并保留自然尺寸。
    assert_eq!(positions[0].1, Rect::new(2.0, 3.0, 20.0, 10.0));
    // 缓存必须记录包含右下 margin 的完整外尺寸。
    assert_eq!(container.cached_content_size.get(), Size::new(30.0, 20.0));
}

// 验证自然测量与 flex-grow 零 basis 是两个独立契约。
#[test]
fn natural_measure_preserves_grow_container_content() {
    // 构造默认 Column 采用的增长型容器。
    let container = Container::new()
        // 使用垂直主轴复现 Card 内的 Column。
        .dir(FlexDirection::Column)
        // 普通父 Flex 应把它作为零 basis 增长项。
        .flex_grow(1.0);
    // 注入已经由子布局计算出的自然内容尺寸。
    container.cached_content_size.set(Size::new(188.0, 102.0));
    // 普通测量继续保留历史零 basis 行为。
    assert_eq!(
        // 通过普通布局入口读取父 Flex basis。
        WidgetLayout::measure(&container, Constraints::unconstrained()),
        // 增长型容器的普通 basis 为零。
        Size::zero(),
    );
    // 固有尺寸父容器必须能读取同一子树的真实内容尺寸。
    assert_eq!(
        // 通过新拆分的自然测量窄契约读取内容。
        WidgetLayout::measure_natural(&container, Constraints::unconstrained()),
        // 自然结果不受 flex-grow 归零影响。
        Size::new(188.0, 102.0),
    );
}

// 验证有限父约束会截断上一个大窗口留下的内容尺寸缓存。
#[test]
fn cached_content_size_respects_finite_parent_constraints() {
    // 普通非增长容器会使用子布局缓存作为自然尺寸下限。
    let container = Container::new();
    // 模拟最大化阶段记录的标题栏宽度。
    container.cached_content_size.set(Size::new(1920.0, 48.0));
    // 还原窗口提供更小但有限的客户区约束。
    let restored = WidgetLayout::measure(
        &container,
        Constraints::loose(Size::new(1200.0, 800.0)),
    );
    // 缓存不得突破有限父宽度，否则子树会继续按最大化尺寸排列。
    assert_eq!(restored, Size::new(1200.0, 48.0));
    // 无界测量仍保留真实内容范围，供滚动容器计算溢出尺寸。
    assert_eq!(
        WidgetLayout::measure(&container, Constraints::unconstrained()),
        Size::new(1920.0, 48.0),
    );
}

// 验证确定的父级交叉轴会覆盖大窗口阶段留下的子项自然宽度。
#[test]
fn stretch_cross_axis_shrinks_after_parent_frame_shrinks() {
    // 默认 Column 在交叉轴采用 Stretch。
    let container = Container::new();
    // 模拟标题栏子树仍报告最大化阶段的自然宽度。
    let child = LayoutChild::new(WidgetId::new(1), Size::new(1920.0, 48.0));
    // 还原后的容器 frame 已由客户区锁定为 1200 宽。
    let positions = container.layout_children(
        Rect::new(0.0, 0.0, 1200.0, 800.0),
        &[child.clone()],
        &WidgetTree::new(),
    );
    // Stretch 必须立即采用当前 frame，不能继续保留旧自然宽度。
    assert_eq!(positions[0].1.w, 1200.0);
    // 零宽 bootstrap 仍允许自然内容建立第一轮交叉轴尺寸。
    let bootstrap = container.layout_children(Rect::zero(), &[child], &WidgetTree::new());
    assert_eq!(bootstrap[0].1.w, 1920.0);
}

// 验证默认可见溢出与显式隐藏裁剪具有不同运行时结果。
#[test]
fn explicit_clip_content_controls_container_children_clip() {
    // 使用非零边界验证返回矩形保持完整。
    let frame = Rect::new(4.0, 6.0, 80.0, 40.0);
    // 默认容器不得意外裁剪既有 Rust 子树。
    assert_eq!(Container::new().children_clip(frame), None);
    // 构造显式 overflow hidden 对应的样式。
    let hidden_style = Style {
        // 开启直接子树裁剪。
        clip_content: Some(true),
        // 保留其他容器默认字段。
        ..Style::container()
    };
    // 显式隐藏必须裁剪到当前完整 frame。
    assert_eq!(
        // 使用公开样式替换入口构造隐藏溢出容器。
        Container::new().style(hidden_style).children_clip(frame),
        // 裁剪边界必须等于节点边界。
        Some(frame)
    );
    // 显式 visible 必须能够清除裁剪而不改变身份。
    let visible_style = Style {
        // 保存显式不裁剪声明。
        clip_content: Some(false),
        // 保留其他容器默认字段。
        ..Style::container()
    };
    // 显式可见与默认行为一致。
    assert_eq!(
        // 应用显式可见样式。
        Container::new().style(visible_style).children_clip(frame),
        // 子树不应取得裁剪矩形。
        None
    );
}

// 验证 StyleExt 完整阴影入口不会丢失横纵偏移。
#[test]
fn style_ext_box_shadow_preserves_full_definition() {
    // 构造具有非零双轴偏移的阴影定义。
    let shadow = BoxShadowDef::new(
        // 使用确定 RGBA 颜色便于精确比较。
        crate::draw::Color::from_rgba(10, 20, 30, 40),
        // 保存模糊半径。
        8.0,
        // 保存水平负偏移。
        -2.0,
        // 保存垂直正偏移。
        4.0,
    );
    // 使用公开 column 与 StyleExt 构造声明视图并完成适配展开。
    let node = crate::ui::column(()).box_shadow(Some(shadow)).into_node();
    // 读取展开后的底层 Container 完整样式快照。
    match node.widget.snapshot_fields() {
        // 检查阴影定义一一保留。
        SnapshotFields::Container { style } => {
            // 快照必须等于输入的完整定义。
            assert_eq!(style.box_shadow, Some(shadow));
        }
        // 其他组件类型表示公开 column 契约被破坏。
        _ => panic!("column 必须物化 Container 节点"),
    }
}
