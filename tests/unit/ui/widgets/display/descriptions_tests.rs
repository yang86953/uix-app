// 引入被测描述列表与类型化数据项。
use super::{Descriptions, DescriptionsItem};
// 引入公开 View 构建入口。
use crate::ui::view::View;

// 验证 UIX 声明壳保持原描述列表单叶节点与类型化数据。
#[test]
fn uix_root_preserves_descriptions_kernel_and_items() {
    // 构建两列且包含跨列项目的描述列表。
    let node = View::build(
        Descriptions::new()
            .column(2)
            .add(DescriptionsItem::new("姓名", "Ada").span(2)),
    );
    // UIX 声明不得增加包装或展示子节点。
    assert!(node.children.is_empty());
    // 根动态类型必须继续是拥有网格布局与绘制机制的 Descriptions。
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Descriptions>()
        .expect("UIX 根必须保留 Descriptions 内核");
    // 列数与类型化数据必须无损进入同一内核。
    assert_eq!(kernel.column, 2);
    assert_eq!(kernel.items.len(), 1);
    assert_eq!(kernel.items[0].label, "姓名");
    assert_eq!(kernel.items[0].span, 2);
    // UIX 视觉配置必须进入真实网格内核。
    assert_eq!(
        kernel.visual_contract_for_test(),
        (600.0, 140.0, 32.0, 15.0, 13.0, 8.0, 6.0)
    );
}

// UIX 默认网格只填充未显式设置字段，且实例共享静态视觉表。
#[test]
fn uix_defaults_preserve_authored_grid_and_share_visuals() {
    let authored = View::build(
        Descriptions::new()
            .bordered(true)
            .column(2)
            .label_width(120.0),
    );
    let defaults = View::build(Descriptions::new());
    let authored = authored
        .widget
        .as_any()
        .downcast_ref::<Descriptions>()
        .expect("作者 Descriptions 必须保留内核");
    let defaults = defaults
        .widget
        .as_any()
        .downcast_ref::<Descriptions>()
        .expect("默认 Descriptions 必须保留内核");
    assert_eq!(
        (authored.bordered, authored.column, authored.label_width),
        (true, 2, 120.0)
    );
    assert_eq!(
        (defaults.bordered, defaults.column, defaults.label_width),
        (false, 3, 100.0)
    );
    assert!(authored.shares_visual_with_for_test(defaults));
}

// 同一宽度的测量与绘制必须复用行高向量，数据变化后重新计算但保留容量。
#[test]
fn row_height_cache_reuses_capacity_and_invalidates_on_sync() {
    let mut descriptions = Descriptions::new()
        .column(1)
        .add(DescriptionsItem::new("字段", "短值"));
    let first = descriptions.row_heights(240.0);
    let first_height = first[0];
    let first_pointer = first.as_ptr();
    drop(first);
    let repeated = descriptions.row_heights(240.0);
    assert_eq!(repeated.as_ptr(), first_pointer);
    drop(repeated);

    descriptions.sync_from(Descriptions::new().column(1).add(DescriptionsItem::new(
        "字段",
        "这是一个会在窄列中换成多行、从而改变缓存行高的较长字段值",
    )));
    let refreshed = descriptions.row_heights(240.0);
    assert_eq!(refreshed.as_ptr(), first_pointer);
    assert!(refreshed[0] > first_height);
}

// 验证流式项目迭代器保持跨列项目的原有装箱顺序。
#[test]
fn placement_iterator_preserves_grid_packing() {
    // 三列依次放入一列、两列、两列项目，第三项必须换到下一行。
    let descriptions = Descriptions::new()
        .column(3)
        .add(DescriptionsItem::new("甲", "1"))
        .add(DescriptionsItem::new("乙", "2").span(2))
        .add(DescriptionsItem::new("丙", "3").span(2));
    let placements = descriptions
        .item_placements(3)
        .map(|placement| (placement.row, placement.column, placement.span))
        .collect::<Vec<_>>();

    assert_eq!(placements, vec![(0, 0, 1), (0, 1, 2), (1, 0, 2)]);
}
