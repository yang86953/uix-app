// 导入当前模块的虚拟滚动类型与私有运行态。
use super::*;
// 导入按 key 核对子项身份所需的映射。
use std::collections::HashMap;

// 导入构建测试树所需的 View 适配器。
use crate::ui::adapter::ViewAdapter;
// 导入访问组件树子项所需的核心 trait。
use crate::ui::component::widget::WidgetCore;
// 导入构造行 View 所需的节点类型。
use crate::ui::view::ViewNode;
// 导入最小文本行组件。
use crate::ui::widgets::Label;

// 收集根节点直接子项的稳定 key 与组件标识。
fn keyed_children(tree: &WidgetTree, root: ComponentId) -> HashMap<String, ComponentId> {
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
fn overlapping_materialized_rows_retain_component_ids() {
    // 构造带业务稳定 key 的固定行高虚拟列表。
    let view = VirtualScroll::new()
        .item_count(100)
        .item_height(10.0)
        .overscan(1)
        .size(100.0, 30.0)
        .render(|index| {
            // 每一行用绝对索引声明稳定业务 key。
            ViewNode::leaf(Label::new(index.to_string())).key(format!("row-{index}"))
        });
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
        .expect("virtual root component")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<VirtualScroll>()
        .expect("virtual scroll component");
    // 写入事件路径会产生的有限滚动偏移。
    scroll.scroll_offset = 20.0;
    // 刷新物化窗口并要求结构确实发生变化。
    assert!(tree.refresh_virtual_scroll_component(root, Some(30.0)));
    // 收集刷新后的稳定 key 与组件标识。
    let after = keyed_children(&tree, root);

    // 三个重叠行必须继续使用原组件，保留内部状态与焦点身份。
    for key in ["row-1", "row-2", "row-3"] {
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
        .and_then(|node| node.component().as_any().downcast_ref::<Label>())
        .expect("first virtual label");
    // 原组件必须呈现新版 renderer 文本。
    assert_eq!(first.text(), "new-0");
}
