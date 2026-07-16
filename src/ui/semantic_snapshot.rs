//! Transport-neutral semantic tree snapshots.
//!
//! Headless tests, the event-driven JSON recorder, and the opt-in Agent Bridge
//! all consume this model so selectors, redaction, bounds, and action
//! capabilities cannot drift between control surfaces.

use crate::core::{ComponentId, Point, Rect};
use crate::ui::component_snapshot::{AccessibilitySnapshot, SelectionSnapshot};
use crate::ui::core::widget::{WidgetCore, WidgetTree};
use crate::ui::semantic_action::SemanticActionKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticTarget {
    NodeId(ComponentId),
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

impl From<ComponentId> for SemanticTarget {
    fn from(id: ComponentId) -> Self {
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
pub struct SemanticNode {
    pub id: ComponentId,
    pub automation_id: Option<String>,
    pub parent: Option<ComponentId>,
    pub frame: Rect,
    pub visible_bounds: Option<Rect>,
    pub focused: bool,
    pub accessibility: AccessibilitySnapshot,
    pub selection: Option<SelectionSnapshot>,
    pub actions: Vec<SemanticActionKind>,
}

// These conveniences are part of the public `test-harness` alias today; the
// transport-neutral node itself is also compiled for opt-in session tracking.
#[cfg_attr(not(feature = "test-harness"), allow(dead_code))]
impl SemanticNode {
    pub fn is_visible(&self) -> bool {
        self.visible_bounds.is_some()
    }

    pub fn is_enabled(&self) -> bool {
        !self.accessibility.state.disabled
    }

    pub fn center(&self) -> Option<Point> {
        let bounds = self.visible_bounds?;
        Some(Point::new(
            bounds.x + bounds.w * 0.5,
            bounds.y + bounds.h * 0.5,
        ))
    }

    pub fn supports(&self, action: SemanticActionKind) -> bool {
        self.actions.contains(&action)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SemanticSnapshotBody {
    pub(crate) nodes: Vec<SemanticNode>,
}

impl WidgetTree {
    fn exposes_semantic_node(&self, id: ComponentId) -> bool {
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
                || !parent.component().exposes_semantic_children()
            {
                return false;
            }
            ancestor = parent.parent();
        }
        true
    }

    pub(crate) fn semantic_snapshot_body(&self) -> SemanticSnapshotBody {
        let focused = self.managers().focus.focused_component();
        let nodes = self
            .traverse()
            .iter()
            .copied()
            .filter(|id| self.exposes_semantic_node(*id))
            .filter_map(|id| {
                let node = self.get(id)?;
                let snapshot = node.component_snapshot(id);
                let mut accessibility = snapshot.accessibility();
                let selection = snapshot.selection();
                if !accessibility.state.password {
                    if let Some(input) = node
                        .component()
                        .as_any()
                        .downcast_ref::<crate::ui::widgets::Input>()
                    {
                        accessibility.state.value_text = (!input.current_value().is_empty())
                            .then(|| input.current_value().to_owned());
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
                    accessibility,
                    selection,
                    actions,
                })
            })
            .collect();
        SemanticSnapshotBody { nodes }
    }
}
