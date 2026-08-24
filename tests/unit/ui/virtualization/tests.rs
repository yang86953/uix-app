// 导入当前模块的虚拟滚动类型与私有运行态。
use super::*;
// 导入按 key 核对子项身份所需的映射。
use std::collections::HashMap;

// 导入构建测试树所需的 View 适配器。
use crate::ui::adapter::ViewAdapter;
// 导入访问组件树子项所需的核心 trait。
use crate::ui::widget_runtime::widget::WidgetCore;
// 导入直接构造并调用布局入口所需的子项与 trait。
use crate::ui::{LayoutChild, WidgetLayout};
// 导入构造行 View 所需的节点类型。
use crate::ui::view::ViewNode;
// 导入最小文本行组件。
use crate::ui::widgets::Label;

// 收集根节点直接子项的稳定 key 与组件标识。
fn keyed_children(tree: &WidgetTree, root: WidgetId) -> HashMap<String, WidgetId> {
    // 读取虚拟滚动当前物化的直接子项。
    let children = tree.get(root).expect("virtual root exists").children();
    // 只收集带稳定 key 的可复用行。
    children
        .iter()
        .filter_map(|child_id| {
            // 读取行节点及其声明 key。
            let child = tree.get(*child_id)?;
            // 把借用的 key 转为测试拥有的字符串。
            Some((child.key()?.to_string(), *child_id))
        })
        .collect()
}

// 验证可见窗口滑动后重叠 key 继续复用原组件身份。
#[test]
fn overlapping_materialized_rows_retain_widget_ids() {
    // 构造带业务稳定 key 的固定行高虚拟列表。
    let view = VirtualScroll::new()
        .item_count(100)
        .item_height(10.0)
        .overscan(1)
        .size(100.0, 30.0)
        // 用显式键工厂让业务身份在行构建前进入捕获命名空间。
        .render_keyed(
            // 当前用绝对索引模拟稳定业务 id。
            |index| format!("row-{index}"),
            // 行工厂不再自行设置根 key。
            |index| ViewNode::leaf(Label::new(index.to_string())),
        );
    // 构建后会按配置视口立即物化索引零到三。
    let mut tree = ViewAdapter::build(view);
    // 读取虚拟滚动根标识。
    let root = tree.root_id().expect("virtual root id");
    // 保存首次物化窗口的 key 到组件标识映射。
    let before = keyed_children(&tree, root);
    // 首个窗口必须包含四个带 overscan 的行。
    assert_eq!(before.len(), 4);

    // 将运行态偏移推进两行，使新窗口变为索引一到五。
    let scroll = tree
        .get_mut(root)
        .expect("virtual root widget")
        .widget_mut()
        .as_any_mut()
        .downcast_mut::<VirtualScroll>()
        .expect("virtual scroll widget");
    // 写入事件路径会产生的有限滚动偏移。
    scroll.scroll_offset.set(20.0);
    // 刷新物化窗口并要求结构确实发生变化。
    assert!(tree.refresh_virtual_scroll_widget(root, Some(30.0)));
    // 收集刷新后的稳定 key 与组件标识。
    let after = keyed_children(&tree, root);

    // 三个重叠行必须继续使用原组件，保留内部状态与焦点身份。
    for key in [
        // 业务键会被框架规范化以隔离索引身份模式。
        "virtual-scroll-business:row-1",
        // 中间重叠项继续使用同一规范化业务身份。
        "virtual-scroll-business:row-2",
        // 窗口尾部重叠项同样原位复用。
        "virtual-scroll-business:row-3",
    ] {
        // 核对同一业务 key 的组件标识没有变化。
        assert_eq!(after.get(key), before.get(key), "{key} must be reused");
    }
}

