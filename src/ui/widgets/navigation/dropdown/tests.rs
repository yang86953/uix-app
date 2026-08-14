// 引入被测 Dropdown 私有运行时入口与公开数据类型。
use super::{Dropdown, DropdownItem};
// 引入事件行为 trait 以读取 Change 事实和焦点范围通知。
use crate::ui::component::traits::EventHandler;
// 引入组件基础 trait 以验证 trigger 子树挂载生命周期。
use crate::ui::component::traits::WidgetComponent;
// 引入组件渲染 trait 以验证浮层登记契约。
use crate::ui::component::traits::WidgetRender;
// 引入稳定测试组件身份与指针坐标。
use crate::core::{ComponentId, Point, Rect};
// 引入触发枚举、输入事件和语义载荷。
use crate::ui::{EventResult, KeyMod, MouseButton, SemanticPayload, SystemEvent, TriggerMode};
// 使用真实按钮 View builder 验证组合 trigger 所有权入口。
use crate::ui::widgets::button;

// 构造不携带业务含义的指针移动事件。
fn dummy_event() -> SystemEvent {
    // 语义读取只负责取走先前建立的选择事实。
    SystemEvent::PointerMove {
        // 使用稳定组件内坐标。
        pos: Point::new(0.0, 0.0),
        // 测试不携带组合修饰键。
        mods: KeyMod::NONE,
    }
}

// 提取 Dropdown 已建立的 Change 文本载荷。
fn change_payload(dropdown: &Dropdown) -> String {
    // 读取一次待发语义事件。
    let event = dropdown
        // 使用稳定测试身份消费选择事实。
        .semantic_event(ComponentId::default(), &dummy_event())
        // 有效选择必须发布 Change。
        .expect("Dropdown 选择应产生 Change");
    // 只接受稳定 key 文本载荷。
    match event.payload {
        // 返回拥有型业务 key。
        SemanticPayload::Text(value) => value,
        // 其他载荷形状表示公开契约回归。
        _ => panic!("Dropdown Change 应使用稳定 key 文本载荷"),
    }
}

// 验证同名 label 的选项使用独立 key 选择与发布事件。
#[test]
fn duplicate_labels_select_and_emit_stable_key() {
    // 构造两个展示文字相同但身份不同的选项。
    let mut dropdown = Dropdown::new("")
        // 把真实按钮子树交给 Dropdown owner。
        .trigger_view(button("更多"))
        // 提供 keyed 选项数据。
        .keyed_items(vec![
            // 首项使用 edit key。
            DropdownItem::from_text("操作", "edit"),
            // 次项使用 copy key。
            DropdownItem::from_text("操作", "copy"),
        ]);
    // 打开列表以建立可见行顺序。
    dropdown.open();

    // 直接选择第二条可见 keyed 选项。
    assert!(dropdown.select_index(1));

    // 内部选择事实必须保存业务 key。
    assert_eq!(dropdown.current_value(), Some("copy"));
    // Change 载荷不得退回重复 label。
    assert_eq!(change_payload(&dropdown), "copy");
}

// 验证 reconcile 按 key 保持选择与递归展开状态。
#[test]
fn reconcile_preserves_selection_and_groups_by_key() {
    // 构造带同名分组和独立 leaf 的当前声明。
    let mut current = Dropdown::new("")
        // 使用静态按钮作为当前 trigger 子树。
        .trigger_view(button("更多"))
        // 提供当前 keyed 数据树。
        .keyed_items(vec![
            // 同名分组使用稳定 group key。
            DropdownItem::from_text("操作", "group").children(vec![
                // 子项使用稳定 copy key。
                DropdownItem::from_text("复制", "copy"),
            ]),
            // 顶层编辑项使用独立 key。
            DropdownItem::from_text("操作", "edit"),
        ]);
    // 展开首个 keyed 分组。
    assert!(current.toggle_or_select(0));
    // 展开后选择其首个子项。
    assert!(current.select_index(1));
    // 下一声明调整顶层顺序但保留相同业务身份。
    let next = Dropdown::new("")
        // 下一声明重新提供同身份 trigger View。
        .trigger_view(button("更多"))
        // 提供重排后的 keyed 数据树。
        .keyed_items(vec![
            // 编辑项移动到首位。
            DropdownItem::from_text("操作", "edit"),
            // 分组移动到次位且保持 key。
            DropdownItem::from_text("操作", "group").children(vec![
                // 子项展示文字变化但保持 copy key。
                DropdownItem::from_text("复制项目", "copy"),
            ]),
        ]);

    // 执行组件树使用的原位声明刷新。
    current.sync_from(next);

    // 选择必须按 key 保持，不受 label 和顺序变化影响。
    assert_eq!(current.current_value(), Some("copy"));
    // 分组展开状态必须按稳定 key 保持。
    assert_eq!(current.expanded_keys, vec!["group".to_string()]);
    // 新顺序下 copy 是第三条可见行。
    assert_eq!(current.selected_index(), Some(2));
}

