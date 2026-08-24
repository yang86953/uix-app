// 引入被测 List 与私有角色契约。
use super::*;
// 引入指针坐标。
use crate::core::Point;
// 引入真实按钮指针事件所需的原生输入类型。
use crate::platform::windowing::{KeyMod, MouseButton};
// 引入真实声明树物化与协调入口。
use crate::ui::adapter::ViewAdapter;
// 引入组件树布局 trait。
use crate::ui::widget_runtime::traits::WidgetLayout;
// 引入读取子身份与写入根 frame 所需的组件树核心契约。
use crate::ui::widget_runtime::widget::WidgetCore;
// 引入 Empty、事件、ViewNode 与公开按钮。
use crate::ui::{EventResult, SystemEvent, ViewNode};
// 引入公开按钮与空状态组件。
use crate::ui::widgets::{Button, Empty};
// 引入点击次数共享单元。
use std::cell::Cell as CounterCell;
// 引入点击处理器共享所有权。
use std::rc::Rc as Shared;

// 验证三个真实插槽按正常流排列且加载按钮保留业务点击。
#[test]
// 测试名称覆盖布局与事件所有权。
fn view_slots_follow_list_flow_and_keep_button_click() {
    // 建立真实加载按钮独占的点击计数。
    let clicks = Shared::new(CounterCell::new(0));
    // 为按钮处理器克隆共享句柄。
    let load_clicks = Shared::clone(&clicks);
    // 构造带三个真实角色与一个文本数据行的 List。
    let view = List::new()
        // 数据行继续由 List 自绘。
        .items(vec!["任务"])
        // 页首使用真实按钮暴露自然高度。
        .header_view(ViewNode::leaf(Button::new("页首")))
        // 页尾使用独立真实按钮。
        .footer_view(ViewNode::leaf(Button::new("页尾")))
        // 加载入口保留完整点击闭包。
        .load_more_view(
            // 事件处理器属于完整 ViewNode 而非 List 组件字段。
            ViewNode::leaf(Button::new("加载更多")).on_click_fn(move || {
                // 只有真实加载按钮负责累加业务点击。
                load_clicks.set(load_clicks.get() + 1);
            }),
        );
    // 通过真实适配器物化 List 与全部插槽子树。
    let mut tree = ViewAdapter::build(view);
    // 取得稳定 List 根身份。
    let root = tree.root_id().expect("List 根必须存在");
    // 读取固定角色顺序的三个真实子身份。
    let children = tree.get(root).expect("List 根必须可读").children().to_vec();
    // 三个角色必须各产生一个直接子节点。
    assert_eq!(children.len(), 3);
    // 为根分配足够容纳全部正常流内容的 frame。
    tree.get_mut(root)
        // 根节点必须可写。
        .expect("List 根必须存在")
        // 使用固定宽度隔离真实子树高度收敛。
        .set_frame(Rect::new(0.0, 0.0, 400.0, 220.0));
    // 执行框架真实多阶段布局收敛。
    tree.layout();
    // 读取页首最终 frame。
    let header = tree.get(children[0]).expect("页首必须存在").frame();
    // 读取页尾最终 frame。
    let footer = tree.get(children[1]).expect("页尾必须存在").frame();
    // 读取加载入口最终 frame。
    let load_more = tree.get(children[2]).expect("加载入口必须存在").frame();
    // 页首必须从 List 顶部进入正常流。
    assert_eq!(header.y, 0.0);
    // 页尾必须位于页首和一个标准文本行之后。
    assert!(footer.y >= header.y + header.h + list_item_height(ControlSize::Medium));
    // 加载入口必须位于页尾之后且不与其重叠。
    assert!(load_more.y >= footer.y + footer.h);
    // 计算加载按钮内部的稳定点击坐标。
    let click_position = Point::new(
        // 水平方向取按钮中心。
        load_more.x + load_more.w * 0.5,
        // 垂直方向取按钮中心。
        load_more.y + load_more.h * 0.5,
    );
    // 在真实加载按钮内部按下主指针。
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            // 使用刚才计算的按钮内部坐标。
            pos: click_position,
            // 使用标准左键。
            button: MouseButton::Left,
            // 不使用修饰键。
            mods: KeyMod::NONE,
        }),
        // 真实按钮必须处理按下。
        EventResult::Handled
    );
    // 在同一按钮内部释放主指针。
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            // 使用与按下相同的位置。
            pos: click_position,
            // 使用配对左键。
            button: MouseButton::Left,
            // 不使用修饰键。
            mods: KeyMod::NONE,
        }),
        // 真实按钮必须处理释放并合成点击。
        EventResult::Handled
    );
    // List 父组件不得吞掉或复制子按钮点击。
    assert_eq!(clicks.get(), 1);
}