// 验证未声明业务 key 的固定索引行获得确定性后备身份。
#[test]
fn unkeyed_rows_receive_stable_index_keys() {
    // 构造与使用文档一致的无显式 key 虚拟列表。
    let view = VirtualScroll::new()
        .item_count(10)
        .item_height(10.0)
        .overscan(1)
        .size(100.0, 30.0)
        .render(|index| ViewNode::leaf(Label::new(index.to_string())));
    // 构建初始物化窗口。
    let tree = ViewAdapter::build(view);
    // 读取虚拟滚动根标识。
    let root = tree.root_id().expect("virtual root id");
    // 按物化顺序收集框架生成的后备 key。
    let keys: Vec<_> = tree
        .get(root)
        .expect("virtual root exists")
        .children()
        .iter()
        .map(|child_id| {
            // 每个无业务 key 行都应获得绝对索引 key。
            tree.get(*child_id)
                .and_then(|child| child.key())
                .expect("virtual row fallback key")
                .to_string()
        })
        .collect();
    // 初始三行与一行 overscan 的身份必须确定且连续。
    assert_eq!(
        keys,
        [
            "virtual-scroll-item:0",
            "virtual-scroll-item:1",
            "virtual-scroll-item:2",
            "virtual-scroll-item:3",
        ]
    );
}

// 验证 renderer 更新会原位协调内容而不会重建稳定窗口。
#[test]
fn renderer_updates_patch_rows_without_replacing_ids() {
    // 构造首版无显式 key 行，依赖框架绝对索引后备身份。
    let initial = VirtualScroll::new()
        .item_count(10)
        .item_height(10.0)
        .overscan(1)
        .size(100.0, 30.0)
        .render(|index| ViewNode::leaf(Label::new(format!("old-{index}"))));
    // 构建首版虚拟滚动树。
    let mut tree = ViewAdapter::build(initial);
    // 保存稳定根标识。
    let root = tree.root_id().expect("virtual root id");
    // 保存首版行身份。
    let before = keyed_children(&tree, root);

    // 用相同窗口配置和新版 renderer 协调整棵声明树。
    ViewAdapter::reconcile(
        &mut tree,
        VirtualScroll::new()
            .item_count(10)
            .item_height(10.0)
            .overscan(1)
            .size(100.0, 30.0)
            .render(|index| ViewNode::leaf(Label::new(format!("new-{index}")))),
    );
    // 收集协调后的行身份。
    let after = keyed_children(&tree, root);
    // 相同物化窗口的全部行必须保留组件标识。
    assert_eq!(after, before);

    // 读取首行标签组件以核对 renderer 新数据已经写入。
    let first = after
        .get("virtual-scroll-item:0")
        .and_then(|id| tree.get(*id))
        .and_then(|node| node.widget().as_any().downcast_ref::<Label>())
        .expect("first virtual label");
    // 原组件必须呈现新版 renderer 文本。
    assert_eq!(first.text(), "new-0");
}

// 验证正常有限输入仍按可见行与双侧 overscan 精确计算窗口。
#[test]
fn finite_range_preserves_visible_rows_and_overscan() {
    // 五十像素偏移对应第五行，三十像素视口覆盖到第八行。
    let range = virtual_list_index_range(100, 10.0, 50.0, 30.0, 2);
    // 双侧各扩展两行后应物化索引三到九。
    assert_eq!(range, (3, 10));
    // 超出内容末端的恢复偏移必须先夹到最后一个完整视口。
    let clamped = virtual_list_index_range(10, 10.0, 10_000.0, 30.0, 1);
    // 末端三行与一行起始侧 overscan 应保持可见。
    assert_eq!(clamped, (6, 10));
}

// 验证非法或无可见面积的度量统一退化为空窗口。
#[test]
fn invalid_measurements_return_an_empty_range() {
    // 覆盖非有限、零值和负值行高。
    for item_height in [f32::NAN, f32::INFINITY, 0.0, -1.0] {
        // 非法行高不能产生任何物化行。
        assert_eq!(
            virtual_list_index_range(100, item_height, 50.0, 30.0, 2),
            (0, 0)
        );
    }
    // 覆盖非有限、零值和负值视口高度。
    for viewport_height in [f32::NAN, f32::INFINITY, 0.0, -1.0] {
        // 无有效可见面积时不得仅因 overscan 物化行。
        assert_eq!(
            virtual_list_index_range(100, 10.0, 50.0, viewport_height, 2),
            (0, 0)
        );
    }
}