// 验证动态 keyed 数据门禁允许同名 label 并拒绝空 key 与重复 key。
#[test]
fn keyed_items_keep_first_unique_identity_with_diagnostics() {
    // 构造同名、空 key 与重复 key 混合数据。
    let dropdown = Dropdown::new("").keyed_items(vec![
        // 首个 dup key 取得身份所有权。
        DropdownItem::from_text("同名", "dup"),
        // 同名但不同 key 必须合法保留。
        DropdownItem::from_text("同名", "other"),
        // 后出现的重复 key 必须被拒绝。
        DropdownItem::from_text("重复", "dup"),
        // 空 key 无法成为新 UIX 选项身份。
        DropdownItem::from_text("空值", ""),
    ]);

    // 只有两个唯一非空 key 进入运行时列表。
    assert_eq!(dropdown.items.len(), 2);
    // 重复和空 key 分别产生一条可观察诊断。
    assert_eq!(dropdown.diagnostics().len(), 2);
}

// 验证 focus/contextMenu 触发与 trigger 子树挂载释放边界。
#[test]
fn composite_trigger_modes_and_child_lifecycle_are_owned() {
    // focus 模式由完整 trigger 子树焦点范围驱动。
    let mut focus_dropdown = Dropdown::new("")
        // 把真实按钮子树交给 Dropdown owner。
        .trigger_view(button("更多"))
        // 使用公开 focus 触发枚举。
        .trigger(TriggerMode::Focus);
    // 通过 System 私有子树提供器读取 owner 持有的完整 ViewNode。
    let provider = WidgetComponent::as_view_children(&focus_dropdown)
        // 组合 Dropdown 必须公开子树建造能力端口。
        .expect("组合 Dropdown 应提供 trigger ViewNode");
    // 首次物化必须恰好交付一个 trigger 子树。
    assert_eq!(provider.build_view_children().len(), 1);
    // 同一声明句柄不能重复挂载已经取走的子树。
    assert!(provider.build_view_children().is_empty());
    // 模拟唯一 trigger 子树完成挂载。
    WidgetComponent::on_children_changed(&mut focus_dropdown, 1);
    // 组合 owner 不与内部 trigger 重复进入 Tab 顺序。
    assert_eq!(WidgetComponent::tab_index(&focus_dropdown), 0);
    // 子树获得焦点时打开下拉层。
    assert_eq!(
        EventHandler::on_focus_within(&mut focus_dropdown, true),
        EventResult::Handled
    );
    // focus trigger 已进入打开状态。
    assert!(focus_dropdown.is_open());
    // 焦点离开完整子树时关闭下拉层。
    let _ = EventHandler::on_focus_within(&mut focus_dropdown, false);
    // 关闭事实立即可见，离场动画可继续呈现。
    assert!(!focus_dropdown.is_open());
    // 模拟 trigger 子树卸载释放。
    WidgetComponent::on_children_changed(&mut focus_dropdown, 0);
    // 缺失子树时 owner 恢复可聚焦入口，避免失去恢复路径。
    assert_eq!(WidgetComponent::tab_index(&focus_dropdown), 1);

    // contextMenu 模式只接受右键触发。
    let mut context_dropdown = Dropdown::new("").trigger(TriggerMode::ContextMenu);
    // 左键不得打开右键菜单。
    assert_eq!(
        EventHandler::on_event(
            &mut context_dropdown,
            &SystemEvent::PointerDown {
                // 坐标命中触发区域。
                pos: Point::new(4.0, 4.0),
                // 使用不被接受的左键。
                button: MouseButton::Left,
                // 不携带修饰键。
                mods: KeyMod::NONE,
            },
        ),
        EventResult::NotHandled
    );
    // 右键必须打开 contextMenu。
    assert_eq!(
        EventHandler::on_event(
            &mut context_dropdown,
            &SystemEvent::PointerDown {
                // 坐标继续命中触发区域。
                pos: Point::new(4.0, 4.0),
                // 使用 contextMenu 主按钮。
                button: MouseButton::Right,
                // 不携带修饰键。
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );
    // 右键触发后进入打开状态。
    assert!(context_dropdown.is_open());
}

// 打开时必须登记 Popover 浮层：外部点击关闭与 Esc 路由依赖 OverlayStack。
#[test]
fn open_dropdown_registers_popover_overlay_with_outside_dismiss() {
    // 构造带两个可见选项的打开下拉菜单。
    let mut dropdown = Dropdown::new("Menu").keyed_items(vec!["复制".into(), "粘贴".into()]);
    // 进入打开状态（行高 30 像素 × 2 + 触发区 32 像素）。
    dropdown.open();

    // 打开状态必须生成浮层登记。
    let overlay = WidgetRender::overlay_entry(
        &dropdown,
        ComponentId::default(),
        Rect::new(40.0, 50.0, 160.0, 32.0),
    )
    // 在场状态必须返回登记。
    .expect("打开的 Dropdown 应生成浮层登记");
    // 菜单与触发区共同构成命中范围。
    assert_eq!(
        overlay.bounds_rect(),
        Some(Rect::new(40.0, 50.0, 160.0, 32.0 + 60.0))
    );
    // 与 Select 弹层一致的 z 顺序。
    assert_eq!(overlay.z_index_value(), 900);
    // 外部点击必须注册为可取消端口。
    assert!(overlay.dismisses_on_outside());
    // 关闭后不再登记浮层：等离场动画结束后验证。
    dropdown.close();
    // 驱动关闭动画直到完全离场。
    while crate::ui::component::traits::WidgetAnimation::update_animation(&mut dropdown, 0.05) {}
    assert!(
        WidgetRender::overlay_entry(
            &dropdown,
            ComponentId::default(),
            Rect::new(40.0, 50.0, 160.0, 32.0),
        )
        .is_none()
    );
}
