//! Opt-in, per-window semantic revision state.
//!
//! Tracking is disabled until a recorder or Agent Bridge explicitly enables
//! it. A disabled session therefore performs no semantic-tree traversal.

use crate::core::WindowId;
use crate::ui::WidgetTree;
use crate::ui::accessibility::semantic_snapshot::{SemanticNode, SemanticSnapshotBody};

const INITIAL_WINDOW_GENERATION: u64 = 1;

/// 语义发布端口契约（SMC-06 P1 修复）：本边界持有能力端口，`agent` Module
/// 提供实现（`AgentWindowRegistration`），组合根组装期注入。
pub(crate) trait AgentSemanticsPort: std::fmt::Debug {
    fn window_id(&self) -> WindowId;
    fn generation(&self) -> u64;
    fn publish_semantics(&self, snapshot: &WindowSemanticSnapshot);
    fn publish_window_state(&self, state: AgentWindowState);
}

/// UI turn 发布给 Agent 目录的跨平台窗口状态；不包含原生句柄或平台枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentWindowState {
    pub(crate) visible: bool,
    pub(crate) presentable: bool,
    pub(crate) logical_width: i32,
    pub(crate) logical_height: i32,
    pub(crate) maximized: bool,
    pub(crate) minimized: bool,
    pub(crate) fullscreen: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WindowSemanticSnapshot {
    pub(crate) window_id: WindowId,
    pub(crate) generation: u64,
    pub(crate) revision: u64,
    pub(crate) presented_revision: u64,
    pub(crate) closed: bool,
    pub(crate) nodes: Vec<SemanticNode>,
}

#[derive(Debug)]
pub(crate) struct WindowSemanticState {
    enabled: bool,
    snapshot: WindowSemanticSnapshot,
    agent_window: Option<Box<dyn AgentSemanticsPort>>,
}

impl WindowSemanticState {
    pub(crate) fn new(window_id: WindowId) -> Self {
        Self {
            enabled: false,
            snapshot: WindowSemanticSnapshot {
                window_id,
                generation: INITIAL_WINDOW_GENERATION,
                revision: 0,
                presented_revision: 0,
                closed: false,
                nodes: Vec::new(),
            },
            agent_window: None,
        }
    }

    pub(crate) fn bind_agent_window(&mut self, registration: Box<dyn AgentSemanticsPort>) -> bool {
        if self.snapshot.closed || registration.window_id() != self.snapshot.window_id {
            return false;
        }
        self.snapshot.generation = registration.generation();
        registration.publish_semantics(&self.snapshot);
        self.agent_window = Some(registration);
        true
    }

    pub(crate) fn enable(&mut self, tree: &WidgetTree) -> bool {
        if self.enabled || self.snapshot.closed {
            return false;
        }
        self.enabled = true;
        self.snapshot.nodes = tree.semantic_snapshot_body().nodes;
        self.snapshot.revision = 1;
        self.publish_semantics();
        true
    }

    pub(crate) fn refresh(&mut self, tree: &WidgetTree) -> bool {
        if !self.enabled || self.snapshot.closed {
            return false;
        }
        let SemanticSnapshotBody { nodes } = tree.semantic_snapshot_body();
        if self.snapshot.nodes == nodes {
            return false;
        }
        self.snapshot.nodes = nodes;
        self.bump_revision();
        self.publish_semantics();
        true
    }

    pub(crate) fn mark_presented(&mut self) -> bool {
        if !self.enabled
            || self.snapshot.closed
            || self.snapshot.presented_revision == self.snapshot.revision
        {
            return false;
        }
        self.snapshot.presented_revision = self.snapshot.revision;
        self.publish_semantics();
        true
    }

    pub(crate) fn close(&mut self) -> bool {
        if !self.enabled || self.snapshot.closed {
            return false;
        }
        self.snapshot.closed = true;
        self.snapshot.nodes.clear();
        self.bump_revision();
        self.publish_semantics();
        true
    }

    pub(crate) fn publish_agent_window_state(&self, state: AgentWindowState) {
        if let Some(agent_window) = self.agent_window.as_ref() {
            agent_window.publish_window_state(state);
        }
    }

    pub(crate) fn snapshot(&self) -> Option<&WindowSemanticSnapshot> {
        self.enabled.then_some(&self.snapshot)
    }

    fn bump_revision(&mut self) {
        self.snapshot.revision = self.snapshot.revision.saturating_add(1).max(1);
    }

    fn publish_semantics(&self) {
        if let Some(agent_window) = self.agent_window.as_ref() {
            agent_window.publish_semantics(&self.snapshot);
        }
    }
}
