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
