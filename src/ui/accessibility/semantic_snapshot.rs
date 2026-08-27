//! Transport-neutral semantic tree snapshots.
//!
//! Headless tests, the event-driven JSON recorder, and the opt-in Agent Bridge
//! all consume this model so selectors, redaction, bounds, and action
//! capabilities cannot drift between control surfaces.

use crate::core::{Point, Rect, WidgetId};
use crate::ui::semantic_action::SemanticActionKind;
use crate::ui::widget_runtime::widget::{WidgetCore, WidgetTree};
use crate::ui::widget_snapshot::{AccessibilitySnapshot, SelectionSnapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
/// 自动化与 Agent 共享的稳定语义目标。
pub enum SemanticTarget {
    NodeId(WidgetId),
    AutomationId(String),
}

impl SemanticTarget {
    pub(crate) fn label(&self) -> String {
        match self {
            Self::NodeId(id) => id.to_string(),
            Self::AutomationId(id) => id.clone(),
        }
    }
}

impl From<WidgetId> for SemanticTarget {
    fn from(id: WidgetId) -> Self {
        Self::NodeId(id)
    }
}

impl From<String> for SemanticTarget {
    fn from(id: String) -> Self {
        Self::AutomationId(id)
    }
}

impl From<&str> for SemanticTarget {
    fn from(id: &str) -> Self {
        Self::AutomationId(id.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq)]
/// 与具体窗口后端无关的公开语义节点快照。
pub struct SemanticNode {
    pub id: WidgetId,
    pub automation_id: Option<String>,
    pub parent: Option<WidgetId>,
    pub frame: Rect,
    pub visible_bounds: Option<Rect>,
    pub focused: bool,
    /// 该节点当前是否为指针悬停目标；供自动化读取悬停事实。
    pub hovered: bool,
    pub accessibility: AccessibilitySnapshot,
    pub selection: Option<SelectionSnapshot>,
    pub actions: Vec<SemanticActionKind>,
}

// These conveniences are part of the public `test-harness` alias today; the
// transport-neutral node itself is also compiled for opt-in session tracking.
#[cfg_attr(not(feature = "test-harness"), allow(dead_code))]
impl SemanticNode {
    /// 判断节点当前是否具有可见命中区域。
    pub fn is_visible(&self) -> bool {
        self.visible_bounds.is_some()
    }

    /// 判断节点当前是否允许语义交互。
    pub fn is_enabled(&self) -> bool {
        !self.accessibility.state.disabled
    }

    /// 返回当前可见区域的中心点。
    pub fn center(&self) -> Option<Point> {
        let bounds = self.visible_bounds?;
        Some(Point::new(
            bounds.x + bounds.w * 0.5,
            bounds.y + bounds.h * 0.5,
        ))
    }

    /// 判断节点是否公开指定语义动作。
    pub fn supports(&self, action: SemanticActionKind) -> bool {
        self.actions.contains(&action)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SemanticSnapshotBody {
    pub(crate) nodes: Vec<SemanticNode>,
}

impl WidgetTree {
    fn exposes_semantic_node(&self, id: WidgetId) -> bool {
        if self.is_pending_removal_subtree(id) {
            return false;
        }
        if self
            .get(id)
            .is_none_or(|node| node.accessibility().role == crate::ui::AccessibilityRole::None)
        {
            return false;
        }
        let mut ancestor = self.get(id).and_then(|node| node.parent());
        while let Some(parent_id) = ancestor {
            let Some(parent) = self.get(parent_id) else {
                return false;
            };
            if parent.accessibility().role == crate::ui::AccessibilityRole::None
                || !parent.widget().exposes_semantic_children()
            {
                return false;
            }
            ancestor = parent.parent();
        }
        true
    }

    pub(crate) fn semantic_snapshot_body(&self) -> SemanticSnapshotBody {
        // 停止树不得枚举半提交节点或读取组件语义快照。
        if !self.accepts_external_work() {
            // 保持公开快照签名，并以合法空体表达无可访问节点。
            return SemanticSnapshotBody::default();
        }
        let focused = self.managers().focus.focused_widget();
        let hovered = self.managers().interaction.hovered_widget();
        let nodes = self
            .traverse()
            .iter()
            .copied()
            .filter(|id| self.exposes_semantic_node(*id))
            .filter_map(|id| {
                let node = self.get(id)?;
                let snapshot = node.widget_snapshot(id);
                let mut accessibility = snapshot.accessibility();
                let selection = snapshot.selection();
                if !accessibility.state.password {
                    if let Some(value) =
                        crate::ui::tree_widget_hooks::widget_input_value(node.widget())
                    {
                        accessibility.state.value_text = (!value.is_empty()).then_some(value);
                    }
                }
                let actions = self.supported_semantic_actions(id);
                Some(SemanticNode {
                    id,
                    automation_id: node.automation_id().map(str::to_owned),
                    parent: node.parent(),
                    frame: self
                        .node_visual_rect(id, node.frame())
                        .unwrap_or_else(|| node.frame()),
                    visible_bounds: self.visible_rect_for(id),
                    focused: focused == Some(id),
                    hovered: hovered == Some(id),
                    accessibility,
                    selection,
                    actions,
                })
            })
            .collect();
        SemanticSnapshotBody { nodes }
    }
}
