//! 作用域订阅句柄：唯一释放责任方，幂等注销，失效空壳。

use std::cell::RefCell;
use std::rc::Weak;

use super::event_bus::{BusState, Registry};

/// 作用域订阅句柄。
///
/// 生命周期契约（通用 SMC I-12）：
///
/// - 由订阅方持有，是唯一释放责任方；显式 [`Subscription::unsubscribe`]
///   或 Drop 时自动注销，释放幂等；
/// - Bus 关闭后句柄变为失效空壳：`is_active` 为 false，释放安全，
///   不继续代表活动绑定；
/// - Bus 销毁后（Weak 升级失败）释放同样安全；
/// - 注销从注册表移除后，已取得快照或正在执行的处理器仍可能访问
///   接收者；接收者销毁前必须满足静默注销、完成栅栏、安全所有权或
///   原子有效性检查之一（由接收方负责）。
pub struct Subscription {
    pub(crate) registry: Weak<RefCell<Registry>>,
    pub(crate) id: usize,
}

impl Subscription {
    /// 显式注销（幂等）：从注册表移除本绑定。
    ///
    /// 注册表不存在（Bus 已销毁）或条目已不存在（重复释放）时均为
    /// 安全 no-op。
    pub fn unsubscribe(&mut self) {
        if let Some(reg) = self.registry.upgrade() {
            reg.borrow_mut().unsubscribe(self.id);
        }
    }

    /// 是否仍绑定有效（Active）Bus。
    ///
    /// Bus 关闭或销毁后返回 false：句柄只是失效空壳。
    pub fn is_active(&self) -> bool {
        self.registry
            .upgrade()
            .is_some_and(|reg| reg.borrow().state == BusState::Active)
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        // 释放责任方唯一：Drop 时自动注销（幂等，重复调用安全）。
        self.unsubscribe();
    }
}