// 验证病理视口与 overscan 不能越过单次物化安全预算。
#[test]
fn extreme_range_is_bounded_by_the_materialization_budget() {
    // 使用远超安全窗的可见范围和饱和 overscan 模拟不可信恢复状态。
    let range = virtual_list_index_range(usize::MAX, 1.0, 50_000.0, 10_000.0, usize::MAX);
    // 可见内容优先从首个可见行起保留四千零九十六项。
    assert_eq!(range, (50_000, 54_096));
    // 最终窗口无论输入规模如何都必须保持有界。
    assert!(range.1.saturating_sub(range.0) <= 4_096);
}

// 验证共享滚动状态不会保存或返回非有限偏移。
#[test]
fn virtual_list_scroll_keeps_pathological_offsets_finite() {
    // 构造默认有限滚动状态。
    let mut scroll = VirtualListScroll::new();
    // 超大列表的最大偏移也必须可用于后续有限几何计算。
    let max = scroll.max_scroll_offset(usize::MAX, f32::MAX / 8.0, 1.0);
    // 禁止总高度乘法溢出为无穷大。
    assert!(max.is_finite());
    // 有效超大内容仍应保留正的可滚动范围。
    assert!(max > 0.0);
    // 非数滚动增量应被忽略而非污染状态。
    assert_eq!(scroll.scroll_by(f32::NAN, 100, 10.0, 20.0), 0.0);
    // 忽略后状态仍停留在有限原点。
    assert_eq!(scroll.scroll_offset(), 0.0);
    // 正无穷增量只能夹到有限内容末端。
    let applied = scroll.scroll_by(f32::INFINITY, usize::MAX, f32::MAX / 8.0, 1.0);
    // 对外报告的实际增量必须有限。
    assert!(applied.is_finite());
    // 正无穷具有向下方向语义，应抵达有限内容末端。
    assert!(applied > 0.0);
    // 内部偏移也必须保持有限。
    assert!(scroll.scroll_offset().is_finite());
}

// 验证虚拟滚动总高度与最终子项 frame 遵守有限几何不变量。
#[test]
fn virtual_scroll_sanitizes_total_height_and_layout_frames() {
    // 构造乘法会溢出 f32 的超大但有限列表度量。
    let huge = VirtualScroll::new()
        .item_count(usize::MAX)
        .item_height(f32::MAX / 8.0);
    // 对外总高度必须夹到有限表示范围。
    assert!(huge.total_height().is_finite());
    // 有效超大列表不能被误判为空内容。
    assert!(huge.total_height() > 0.0);

    // 构造携带非法行高与偏移的最小物化窗口。
    let scroll = VirtualScroll::new().item_count(1).item_height(f32::NAN);
    // 模拟协调器已物化第一行。
    scroll.materialized_range.set(Some((0, 1)));
    // 模拟不可信恢复状态写入无穷偏移。
    scroll.scroll_offset.set(f32::INFINITY);
    // 构造一个无需读取树内容的布局子项。
    let children = [LayoutChild::new(WidgetId::new(1), Size::new(10.0, 10.0))];
    // 空树足以覆盖虚拟行的纯几何放置路径。
    let tree = WidgetTree::new();
    // 输入同时包含非法坐标、宽度与高度。
    let positions = scroll.layout_children(
        Rect::new(f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::NAN),
        &children,
        &tree,
    );
    // 读取唯一子项的最终 frame。
    let frame = positions[0].1;
    // 所有坐标与尺寸都必须为有限值。
    for value in [frame.x, frame.y, frame.w, frame.h] {
        // 禁止任何非有限值进入布局树。
        assert!(value.is_finite());
    }
    // 尺寸还必须满足非负约束。
    assert!(frame.w >= 0.0 && frame.h >= 0.0);
    // 非数行高应明确退化为零高度。
    assert_eq!(frame.h, 0.0);
}

