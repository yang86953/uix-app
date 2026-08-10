// 复用父模块中的 TreeSelect 实现与私有方法。
use super::TreeSelect;
// 引入断言所需的矩形基础类型。
use crate::core::Rect;
// 引入受控选择测试所需的响应式状态。
use crate::ui::reactive::state::State;
// 引入树节点公开构造类型。
use crate::ui::widgets::display::tree::TreeNode;

// 受控绑定必须以稳定节点 key 驱动标题并接收用户选择。
#[test]
// 测试名称说明状态到标题及选择到状态的双向契约。
fn controlled_value_uses_stable_key_and_commits_selection() {
    // 初始状态指向第二个节点的稳定 key。
    let selected = State::new("node-1".to_owned());
    // 先声明树结构，再绑定外部状态。
    let mut tree_select = TreeSelect::new()
        // 提供两个可选择叶节点。
        .nodes(leaf_nodes(2))
        // 绑定外部稳定 key 状态。
        .bind_value(&selected);

    // 组件展示值应由稳定 key 对应的标题派生。
    assert_eq!(tree_select.value(), "节点1");
    // 组件公开 key 应保持外部状态中的稳定值。
    assert_eq!(tree_select.value_key(), "node-1");

    // 模拟用户选择第一个扁平节点。
    assert!(tree_select.select_flat_index(0));
    // 用户选择必须把稳定 key 写回外部状态。
    assert_eq!(selected.get(), "node-0");

    // 模拟业务逻辑把受控状态切回第二个节点。
    selected.set("node-1".to_owned());
    // 声明式重建必须把最新状态同步进既有实例。
    tree_select.sync_from(TreeSelect::new().nodes(leaf_nodes(2)).bind_value(&selected));
    // 重建后展示标题必须跟随外部稳定 key。
    assert_eq!(tree_select.value(), "节点1");
    // 重建后公开 key 必须与外部状态一致。
    assert_eq!(tree_select.value_key(), "node-1");
}

// 构造指定数量的稳定叶子节点。
fn leaf_nodes(count: usize) -> Vec<TreeNode> {
    // 为每个序号创建不同标题与键。
    (0..count)
        // 将序号映射为树节点。
        .map(|index| TreeNode::new(&format!("节点{index}"), &format!("node-{index}")))
        // 收集为 TreeSelect 所需节点列表。
        .collect()
}

// 靠近表面底边时弹层必须翻转并按上方空间缩高。
#[test]
// 测试名称说明纵向表面约束职责。
fn overlay_entry_constrains_popup_near_surface_bottom() {
    // 创建包含多行节点的树选择器。
    let mut tree_select = TreeSelect::new().nodes(leaf_nodes(20));
    // 打开树选择弹层参与登记。
    tree_select.open();
    // 将触发器放在一百二十像素高表面的底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 构造不足以容纳自然弹层高度的当前逻辑表面。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 通过组件树使用的显式表面入口创建登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测树选择组件。
        &tree_select,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(12),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的树选择器应生成浮层登记")
    // 读取登记的绝对弹层矩形。
    .bounds_rect()
    // 树选择弹层登记必须声明边界。
    .expect("树选择弹层应声明边界");

    // 下方空间不足时弹层应完整位于触发器上方。
    assert!(overlay.y + overlay.h <= frame.y);
    // 弹层顶边不得越出当前表面。
    assert!(overlay.y >= surface.y);
    // 弹层底边不得越出当前表面。
    assert!(overlay.y + overlay.h <= surface.y + surface.h);
}

// 触发器靠近窄表面右边缘时弹层必须横向收敛。
#[test]
// 测试名称说明横向表面约束职责。
fn overlay_entry_constrains_popup_to_narrow_surface() {
    // 创建单行树选择器。
    let mut tree_select = TreeSelect::new().nodes(leaf_nodes(1));
    // 打开树选择弹层参与登记。
    tree_select.open();
    // 构造宽于表面且靠近右边界的触发器。
    let frame = Rect::new(70.0, 20.0, 120.0, 32.0);
    // 使用一百像素宽的窄逻辑表面。
    let surface = Rect::new(0.0, 0.0, 100.0, 260.0);
    // 通过显式表面入口创建弹层登记。
    let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测树选择组件。
        &tree_select,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(13),
        // 传入靠近右边界的触发器 frame。
        frame,
        // 传入当前窄表面。
        surface,
    )
    // 打开状态必须生成浮层登记。
    .expect("打开的树选择器应生成浮层登记")
    // 读取登记的绝对弹层矩形。
    .bounds_rect()
    // 树选择弹层登记必须声明边界。
    .expect("树选择弹层应声明边界");

    // 弹层左边不得越出当前表面。
    assert!(overlay.x >= surface.x);
    // 弹层右边不得越出当前表面。
    assert!(overlay.x + overlay.w <= surface.x + surface.w);
    // 弹层宽度不得超过当前表面宽度。
    assert!(overlay.w <= surface.w);
}

