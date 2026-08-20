// 引入受测私有同步与选择入口。
use super::{SelectableItem, SelectableList};
// 引入语义事件与受控状态测试类型。
use crate::ui::{ComponentId, EventHandler, SemanticKind, SemanticPayload, State, SystemEvent};

// 构造两个具有稳定 id 的列表条目。
fn items() -> Vec<SelectableItem> {
    // 返回顺序固定的测试数据。
    vec![
        // 首项使用 alpha 稳定身份。
        SelectableItem::new("alpha", "Alpha"),
        // 次项使用 beta 稳定身份。
        SelectableItem::new("beta", "Beta"),
    ]
}

// 验证外部空值和失效 id 保持调用方所有且界面无选中项。
#[test]
fn controlled_active_keeps_empty_and_unmatched_state() {
    // 初始外部状态不选择任何条目。
    let active = State::new(None::<String>);
    // 先提供数据再绑定受控状态。
    let mut list = SelectableList::new().items(items()).active_state(&active);
    // 空状态不得伪装成首项选择。
    assert_eq!(list.selected_id(), None);

    // 调用方随后提供集合中不存在的稳定 id。
    active.set(Some("missing".to_string()));
    // 模拟下一次交互前的受控同步。
    list.sync_bound_active();

    // 失效 id 仍不得显示成任意条目。
    assert_eq!(list.selected_id(), None);
    // 运行时不得反向归一化调用方拥有的状态。
    assert_eq!(active.get(), Some("missing".to_string()));
}

// 验证用户选择先写回状态并只产生一次稳定 id Change。
#[test]
fn controlled_selection_writes_state_before_single_change() {
    // 初始受控状态不选择任何条目。
    let active = State::new(None::<String>);
    // 构造受控列表。
    let mut list = SelectableList::new().items(items()).active_state(&active);

    // 模拟用户激活第二项。
    list.select(1, true);
    // 语义事件取走前外部状态必须已经更新。
    assert_eq!(active.get(), Some("beta".to_string()));
    // 从组件取走本次变化事件。
    let event = list
        .semantic_event(ComponentId::new(7), &SystemEvent::FocusIn)
        .expect("用户选择应产生 Change 事件");

    // 事件类型必须是统一的 Change。
    assert_eq!(event.kind, SemanticKind::Change);
    // 载荷必须使用稳定条目 id 而不是索引或展示文字。
    assert!(matches!(event.payload, SemanticPayload::Text(value) if value == "beta"));
    // 同一次选择不得被重复发布。
    assert!(
        list.semantic_event(ComponentId::new(7), &SystemEvent::FocusIn)
            .is_none()
    );
}

// 验证未绑定构建器继续保留原有索引初值。
#[test]
fn uncontrolled_active_index_remains_compatible() {
    // 使用既有 active(index) 构建器选择第二项。
    let list = SelectableList::new().items(items()).active(1);
    // 非受控模式仍按既有索引返回稳定 id。
    assert_eq!(list.selected_id(), Some("beta"));
}