// 验证稀疏测量能够修正项目起点、总高度和可见范围。
#[test]
fn variable_measurements_keep_prefix_offsets_and_total_height() {
    // 使用十像素作为未知项目的估算高度。
    let mut cache = VirtualListMeasurementCache::new();
    // 第二项目的实际高度是估算值的两倍。
    assert!(cache.record(1, 20.0));
    // 第四项目的实际高度小于估算值。
    assert!(cache.record(3, 5.0));
    // 重复写入相同高度不应制造刷新。
    assert!(!cache.record(1, 20.0));
    // 非法高度不能污染已有缓存。
    assert!(!cache.record(2, f32::NAN));
    // 第一个项目起点仍然位于内容原点。
    assert_eq!(cache.offset_for_index(0, 10.0), 0.0);
    // 第二个项目起点只包含第一个估算项目。
    assert_eq!(cache.offset_for_index(1, 10.0), 10.0);
    // 第三个项目起点已经包含第二项目的实际增量。
    assert_eq!(cache.offset_for_index(2, 10.0), 30.0);
    // 第四个项目起点继续使用未知第三项目的估算高度。
    assert_eq!(cache.offset_for_index(3, 10.0), 40.0);
    // 五项目总高度等于十、二十、十、五、十的和。
    assert_eq!(cache.total_height(5, 10.0), 55.0);
    // 三十像素偏移和十五像素视口覆盖第二、第三项目。
    assert_eq!(
        virtual_list_index_range_with_measurements(5, 10.0, &cache, 30.0, 15.0, 0),
        (2, 4)
    );
}

// 验证稳定帧预检与正式测量写入使用完全相同的变化判定。
#[test]
fn measurement_change_preflight_matches_record_semantics() {
    // 新索引的有效高度必须进入变化路径。
    let mut cache = VirtualListMeasurementCache::new();
    assert!(cache.would_record_change(7, 18.0));
    assert!(cache.record(7, 18.0));
    // 相同高度必须在锚点计算前直接退出。
    assert!(!cache.would_record_change(7, 18.0));
    assert!(!cache.record(7, 18.0));
    // 非法输入同样不得被预检误判为可提交变化。
    for height in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
        assert!(!cache.would_record_change(7, height));
        assert!(!cache.record(7, height));
    }
    // 合法的新高度仍由原提交路径替换并推进结构代际。
    let generation = cache.generation();
    assert!(cache.would_record_change(7, 24.0));
    assert!(cache.record(7, 24.0));
    assert_eq!(cache.get(7), Some(24.0));
    assert_eq!(cache.generation(), generation.wrapping_add(1));
}

// 验证项目偏移只复用逐位相同的最终结果，并随测量代际自动失效。
#[test]
fn variable_item_offset_cache_preserves_exact_results_and_invalidates() {
    // 构造带稀疏实际高度的可变列表。
    let scroll = VirtualScroll::new()
        .item_count(8)
        .item_height(10.0)
        .variable_height();
    assert!(scroll.measure_item(0, 20.0));
    // 首次查询仍由原前缀算法计算并保存最终 f32。
    let first = scroll.item_offset(4);
    assert_eq!(first, 50.0);
    assert_eq!(scroll.item_offset_cache.borrow().offsets.len(), 1);
    // 同一代际重复读取必须逐位复用且不增加缓存项。
    let cached = scroll.item_offset(4);
    assert_eq!(cached.to_bits(), first.to_bits());
    assert_eq!(scroll.item_offset_cache.borrow().offsets.len(), 1);
    // 新测量推进代际后，旧值必须失效并由原算法重新计算。
    assert!(scroll.measure_item(1, 30.0));
    let refreshed = scroll.item_offset(4);
    assert_eq!(refreshed, 70.0);
    assert_ne!(refreshed.to_bits(), first.to_bits());
    assert_eq!(scroll.item_offset_cache.borrow().offsets.len(), 1);
}

// 验证物化范围缓存只命中完整相同输入，并随测量代际更新。
#[test]
fn variable_scroll_range_cache_reuses_exact_result_and_invalidates() {
    // 构造可产生明确边界变化的可变高度列表。
    let scroll = VirtualScroll::new()
        .item_count(10)
        .item_height(10.0)
        .variable_height()
        .overscan(0);
    scroll.scroll_offset.set(20.0);
    // 首次结果由原算法计算并写入单值缓存。
    let first = scroll.scroll_range(15.0);
    assert_eq!(first, (2, 4));
    let first_entry = scroll.range_cache.get().expect("range cache should be set");
    // 完全相同输入直接复用同一结果和同一键。
    assert_eq!(scroll.scroll_range(15.0), first);
    assert_eq!(
        scroll.range_cache.get().map(|entry| entry.key),
        Some(first_entry.key)
    );
    // 首项变高会推进测量代际，使旧范围自动失效。
    assert!(scroll.measure_item(0, 20.0));
    let refreshed = scroll.scroll_range(15.0);
    assert_eq!(refreshed, (1, 3));
    let refreshed_entry = scroll
        .range_cache
        .get()
        .expect("range cache should refresh");
    assert_ne!(refreshed_entry.key, first_entry.key);
}

