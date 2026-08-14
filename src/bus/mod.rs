//! 进程内同步类型化事件分发基础设施。
//!
//! # SMC 定级（系统列表）
//!
//! `bus` 是与 `core` 同级、按模式外基础依赖审计的候选领域边界（待定级）：
//! 承载通用 SMC 定义的进程内 EventBus（可复用 Component 定义），不承担
//! 领域职责，也不作为第四运行层。每个 Module / System 创建自己的
//! [`EventBus`] 实例；禁止全局单例、静态可变 Bus 或跨 System 共享实例。
//! 普通业务 Component 只注入最窄的类型化发布能力，不取得完整 Bus。
//!
//! # 语义（对齐通用 SMC「Event 与进程内 EventBus」章节）
//!
//! - 事件 = 已经发生的不可变事实（[`Fact`]），按精确类型同步分发，
//!   不因继承、结构兼容或字段相似隐式扩大订阅；
//! - `publish` 返回前本轮匹配处理器已全部执行完毕，处理器沿用发布者
//!   执行上下文；处理器执行顺序不是业务契约；
//! - 处理器失败（panic）默认隔离并继续（P-05），技术分发报告
//!   （[`DispatchReport`]）不充当业务应答；
//! - 分发前先取处理器快照，不在注册表排他保护区内执行用户处理器；
//!   分发期间的注册 / 注销延迟到下一轮生效（快照语义）；
//! - 处理器内嵌套发布允许（调用栈顺序），深度上限
//!   [`MAX_DISPATCH_DEPTH`]；**同一处理器不可重入**：执行期间嵌套
//!   发布再次命中时跳过并计入 `DispatchReport::skipped`；
//! - 单线程同步基础设施（`Rc<RefCell>`，`!Send + !Sync`），线程亲和；
//!   并发语义 = 无（跨线程使用被类型系统拒绝）；
//! - 每条同步订阅进入路由清单（`docs/架构/路由清单.md`）。

mod event_bus;
mod subscription;

pub use event_bus::Publisher;
pub use event_bus::{BusState, DispatchReport, EventBus, MAX_DISPATCH_DEPTH};
pub use subscription::Subscription;

/// 事件契约标记：表达一个已经发生、可被零个或多个订阅者独立观察的
/// 不可变事实。
///
/// 负载安全规则（不得携带可变引用、回调、资源句柄、事务对象或发布者
/// 私有错误类型）由事件类型定义方按文档纪律保证，不由本 trait 强制。
pub trait Fact: 'static + Send + Sync {}

impl<T: 'static + Send + Sync> Fact for T {}