// 验证固定角色 key 在中间插槽增删时保留已有子树身份。
#[test]
// 测试名称覆盖协调身份和移除释放。
fn keyed_roles_survive_reconcile_when_other_slot_changes() {
    // 初始声明只包含页首和加载入口。
    let mut tree = ViewAdapter::build(
        // 非空数据保证 List 而非 Empty 成为根。
        List::new()
            // 建立单行文本内容。
            .items(vec!["任务"])
            // 建立页首角色。
            .header_view(ViewNode::leaf(Button::new("页首")))
            // 建立加载入口角色。
            .load_more_view(ViewNode::leaf(Button::new("加载"))),
    );
    // 取得 List 根身份。
    let root = tree.root_id().expect("List 根必须存在");
    // 保存初始两个角色身份。
    let first = tree.get(root).expect("List 根必须可读").children().to_vec();
    // 初始页首与加载入口必须同时存在。
    assert_eq!(first.len(), 2);
    // 插入页尾并更新三个角色内容。
    ViewAdapter::reconcile(
        // 原位更新现有组件树。
        &mut tree,
        // 相同角色使用 List 私有固定 key。
        List::new()
            // 保持根仍为非空 List。
            .items(vec!["新任务"])
            // 更新页首内容但保留角色身份。
            .header_view(ViewNode::leaf(Button::new("新页首")))
            // 新增中间页尾角色。
            .footer_view(ViewNode::leaf(Button::new("页尾")))
            // 更新加载文字但保留角色身份。
            .load_more_view(ViewNode::leaf(Button::new("继续加载"))),
    );
    // 读取协调后的三个角色身份。
    let next = tree.get(root).expect("List 根必须可读").children().to_vec();
    // 新增页尾后必须产生三个角色。
    assert_eq!(next.len(), 3);
    // 固定页首 key 必须保留原 WidgetId。
    assert_eq!(next[0], first[0]);
    // 固定加载入口 key 不得因中间插槽插入而错配。
    assert_eq!(next[2], first[1]);
    // 再次协调为只有加载入口的 List。
    ViewAdapter::reconcile(
        // 原位更新同一组件树。
        &mut tree,
        // 移除页首与页尾。
        List::new()
            // 保持非空 List 根类型稳定。
            .items(vec!["任务"])
            // 只保留加载入口角色。
            .load_more_view(ViewNode::leaf(Button::new("加载"))),
    );
    // 读取最终唯一角色身份。
    let final_children = tree.get(root).expect("List 根必须可读").children();
    // 只有加载入口应当留在树中。
    assert_eq!(final_children, &[first[1]]);
    // 已移除页首身份必须完成正常树清理。
    assert!(tree.get(first[0]).is_none());
    // 已移除页尾身份必须完成正常树清理。
    assert!(tree.get(next[1]).is_none());
}

// 验证空数据仍以 Empty 替换整个 List 而不挂载声明插槽。
#[test]
// 测试名称覆盖空数据替代与子树所有权边界。
fn empty_data_replaces_list_and_drops_declared_slots() {
    // 构造空数据但声明全部真实节点插槽的 List。
    let tree = ViewAdapter::build(
        // 缺省数据为空。
        List::new()
            // 声明页首节点。
            .header_view(ViewNode::leaf(Button::new("页首")))
            // 声明页尾节点。
            .footer_view(ViewNode::leaf(Button::new("页尾")))
            // 声明加载入口节点。
            .load_more_view(ViewNode::leaf(Button::new("加载"))),
    );
    // 取得物化后的根身份。
    let root = tree.root_id().expect("Empty 根必须存在");
    // 根组件必须是现有 Empty 替代而非空 List 壳。
    assert!(
        tree.get(root)
            // 根节点必须可读。
            .expect("Empty 根必须可读")
            // 读取具体组件类型。
            .widget()
            // 进入动态类型检查。
            .as_any()
            // 验证现有 Empty 组件。
            .downcast_ref::<Empty>()
            // 确认转换成功。
            .is_some()
    );
    // 被替代的 List 插槽不得泄漏为 Empty 子节点。
    assert!(
        tree.get(root)
            .expect("Empty 根必须可读")
            .children()
            .is_empty()
    );
}

