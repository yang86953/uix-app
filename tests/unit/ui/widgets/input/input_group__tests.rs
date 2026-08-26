// 引入当前组合构建器。
use super::*;
// 引入 Input 快照读取契约。
use crate::ui::{SnapshotFields, SnapshotSource};

// 验证附加文本和双向状态落到同一个 Input。
#[test]
fn value_and_addons_share_one_input_contract() {
    // 创建复合输入的外部文本状态。
    let amount = State::new(String::from("12.50"));
    // 构造完整前后附加文本组合并取出内部输入。
    let input = InputGroup::new()
        // 设置货币前缀。
        .addon_before("¥")
        // 绑定文本值。
        .value(&amount)
        // 设置单位后缀。
        .addon_after("元")
        // 物化唯一内部输入。
        .into_input();
    // 内部输入必须读取绑定状态的当前值。
    assert_eq!(input.current_value(), "12.50");
    // 读取输入公开快照。
    let SnapshotFields::Input {
        // 提取前置附加文本。
        addon_before,
        // 提取后置附加文本。
        addon_after,
        // 忽略其他输入配置。
        ..
    } = input.snapshot_fields()
    else {
        // 快照类型不匹配表示组合边界破坏。
        panic!("InputGroup 必须物化为 Input 快照");
    };
    // 两侧附加文本必须完整保留。
    assert_eq!(addon_before, "¥");
    // 后置单位必须完整保留。
    assert_eq!(addon_after, "元");
}
