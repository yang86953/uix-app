//! app System 私有边界事件契约（SystemEvent(SMC)）。
//!
//! # 归属（通用 SMC 文档）
//!
//! 本模块持有 app System 跨 Module 的事实广播契约（SystemEvent(SMC)）；
//! 任何 Module 不拥有（与 `window_semantics` / `queues` 同级别的 System
//! 私有边界内容）。事件只在所属边界越过事实建立点后发布；订阅在组装期
//! 注册，每条订阅进入路由清单（`docs/架构/路由清单.md`）。
//!
//! # 试点（阶段 D，2026-08-03）
//!
//! 首个接入场景：主题变更事实。事实建立点为 `apply_runtime_theme_change`
//! 完成主题替换后（新主题已写入主题槽并分发给全部窗口 UI 树）；
//! 失败或回滚时不得发布。感知方：主题变更诊断遥测（tracing）。

use crate::bus::{EventBus, Publisher};

/// 主题已生效事实（SystemEvent(SMC)，app System 私有边界拥有）。
///
/// 负载为不可变快照（深色标志 + 单调递增应用序次），不携带可变引用、
/// 回调或资源句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThemeApplied {
    /// 新主题是否为深色模式。
    pub is_dark: bool,
    /// 应用序次（单调递增，用于区分重复应用同一主题）。
    pub revision: u64,
}

/// 主题事实总线类型：app System 私有边界唯一拥有的实例。
///
/// 组合根/应用组装处创建并注入窄能力；禁止全局单例或跨 System 共享。
// 该窄总线是组装契约，当前默认应用路径尚未接入。
#[allow(dead_code)]
pub(crate) type ThemeEventBus = EventBus;

/// 主题事实窄发布端口（P-06）：业务代码只取得本端口，不接触完整 Bus。
// 该发布端口保留给主题组装根，当前默认应用路径尚未接入。
#[allow(dead_code)]
pub(crate) type ThemeEventPublisher = Publisher<ThemeApplied>;

#[cfg(test)]
mod tests {
    //! 契约与链路验证：精确类型分发、事实负载快照、订阅注销语义。

    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;
    use crate::bus::Subscription;

    /// 便捷断言：把 Result 转为值，测试中失败直接 panic。
    fn ok<T>(r: crate::core::error::Result<T>) -> T {
        match r {
            Ok(v) => v,
            Err(e) => panic!("unexpected error: {}", e.short_what()),
        }
    }

    #[test]
    fn theme_applied_dispatches_to_subscriber() {
        // 主题事实总线：订阅者收到 ThemeApplied 精确类型负载。
        let mut bus = ThemeEventBus::new();
        let seen = Rc::new(Cell::new(None::<ThemeApplied>));
        let s = Rc::clone(&seen);
        let _sub = ok(bus.subscribe(move |fact: &ThemeApplied| {
            s.set(Some(*fact));
        }));
        ok(bus.publish(ThemeApplied {
            is_dark: true,
            revision: 1,
        }));
        assert_eq!(
            seen.get(),
            Some(ThemeApplied {
                is_dark: true,
                revision: 1
            })
        );
    }

    #[test]
    fn narrow_publisher_injects_single_fact_capability() {
        // 窄发布端口（P-06）：只发布 ThemeApplied，不暴露完整 Bus。
        let mut bus = ThemeEventBus::new();
        let port: ThemeEventPublisher = bus.publisher();
        let seen = Rc::new(Cell::new(0u64));
        let s = Rc::clone(&seen);
        let _sub = ok(bus.subscribe(move |fact: &ThemeApplied| {
            s.set(fact.revision);
        }));
        ok(port.publish(ThemeApplied {
            is_dark: false,
            revision: 7,
        }));
        assert_eq!(seen.get(), 7);
    }

    #[test]
    fn subscription_scope_handle_unsubscribes_on_drop() {
        // 订阅句柄 Drop 即注销：后续发布不再投递。
        let mut bus = ThemeEventBus::new();
        let hits = Rc::new(Cell::new(0u32));
        let h = Rc::clone(&hits);
        let sub: Subscription = ok(bus.subscribe(move |_: &ThemeApplied| {
            h.set(h.get() + 1);
        }));
        ok(bus.publish(ThemeApplied {
            is_dark: true,
            revision: 1,
        }));
        assert_eq!(hits.get(), 1);
        drop(sub);
        ok(bus.publish(ThemeApplied {
            is_dark: true,
            revision: 2,
        }));
        assert_eq!(hits.get(), 1);
    }
}
