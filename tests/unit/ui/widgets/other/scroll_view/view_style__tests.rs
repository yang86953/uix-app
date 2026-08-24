// 复用当前适配实现与父模块私有观测入口。
use super::*;
// 引入滚动方向构造测试组件。
use crate::platform::windowing::ScrollDirection;

// ViewNode 尺寸和显式零扩张必须覆盖 ScrollBuilder 默认值。
#[test]
fn view_layout_style_sets_fixed_viewport_and_flex_overrides() {
    // 模拟 ScrollBuilder 默认创建的可扩张视口。
    let mut scroll = ScrollView::new(ScrollDirection::Vertical).flex_grow(1.0);
    // 创建只声明布局字段的统一 View 样式。
    let mut style = Style::default();
    // 声明固定视口宽度。
    style.width = Some(360.0);
    // 声明固定视口高度。
    style.height = Some(80.0);
    // 通过 Adapter 使用的同一窄入口应用声明覆盖。
    scroll.apply_view_layout_style(&style, Some(0.0), Some(0.5));

    // 两条显式尺寸轴都必须被共享布局识别为锁定。
    assert_eq!(scroll.explicit_size_locks(), (true, true));
    // 固有尺寸必须精确保留声明视口大小。
    assert_eq!(scroll.intrinsic_size(), crate::core::Size::new(360.0, 80.0));
    // 快照必须携带零扩张和声明的收缩系数供协调比较。
    assert!(matches!(
        // 读取组件自己的稳定快照契约。
        scroll.snapshot_fields(),
        // 只匹配本测试关心的布局字段。
        crate::ui::SnapshotFields::ScrollView {
            // 显式宽度必须进入组件私有配置。
            fixed_width: Some(360.0),
            // 显式高度必须进入组件私有配置。
            fixed_height: Some(80.0),
            // 显式零扩张不能被默认值吞掉。
            flex_grow: 0.0,
            // 收缩覆盖同样必须保留。
            flex_shrink: 0.5,
            // 其余滚动字段不属于当前断言。
            ..
        }
    ));
}
