//! RenderObject 树 — Widget 绘制与合成之间的 retained 层（Phase 9）。
//!
//! 通过 `ScenePaint` 快照构建，按 widget 缓存 Content 阶段 `DisplayList`，
//! 使 invalidation / 录制 / 重放对齐，而不依赖 uix-ui。

mod tree;

pub use tree::{RenderObjectEntry, RenderObjectTree};
