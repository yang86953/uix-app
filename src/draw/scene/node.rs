//! 场景图节点标识（与 `core::ComponentId` 同型，draw 不依赖 ui 域）。

/// 节点标识：scene 拥有节点身份，renderer 经 scene 消费。
pub type NodeId = crate::core::ComponentId;
