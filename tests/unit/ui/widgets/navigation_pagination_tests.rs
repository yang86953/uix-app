// 引入待验证分页器。
use super::Pagination;
// 引入声明端状态句柄。
use crate::ui::State;

// 验证外部更新、用户选择与 pageSize 收敛共享同一双状态。
#[test]
// 声明双向绑定回归。
fn controlled_values_stay_bidirectional() {
    // 创建声明端 current 状态。
    let current = State::new(3_usize);
    // 创建声明端 pageSize 状态。
    let page_size = State::new(10_usize);
    // 构造受控分页器并启用条数切换。
    let mut pagination = Pagination::new(95, 10)
        // 先绑定 pageSize。
        .page_size_state(&page_size)
        // 再绑定 current。
        .current_state(&current)
        // 启用用户 pageSize 交互入口。
        .show_size_changer(true);
    // 初次物化必须读取两个外部值。
    assert_eq!(
        (pagination.get_current(), pagination.get_page_size()),
        (3, 10)
    );
    // 模拟用户选择第五页。
    pagination.select_page(5);
    // 用户页码变化必须写回 current State。
    assert_eq!(current.get(), 5);
    // 模拟用户切换到下一档 pageSize。
    pagination.cycle_page_size(true);
    // pageSize 必须写回二十。
    assert_eq!(page_size.get(), 20);
    // 外部业务把 pageSize 改成五十。
    page_size.set(50);
    // 下一次事件入口采用的同步 helper 吸收外部更新。
    pagination.sync_bound_values();
    // 九十五条在五十条每页时最多两页。
    assert_eq!(pagination.get_current(), 2);
    // current State 必须收到同一收敛值。
    assert_eq!(current.get(), 2);
    // 组件 pageSize 镜像必须与外部一致。
    assert_eq!(pagination.get_page_size(), 50);
}

// 验证非法状态归一化与声明树重建均服从新 State。
#[test]
// 声明 reconcile 双状态回归。
fn reconcile_prefers_declared_and_normalized_values() {
    // 创建越界 current。
    let current = State::new(99_usize);
    // 创建非法零 pageSize。
    let page_size = State::new(0_usize);
    // 初次受控构造执行双状态归一化。
    let mut pagination = Pagination::new(30, 10)
        // 先归一化 pageSize。
        .page_size_state(&page_size)
        // 再归一化 current。
        .current_state(&current);
    // 零 pageSize 必须归一化为一。
    assert_eq!(page_size.get(), 1);
    // 三十条、每页一条时最大页码为三十。
    assert_eq!(current.get(), 30);
    // 业务声明下一轮使用每页十五条和第二页。
    page_size.set(15);
    // 更新声明端 current。
    current.set(2);
    // 构造下一棵声明树中的受控分页器。
    let next = Pagination::new(45, 10)
        // 绑定更新后的 pageSize。
        .page_size_state(&page_size)
        // 绑定更新后的 current。
        .current_state(&current);
    // 让运行时复用旧实例并同步新声明。
    pagination.sync_from(next);
    // reconcile 必须采用最新双状态。
    assert_eq!(
        (pagination.get_current(), pagination.get_page_size()),
        (2, 15)
    );
    // 新 total 也必须进入快照行为。
    assert_eq!(pagination.total_pages(), 3);
}
