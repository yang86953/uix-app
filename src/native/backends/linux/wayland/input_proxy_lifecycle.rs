// Wayland 输入 capability 快照只允许触发确定性的代理边沿转换。

// 输入代理生命周期转换不保存协议对象或跨 Module 状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 该枚举供 seat owner 将完整 capability 快照映射为一次性操作。
pub(crate) enum InputProxyTransition {
    // capability 从无到有时创建唯一代理与回调。
    Bind,
    // capability 从有到无时清理回调并释放唯一代理。
    Release,
    // 重复快照不产生任何协议对象生命周期操作。
    Unchanged,
// 结束输入代理边沿枚举。
}

// 根据 capability 完整快照与当前代理槽位计算唯一边沿动作。
pub(crate) const fn input_proxy_transition(
    // 表示 compositor 本次快照是否声明该输入能力。
    capability_available: bool,
    // 表示 owner 当前是否持有对应协议代理。
    proxy_bound: bool,
// 返回创建、释放或保持不变的确定性决策。
) -> InputProxyTransition {
    // 完整匹配 capability 与代理现状，避免重复创建或重复释放。
    match (capability_available, proxy_bound) {
        // 仅无代理且能力可用时绑定。
        (true, false) => InputProxyTransition::Bind,
        // 仅有代理且能力消失时释放。
        (false, true) => InputProxyTransition::Release,
        // 其余相同状态快照保持幂等。
        _ => InputProxyTransition::Unchanged,
    // 结束 capability 边沿匹配。
    }
// 结束输入代理生命周期决策。
}