// 验证可变行高模式会把已物化子项测量写回 frame 和滚动几何。
#[test]
fn variable_scroll_layout_uses_measured_item_frames() {
    // 构造启用可变高度和零 overscan 的四项目列表。
    let scroll = VirtualScroll::new()
        .item_count(4)
        .item_height(10.0)
        .variable_height()
        .overscan(0);
    // 标记前三个项目已经物化，布局起点从绝对索引零开始。
    scroll.materialized_range.set(Some((0, 3)));
    // 为前三个项目提供不同的测量高度。
    let children = [
        LayoutChild::new(WidgetId::new(1), Size::new(10.0, 10.0)),
        LayoutChild::new(WidgetId::new(2), Size::new(10.0, 20.0)),
        LayoutChild::new(WidgetId::new(3), Size::new(10.0, 5.0)),
    ];
    // 使用有限视口直接调用虚拟列表布局入口。
    let positions = scroll.layout_children(
        Rect::new(0.0, 0.0, 100.0, 25.0),
        &children,
        &WidgetTree::new(),
    );
    // 第一项目从内容原点开始并保留估算高度。
    assert_eq!(positions[0].1, Rect::new(0.0, 0.0, 100.0, 10.0));
    // 第二项目起点只推进第一项目高度。
    assert_eq!(positions[1].1, Rect::new(0.0, 10.0, 100.0, 20.0));
    // 第三项目起点包含前两项目的实际高度。
    assert_eq!(positions[2].1, Rect::new(0.0, 30.0, 100.0, 5.0));
    // 未物化的第四项目仍按十像素估算，因此总高度为四十五像素。
    assert_eq!(scroll.total_height(), 45.0);
    // 已记录的第二项目高度可被外部读取。
    assert_eq!(scroll.measured_item_height(1), Some(20.0));
    // 零偏移和十五像素视口只覆盖前两项目。
    assert_eq!(scroll.scroll_range(15.0), (0, 2));
}

// 验证显式失效会清除旧高度并回退到估算布局。
#[test]
fn measurement_invalidation_reanchors_variable_scroll() {
    // 构造一个带已测量高度的可变列表。
    let scroll = VirtualScroll::new()
        .item_count(2)
        .item_height(10.0)
        .variable_height();
    // 写入当前项目的实际高度。
    assert!(scroll.measure_item(0, 25.0));
    // 当前总高度应包含实际测量。
    assert_eq!(scroll.total_height(), 35.0);
    // 字体或宽度变化后显式清除旧测量。
    scroll.invalidate_measurements();
    // 清理后回退到两个十像素估算项目。
    assert_eq!(scroll.total_height(), 20.0);
    // 清理后的旧测量不能再被读取。
    assert_eq!(scroll.measured_item_height(0), None);
}

// 验证视口上方项目变高时滚动偏移会保持当前可见锚点。
#[test]
fn upper_measurement_change_preserves_visible_anchor() {
    // 构造十像素估算高度和二十像素视口的长列表。
    let scroll = VirtualScroll::new()
        .item_count(10)
        .item_height(10.0)
        .variable_height()
        .size(100.0, 20.0);
    // 将视口定位到第二项目起点，当前可见锚点为索引二。
    scroll.scroll_offset.set(20.0);
    // 第一项目变高二十像素时，上方累计高度增加二十像素。
    assert!(scroll.measure_item(0, 30.0));
    // 滚动偏移同步增加二十像素，保持原索引二的视觉位置。
    assert_eq!(scroll.scroll_offset(), 40.0);
}