// 验证节点存在事实进入 List 快照但不复制子树快照。
#[test]
// 测试名称覆盖兼容字符串与节点角色的互斥事实。
fn snapshot_distinguishes_text_and_view_slots() {
    // 构造三个兼容文本角色。
    let text = List::new()
        // 保持非空数据契约。
        .items(vec!["任务"])
        // 配置文本页首。
        .header("页首")
        // 配置文本页尾。
        .footer("页尾")
        // 配置文本加载入口。
        .load_more("加载");
    // 构造三个真实节点角色。
    let views = List::new()
        // 保持非空数据契约。
        .items(vec!["任务"])
        // 配置节点页首。
        .header_view(ViewNode::leaf(Button::new("页首")))
        // 配置节点页尾。
        .footer_view(ViewNode::leaf(Button::new("页尾")))
        // 配置节点加载入口。
        .load_more_view(ViewNode::leaf(Button::new("加载")));
    // 读取兼容文本快照。
    let SnapshotFields::List {
        // 读取兼容页首文本。
        header,
        // 读取三个节点事实。
        header_view,
        footer_view,
        load_more_view,
        // 忽略其他既有字段。
        ..
    } = text.snapshot_fields()
    else {
        // List 只能发布 List 快照变体。
        panic!("List 必须发布 List 快照变体");
    };
    // 兼容页首文本必须保留原值。
    assert_eq!(header, "页首");
    // 兼容文本路径不得伪造节点角色。
    assert!(!header_view && !footer_view && !load_more_view);
    // 读取节点型 List 快照。
    let SnapshotFields::List {
        // 节点角色必须清除重复文本。
        header,
        footer,
        load_more_text,
        // 读取三个节点存在事实。
        header_view,
        footer_view,
        load_more_view,
        // 忽略其他既有字段。
        ..
    } = views.snapshot_fields()
    else {
        // List 只能发布 List 快照变体。
        panic!("List 必须发布 List 快照变体");
    };
    // 节点角色不得复制任何兼容文本。
    assert!(header.is_empty() && footer.is_empty() && load_more_text.is_empty());
    // 快照必须精确发布三个节点存在事实。
    assert!(header_view && footer_view && load_more_view);
}

// 验证同一角色最后调用的兼容文本或真实节点构建器取得所有权。
#[test]
// 测试名称覆盖 Rust 构建器兼容顺序。
fn last_builder_owns_each_slot_role() {
    // 节点后调用文本应恢复兼容页首。
    let text_last = List::new()
        // 先声明真实页首。
        .header_view(ViewNode::leaf(Button::new("节点")))
        // 再声明兼容文本。
        .header("文本");
    // 最后文本调用必须关闭节点角色。
    assert_eq!(text_last.slot_view_count(), 0);
    // 文本必须成为最终快照来源。
    assert!(matches!(
        text_last.snapshot_fields(),
        SnapshotFields::List {
            header,
            header_view: false,
            ..
        } if header == "文本"
    ));
    // 文本后调用节点应取得页首角色。
    let view_last = List::new()
        // 先声明兼容文本。
        .header("文本")
        // 再声明真实页首。
        .header_view(ViewNode::leaf(Button::new("节点")));
    // 最后节点调用必须建立一个真实角色。
    assert_eq!(view_last.slot_view_count(), 1);
    // 直接 trait 测量在子树尚未登记时必须安全回退。
    assert_eq!(
        WidgetLayout::measure(&view_last, Constraints::unconstrained()),
        // 空数据和未布局节点仍保留旧最小尺寸。
        Size::new(400.0, 100.0)
    );
}

// 验证非空 List 经同目录 UIX 根注入尺寸、行高与留白视觉。
#[test]
fn uix_root_preserves_list_kernel_and_visual_contract() {
    let node = crate::ui::view::View::build(List::new().items(vec!["任务"]));
    assert!(node.children.is_empty());
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<List>()
        .expect("UIX 根必须保留 List 内核");
    assert!(kernel.bordered);
    assert_eq!(
        kernel.visual_contract_for_test(),
        (400.0, 100.0, 32.0, 40.0, 48.0, 16.0)
    );
}

// 验证 List 实例共享 UIX 视觉表，显式边框开关仍保持最高优先级。
#[test]
fn list_instances_share_uix_visual_table_and_preserve_authored_border() {
    // 构建前默认值直接读取 UIX 生成的唯一静态视觉事实。
    assert!(std::ptr::eq(List::new().visual, LIST_VISUAL_REF));
    let first = crate::ui::view::View::build(List::new().items(vec!["一"]));
    let second = crate::ui::view::View::build(List::new().items(vec!["二"]).bordered(false));
    let first = first.widget.as_any().downcast_ref::<List>().unwrap();
    let second = second.widget.as_any().downcast_ref::<List>().unwrap();
    assert!(first.shares_visual_with_for_test(second));
    assert!(std::ptr::eq(first.visual, LIST_VISUAL_REF));
    assert!(!second.bordered);
}
