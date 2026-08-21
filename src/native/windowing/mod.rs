//! platform `windowing` Module：原生窗口、事件源、输入、剪贴板与 IME。
//!
//! SMC 边界：本 Module 是 platform System 的私有实现，由 System 私有边界
//! （`crate::native` 根）拥有。职责：创建/销毁窗口、窗口属性与原生动作、
//! wait/wake/drain 事件、输入服务与 IME 会话；OS callback 只采集数据并
//! 投递目标窗口事件，应用逻辑在 app/ui 的 owner thread 执行。
//!
//! 契约与实现：窗口与事件协议位于 `crate::platform::windowing`，`input`
//! 暂存尚未迁移的原生窄能力契约；OS 提供者在 `backends` 中实现，`shared`
//! 只保存窗口与事件共享状态。
//! 本 Module 只依赖 core，不依赖兄弟 Module。

pub(crate) mod input;
pub(crate) mod shared;

pub(crate) use input::*;
