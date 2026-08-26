// 引入被测公开条目与组件。
use super::{Anchor, AnchorItem};

// 验证同长度重排按 href 保持选择并使旧位置缓存失效。
#[test]
// 声明 Anchor 重排回归测试。
fn reconcile_preserves_active_href_and_invalidates_reordered_positions() {
    // 构造两个具有稳定 href 的锚点。
    let mut anchor = Anchor::new(vec![
        // 登记首个滚动目标。
        AnchorItem::new("概览", "overview"),
        // 登记次个滚动目标。
        AnchorItem::new("设置", "settings"),
    ]);
    // 注入与旧声明顺序对应的位置缓存。
    anchor.set_positions(vec![0.0, 100.0]);
    // 模拟滚动到第二个目标之后。
    anchor.update_active(200.0);
    // 前置条件必须选中 settings。
    assert_eq!(anchor.active_href(), "settings");

    // 构造 href 顺序颠倒的新声明树。
    let next = Anchor::new(vec![
        // 把当前目标移动到首位。
        AnchorItem::new("设置", "settings"),
        // 把原首项目标移动到末位。
        AnchorItem::new("概览", "overview"),
    ]);
    // 执行声明树 reconcile。
    anchor.sync_from(next);

    // 当前项必须跟随稳定 href 到新索引。
    assert_eq!(anchor.active_index(), 0);
    // 当前 href 不得因旧索引而串到 overview。
    assert_eq!(anchor.active_href(), "settings");
    // 旧位置顺序已失配，必须等待调用方重新注入。
    assert_eq!(anchor.anchor_positions, vec![0.0, 0.0]);
}

// 验证同一 href 序列保留位置缓存且空集合保持安全。
#[test]
// 声明 Anchor 稳定序列与空集合回归测试。
fn reconcile_keeps_matching_positions_and_normalizes_empty_items() {
    // 构造稳定的两项目标序列。
    let mut anchor = Anchor::new(vec![
        // 登记首个目标。
        AnchorItem::new("旧概览", "overview"),
        // 登记次个目标。
        AnchorItem::new("旧设置", "settings"),
    ]);
    // 注入调用方测得的位置。
    anchor.set_positions(vec![16.0, 120.0]);
    // 激活第二个滚动目标。
    anchor.update_active(240.0);

    // 仅更新显示标题并保持 href 序列。
    let renamed = Anchor::new(vec![
        // 首项目标身份不变。
        AnchorItem::new("新概览", "overview"),
        // 次项目标身份不变。
        AnchorItem::new("新设置", "settings"),
    ]);
    // 执行仅文案变化的 reconcile。
    anchor.sync_from(renamed);

    // 稳定 href 仍保持第二项选择。
    assert_eq!(anchor.active_href(), "settings");
    // 相同目标序列继续复用有效位置缓存。
    assert_eq!(anchor.anchor_positions, vec![16.0, 120.0]);

    // 用空声明树移除全部目标。
    anchor.sync_from(Anchor::new(Vec::new()));
    // 空集合的公开索引保持安全零值。
    assert_eq!(anchor.active_index(), 0);
    // 空集合没有伪造 href。
    assert_eq!(anchor.active_href(), "");
    // 空集合同步清空位置缓存。
    assert!(anchor.anchor_positions.is_empty());
}