// 弹层缩高后键盘显露必须使用实际视口而非自然高度。
#[test]
// 测试名称说明缩高视口与虚拟滚动账本的一致性。
fn constrained_popup_height_drives_reveal_scroll_range() {
    // 创建二十行节点使内容高度达到五百六十像素。
    let mut tree_select = TreeSelect::new().nodes(leaf_nodes(20));
    // 打开树选择弹层参与表面解析。
    tree_select.open();
    // 将触发器放在表面底部附近，使上方只有八十像素。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 使用一百二十像素高表面触发向上缩高。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 先通过显式登记入口记录实际受约束视口。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测树选择组件。
        &tree_select,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(14),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    );
    // 请求显露最后一行。
    tree_select.reveal_index(19, 20);

    // 五百六十像素内容减八十像素实际视口应留下四百八十像素范围。
    assert_eq!(tree_select.dropdown_scroll.scroll_offset(), 480.0);
}

// 弹层翻到上方后行命中必须复用负向本地偏移。
#[test]
// 测试名称说明登记几何与事件映射的一致性。
fn flipped_popup_geometry_drives_row_hit_mapping() {
    // 创建包含多行节点的树选择器。
    let mut tree_select = TreeSelect::new().nodes(leaf_nodes(20));
    // 打开树选择弹层参与表面解析。
    tree_select.open();
    // 将触发器放在表面底部附近。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 使用一百二十像素高表面迫使弹层向上。
    let surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 通过显式表面入口缓存翻转后的本地弹层矩形。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测树选择组件。
        &tree_select,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(15),
        // 传入靠近底边的触发器 frame。
        frame,
        // 传入当前逻辑表面。
        surface,
    );

    // 相对触发器原点负七十像素落在向上弹层首行。
    assert_eq!(tree_select.dropdown_row_at_y(-70.0), Some(0));
}

// 表面缩小时脏区必须覆盖翻转前弹层仍留在新表面的尾部。
#[test]
// 测试名称说明历史绝对弹层与当前表面的交集职责。
fn surface_change_dirty_covers_previous_popup_tail() {
    // 创建足以生成最大自然高度的树选择器。
    let mut tree_select = TreeSelect::new().nodes(leaf_nodes(20));
    // 打开树选择弹层参与表面解析。
    tree_select.open();
    // 固定触发器位置，使大表面向下、小表面向上。
    let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
    // 首帧表面允许弹层完整向下展开。
    let large_surface = Rect::new(0.0, 0.0, 240.0, 400.0);
    // 先登记向下展开的旧弹层。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入被测树选择组件。
        &tree_select,
        // 使用稳定的测试组件标识。
        crate::core::ComponentId::new(16),
        // 传入固定触发器 frame。
        frame,
        // 传入可完整向下展开的大表面。
        large_surface,
    );
    // 次帧表面迫使弹层翻到上方并缩高。
    let small_surface = Rect::new(0.0, 0.0, 240.0, 120.0);
    // 刷新登记并累计翻转前后的弹层区域。
    let _ = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
        // 传入同一树选择组件。
        &tree_select,
        // 沿用同一测试组件标识。
        crate::core::ComponentId::new(16),
        // 触发器位置保持不变。
        frame,
        // 传入缩小后的当前表面。
        small_surface,
    );
    // 读取组件对当前表面声明的完整脏区。
    let dirty = crate::ui::component::traits::WidgetRender::dirty_rect(&tree_select, frame);

    // 当前向上弹层与触发器从表面顶边开始。
    assert_eq!(dirty.y, small_surface.y);
    // 旧向下弹层在新表面内残留的八像素尾部也必须被清理。
    assert_eq!(dirty.y + dirty.h, small_surface.y + small_surface.h);
}
