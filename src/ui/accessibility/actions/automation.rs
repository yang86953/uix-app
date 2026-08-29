//! Opt-in automation support used by headless tests and external app control.

use std::fmt;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::core::{Point, Rect, WidgetId, WindowId};
use crate::platform::windowing::{KeyCode, KeyMod, MouseButton};
pub use crate::ui::SelectionSnapshot as AutomationSelection;
pub use crate::ui::accessibility::semantic_snapshot::{
    SemanticNode as AutomationNode, SemanticTarget as AutomationTarget,
};
use crate::ui::adapter::ViewAdapter;
use crate::ui::event::SystemEvent;
use crate::ui::semantic_action::SemanticActionError;
pub use crate::ui::semantic_action::{
    SemanticAction as AutomationAction, SemanticActionKind as AutomationActionKind,
};
use crate::ui::view::ViewNode;
use crate::ui::widget_runtime::widget::{EventResult, WidgetCore, WidgetTree};
use crate::ui::widget_snapshot::{AccessibilityRole, AccessibilityState};

pub const AUTOMATION_DIR_ENV: &str = "UIX_AUTOMATION_DIR";
pub const AUTOMATION_SCHEMA: &str = "uix.automation.v1";
const MAX_SETTLE_PASSES: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub struct AutomationSnapshot {
    pub schema: &'static str,
    pub process_id: u32,
    pub window_id: WindowId,
    pub generation: u64,
    pub revision: u64,
    pub presented_revision: u64,
    pub closed: bool,
    pub nodes: Vec<AutomationNode>,
}

impl AutomationSnapshot {
    pub fn find_all(&self, automation_id: &str) -> Vec<&AutomationNode> {
        self.nodes
            .iter()
            .filter(|node| node.automation_id.as_deref() == Some(automation_id))
            .collect()
    }

    pub fn find(&self, automation_id: &str) -> Result<&AutomationNode, AutomationError> {
        self.resolve(&AutomationTarget::AutomationId(automation_id.to_owned()))
    }

    pub fn resolve(&self, target: &AutomationTarget) -> Result<&AutomationNode, AutomationError> {
        match target {
            AutomationTarget::NodeId(id) => self
                .nodes
                .iter()
                .find(|node| node.id == *id)
                .ok_or_else(|| AutomationError::NotFound(id.to_string())),
            AutomationTarget::AutomationId(automation_id) => {
                let matches = self.find_all(automation_id);
                match matches.as_slice() {
                    [] => Err(AutomationError::NotFound(automation_id.clone())),
                    [node] => Ok(node),
                    _ => Err(AutomationError::Ambiguous {
                        automation_id: automation_id.clone(),
                        count: matches.len(),
                    }),
                }
            }
        }
    }

    pub fn to_json(&self) -> String {
        let mut out = String::new();
        let _ = write!(
            out,
            "{{\n  \"schema\": {},\n  \"process_id\": {},\n  \"window_id\": {},\n  \"generation\": {},\n  \"revision\": {},\n  \"presented_revision\": {},\n  \"closed\": {},\n  \"nodes\": [",
            json_string(self.schema),
            self.process_id,
            self.window_id.raw(),
            self.generation,
            self.revision,
            self.presented_revision,
            self.closed
        );

        if self.nodes.is_empty() {
            out.push_str("]\n}\n");
            return out;
        }

        out.push('\n');
        for (index, node) in self.nodes.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            write_node_json(&mut out, node);
        }
        out.push('\n');
        out.push_str("  ]\n}\n");
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationErrorCode {
    NodeNotFound,
    AmbiguousTarget,
    UnsupportedAction,
    InvalidValue,
    NotInteractable,
    Blocked,
    DidNotSettle,
    Internal,
}

impl AutomationErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NodeNotFound => "node_not_found",
            Self::AmbiguousTarget => "ambiguous_target",
            Self::UnsupportedAction => "unsupported_action",
            Self::InvalidValue => "invalid_value",
            Self::NotInteractable => "not_interactable",
            Self::Blocked => "blocked",
            Self::DidNotSettle => "did_not_settle",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationError {
    NotFound(String),
    Ambiguous {
        automation_id: String,
        count: usize,
    },
    NotVisible(String),
    Disabled(String),
    Obscured {
        automation_id: String,
        hit: Option<WidgetId>,
    },
    Blocked {
        automation_id: String,
        blocker: WidgetId,
    },
    UnsupportedAction {
        automation_id: String,
        action: AutomationActionKind,
    },
    InvalidValue {
        automation_id: String,
        action: AutomationActionKind,
    },
    SelectionDisabled {
        automation_id: String,
        index: usize,
    },
    NotHandled(String),
    DidNotSettle {
        passes: usize,
    },
}

impl AutomationError {
    pub const fn code(&self) -> AutomationErrorCode {
        match self {
            Self::NotFound(_) => AutomationErrorCode::NodeNotFound,
            Self::Ambiguous { .. } => AutomationErrorCode::AmbiguousTarget,
            Self::NotVisible(_) | Self::Disabled(_) => AutomationErrorCode::NotInteractable,
            Self::Obscured { .. } | Self::Blocked { .. } => AutomationErrorCode::Blocked,
            Self::UnsupportedAction { .. } => AutomationErrorCode::UnsupportedAction,
            Self::InvalidValue { .. } => AutomationErrorCode::InvalidValue,
            Self::SelectionDisabled { .. } => AutomationErrorCode::NotInteractable,
            Self::NotHandled(_) => AutomationErrorCode::Internal,
            Self::DidNotSettle { .. } => AutomationErrorCode::DidNotSettle,
        }
    }
}

impl fmt::Display for AutomationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "automation node `{id}` was not found"),
            Self::Ambiguous {
                automation_id,
                count,
            } => write!(
                f,
                "automation id `{automation_id}` matched {count} nodes; ids must be unique per window"
            ),
            Self::NotVisible(id) => write!(f, "automation node `{id}` is not visible"),
            Self::Disabled(id) => write!(f, "automation node `{id}` is disabled"),
            Self::Obscured { automation_id, hit } => write!(
                f,
                "automation node `{automation_id}` is not hittable at its visible center (hit {hit:?})"
            ),
            Self::Blocked {
                automation_id,
                blocker,
            } => write!(
                f,
                "automation node `{automation_id}` is blocked by overlay owner {blocker}"
            ),
            Self::UnsupportedAction {
                automation_id,
                action,
            } => write!(
                f,
                "automation node `{automation_id}` does not support `{}`",
                action.as_str()
            ),
            Self::InvalidValue {
                automation_id,
                action,
            } => write!(
                f,
                "automation node `{automation_id}` received an invalid value for `{}`",
                action.as_str()
            ),
            Self::SelectionDisabled {
                automation_id,
                index,
            } => write!(
                f,
                "automation node `{automation_id}` option {index} is disabled"
            ),
            Self::NotHandled(id) => {
                write!(f, "automation action for `{id}` was not handled")
            }
            Self::DidNotSettle { passes } => {
                write!(f, "automation app did not settle after {passes} passes")
            }
        }
    }
}

impl std::error::Error for AutomationError {}

/// In-process driver for deterministic application tests. It uses the same
/// hit testing and `SystemEvent` path as a native window, but no OS window or
/// graphics device is required.
pub struct TestApp {
    build_root: Box<dyn Fn() -> ViewNode>,
    tree: WidgetTree,
    viewport: (f32, f32),
}

impl TestApp {
    pub fn new<F>(size: (f32, f32), build_root: F) -> Self
    where
        F: Fn() -> ViewNode + 'static,
    {
        let viewport = (size.0.max(1.0), size.1.max(1.0));
        let build_root: Box<dyn Fn() -> ViewNode> = Box::new(build_root);
        let root = capture_root(build_root.as_ref());
        let mut tree = ViewAdapter::build_nodes(root);
        tree.set_app_state(crate::ui::AppState::new());
        set_root_frame(&mut tree, viewport);
        tree.layout();

        Self {
            build_root,
            tree,
            viewport,
        }
    }

    pub fn snapshot(&self) -> AutomationSnapshot {
        self.tree.automation_snapshot(WindowId::ROOT)
    }

    /// 返回 selector 对应节点当前对外可见的语义文本。
    ///
    /// 可编辑或可选择控件优先返回脱敏后的 `value_text`，其他节点返回
    /// accessible name；节点没有文本时返回空字符串。
    pub fn text(&self, automation_id: &str) -> Result<String, AutomationError> {
        let snapshot = self.snapshot();
        let node = snapshot.find(automation_id)?;
        Ok(node
            .accessibility
            .state
            .value_text
            .as_deref()
            .or(node.accessibility.name.as_deref())
            .unwrap_or_default()
            .to_owned())
    }

    pub fn perform(
        &mut self,
        target: impl Into<AutomationTarget>,
        action: AutomationAction,
    ) -> Result<(), AutomationError> {
        self.settle()?;
        let target = target.into();
        let label = target.label();
        let node = self.snapshot().resolve(&target)?.clone();
        self.tree
            .perform_semantic_action(node.id, &action)
            .map_err(|error| map_semantic_action_error(label, error))?;
        self.settle()?;
        Ok(())
    }

    pub fn invoke(&mut self, automation_id: &str) -> Result<(), AutomationError> {
        self.perform(automation_id, AutomationAction::Invoke)
    }

    pub fn focus(&mut self, automation_id: &str) -> Result<(), AutomationError> {
        self.perform(automation_id, AutomationAction::Focus)
    }

    pub fn set_value(
        &mut self,
        automation_id: &str,
        value: impl Into<String>,
    ) -> Result<(), AutomationError> {
        self.perform(automation_id, AutomationAction::SetValue(value.into()))
    }

    pub fn insert_text(
        &mut self,
        automation_id: &str,
        text: impl Into<String>,
    ) -> Result<(), AutomationError> {
        self.perform(automation_id, AutomationAction::InsertText(text.into()))
    }

    pub fn select(&mut self, automation_id: &str, index: usize) -> Result<(), AutomationError> {
        self.perform(automation_id, AutomationAction::Select(index.to_string()))
    }

    pub fn toggle(&mut self, automation_id: &str) -> Result<(), AutomationError> {
        self.perform(automation_id, AutomationAction::Toggle)
    }

    pub fn increment(&mut self, automation_id: &str) -> Result<(), AutomationError> {
        self.perform(automation_id, AutomationAction::Increment)
    }

    pub fn decrement(&mut self, automation_id: &str) -> Result<(), AutomationError> {
        self.perform(automation_id, AutomationAction::Decrement)
    }

    pub fn scroll(&mut self, automation_id: &str, delta: Point) -> Result<(), AutomationError> {
        // TestApp accepts the native wheel convention: negative vertical values
        // move the viewport down. The widget event layer normalizes wheel input
        // to positive-down deltas before dispatching it to ScrollView.
        self.perform(
            automation_id,
            AutomationAction::Scroll {
                delta: Point::new(-delta.x, -delta.y),
            },
        )
    }

    pub fn click(&mut self, automation_id: &str) -> Result<(), AutomationError> {
        self.settle()?;
        let node = self.snapshot().find(automation_id)?.clone();
        if !node.is_enabled() {
            return Err(AutomationError::Disabled(automation_id.to_string()));
        }
        if let Some(blocker) = self.tree.blocking_modal_for(node.id) {
            return Err(AutomationError::Blocked {
                automation_id: automation_id.to_string(),
                blocker,
            });
        }
        let Some(pos) = node.center() else {
            return Err(AutomationError::NotVisible(automation_id.to_string()));
        };
        if let Some(blocker) = self.tree.pointer_down_blocker_at(pos) {
            return Err(AutomationError::Blocked {
                automation_id: automation_id.to_string(),
                blocker,
            });
        }
        let hit = self.tree.pointer_target_at(pos);
        let hits_target = hit
            .is_some_and(|hit_id| hit_id == node.id || self.tree.is_descendant_of(hit_id, node.id));
        if !hits_target {
            return Err(AutomationError::Obscured {
                automation_id: automation_id.to_string(),
                hit,
            });
        }

        let down = self.tree.dispatch_event(&SystemEvent::PointerDown {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        });
        let up = self.tree.dispatch_event(&SystemEvent::PointerUp {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        });
        if down == EventResult::NotHandled && up == EventResult::NotHandled {
            return Err(AutomationError::NotHandled(automation_id.to_string()));
        }
        self.settle()?;
        Ok(())
    }

    pub fn type_text(
        &mut self,
        automation_id: &str,
        text: impl Into<String>,
    ) -> Result<(), AutomationError> {
        self.insert_text(automation_id, text)
    }

    pub fn press_key(&mut self, key: KeyCode, mods: KeyMod) -> Result<(), AutomationError> {
        self.settle()?;
        // KeyDown/KeyUp 返回 EventResult 而非 Result，无法直接 `?` 传播；
        // 与同文件 click 的严格 NotHandled 检查不同，按键可合法无接收方
        // （如无焦点组件），这里仅记录日志，保持返回值语义不变。
        if self
            .tree
            .dispatch_event(&SystemEvent::KeyDown { key, mods })
            == EventResult::NotHandled
        {
            tracing::warn!(key = ?key, "automation press_key KeyDown was not handled");
        }
        if self.tree.dispatch_event(&SystemEvent::KeyUp { key, mods }) == EventResult::NotHandled {
            tracing::warn!(key = ?key, "automation press_key KeyUp was not handled");
        }
        self.settle()?;
        Ok(())
    }

    pub fn resize(&mut self, width: f32, height: f32) -> Result<(), AutomationError> {
        self.settle()?;
        self.viewport = (width.max(1.0), height.max(1.0));
        // Resize 同 KeyDown/KeyUp：EventResult 无法 `?`，未处理仅记录日志，行为不变。
        if self.tree.dispatch_event(&SystemEvent::Resize {
            width: self.viewport.0,
            height: self.viewport.1,
        }) == EventResult::NotHandled
        {
            tracing::warn!(
                width = self.viewport.0,
                height = self.viewport.1,
                "automation resize was not handled"
            );
        }
        set_root_frame(&mut self.tree, self.viewport);
        self.tree.layout();
        self.settle()?;
        Ok(())
    }

    pub fn settle(&mut self) -> Result<usize, AutomationError> {
        let mut passes = 0;
        loop {
            // 停止树不得继续执行自动化协调或消费其待处理工作。
            if !self.tree.accepts_external_work() {
                // 复用既有内部未处理错误表达 fail-stop 拒绝。
                return Err(AutomationError::NotHandled("widget_tree".to_owned()));
            }
            let focus_changed = self.tree.drain_app_state_focus_requests();
            let semantic_changed = self.tree.drain_app_state_semantic_events();
            let effects_changed = self.tree.has_pending_effects() && self.tree.tick_effects();
            let reconcile_requested = self.tree.take_reconcile_requested();
            // 动态固有尺寸依赖只投递 Layout；测试驱动必须像真实窗口帧一样消费。
            let layout_requested = self
                .tree
                .invalidation
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .has_layout();
            if !focus_changed
                && !semantic_changed
                && !effects_changed
                && !reconcile_requested
                && !layout_requested
            {
                return Ok(passes);
            }
            if reconcile_requested {
                // Effect 或队列处理后仍须在捕获声明树前复核 fail-stop 状态。
                if !self.tree.accepts_external_work() {
                    // 构建根节点会调用用户闭包，停止树只能返回既有未处理错误。
                    return Err(AutomationError::NotHandled("widget_tree".to_owned()));
                }
                // 复用测试窗口树拥有的状态存储以模拟真实窗口协调。
                let root =
                    ViewAdapter::capture_root_with_store(self.tree.widget_state_store(), || {
                        (self.build_root)()
                    });
                ViewAdapter::reconcile_nodes(&mut self.tree, root);
            }
            set_root_frame(&mut self.tree, self.viewport);
            self.tree.layout();
            passes += 1;
            if passes >= MAX_SETTLE_PASSES {
                return Err(AutomationError::DidNotSettle { passes });
            }
        }
    }
}

fn map_semantic_action_error(automation_id: String, error: SemanticActionError) -> AutomationError {
    match error {
        SemanticActionError::NodeNotFound(_) => AutomationError::NotFound(automation_id),
        SemanticActionError::UnsupportedAction { action, .. } => {
            AutomationError::UnsupportedAction {
                automation_id,
                action,
            }
        }
        SemanticActionError::NotVisible(_) => AutomationError::NotVisible(automation_id),
        SemanticActionError::Disabled(_) => AutomationError::Disabled(automation_id),
        SemanticActionError::SelectionDisabled { index, .. } => {
            AutomationError::SelectionDisabled {
                automation_id,
                index,
            }
        }
        SemanticActionError::InvalidValue { action, .. } => AutomationError::InvalidValue {
            automation_id,
            action,
        },
        SemanticActionError::Blocked { blocker, .. } => AutomationError::Blocked {
            automation_id,
            blocker,
        },
        SemanticActionError::NotHandled { .. } => AutomationError::NotHandled(automation_id),
    }
}

fn capture_root(build_root: &dyn Fn() -> ViewNode) -> ViewNode {
    ViewAdapter::capture_root(build_root)
}

fn set_root_frame(tree: &mut WidgetTree, viewport: (f32, f32)) {
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, viewport.0, viewport.1));
    }
}

impl WidgetTree {
    pub fn automation_snapshot(&self, window_id: WindowId) -> AutomationSnapshot {
        AutomationSnapshot {
            schema: AUTOMATION_SCHEMA,
            process_id: std::process::id(),
            window_id,
            generation: 1,
            revision: 0,
            presented_revision: 0,
            closed: false,
            nodes: self.semantic_snapshot_body().nodes,
        }
    }

    pub(crate) fn configure_automation_from_env(&mut self, window_id: WindowId) {
        let Some(directory) =
            std::env::var_os(AUTOMATION_DIR_ENV).filter(|value| !value.is_empty())
        else {
            return;
        };
        self.automation_recorder = Some(AutomationRecorder::new(window_id, directory.into()));
    }

    pub(crate) fn automation_snapshot_configured(&self) -> bool {
        self.automation_recorder.is_some()
    }

    pub(crate) fn publish_automation_snapshot(
        &mut self,
        generation: u64,
        revision: u64,
        presented_revision: u64,
        nodes: &[AutomationNode],
    ) {
        let Some(recorder) = self.automation_recorder.as_mut() else {
            return;
        };
        let snapshot = AutomationSnapshot {
            schema: AUTOMATION_SCHEMA,
            process_id: std::process::id(),
            window_id: recorder.window_id,
            generation,
            revision,
            presented_revision,
            closed: false,
            nodes: nodes.to_vec(),
        };
        let result = recorder.publish(snapshot);
        if let Err(error) = result {
            tracing::error!("automation snapshot export disabled after write failure: {error}");
            self.automation_recorder = None;
        }
    }

    pub(crate) fn close_automation_snapshot(
        &mut self,
        generation: u64,
        revision: u64,
        presented_revision: u64,
    ) {
        let Some(recorder) = self.automation_recorder.as_mut() else {
            return;
        };
        if let Err(error) = recorder.close(generation, revision, presented_revision) {
            tracing::error!("automation snapshot close marker failed: {error}");
        }
        self.automation_recorder = None;
    }
}

pub(crate) struct AutomationRecorder {
    window_id: WindowId,
    path: PathBuf,
    last_body: Option<String>,
}

impl AutomationRecorder {
    pub(crate) fn new(window_id: WindowId, directory: PathBuf) -> Self {
        let path = directory.join(format!(
            "uix-{}-window-{}.json",
            std::process::id(),
            window_id.raw()
        ));
        Self {
            window_id,
            path,
            last_body: None,
        }
    }

    pub(crate) fn publish(&mut self, snapshot: AutomationSnapshot) -> std::io::Result<()> {
        let body = snapshot.to_json();
        if self.last_body.as_deref() == Some(body.as_str()) {
            return Ok(());
        }
        write_snapshot(&self.path, &body)?;
        self.last_body = Some(body);
        Ok(())
    }

    pub(crate) fn close(
        &mut self,
        generation: u64,
        revision: u64,
        presented_revision: u64,
    ) -> std::io::Result<()> {
        let snapshot = AutomationSnapshot {
            schema: AUTOMATION_SCHEMA,
            process_id: std::process::id(),
            window_id: self.window_id,
            generation,
            revision,
            presented_revision,
            closed: true,
            nodes: Vec::new(),
        };
        self.publish(snapshot)
    }
}

fn write_snapshot(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let extension = format!("json.tmp-{}", std::process::id());
    let temporary = path.with_extension(extension);
    std::fs::write(&temporary, contents)?;
    match std::fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::PermissionDenied
            ) =>
        {
            std::fs::remove_file(path)?;
            std::fs::rename(temporary, path)
        }
        Err(error) => Err(error),
    }
}

fn write_node_json(out: &mut String, node: &AutomationNode) {
    let role = node.accessibility.role.automation_name();
    let state = &node.accessibility.state;
    let _ = write!(
        out,
        "    {{\"id\": {}, \"automation_id\": {}, \"parent\": {}, \"frame\": {}, \"visible_bounds\": {}, \"focused\": {}, \"role\": {}, \"name\": {}, \"state\": {}, \"selection\": {}, \"actions\": {} }}",
        json_string(&node.id.to_string()),
        json_optional_string(node.automation_id.as_deref()),
        node.parent
            .map(|id| json_string(&id.to_string()))
            .unwrap_or_else(|| "null".to_string()),
        rect_json(node.frame),
        node.visible_bounds
            .map(rect_json)
            .unwrap_or_else(|| "null".to_string()),
        node.focused,
        json_string(role),
        json_optional_string(node.accessibility.name.as_deref()),
        accessibility_state_json(state),
        selection_json(node.selection.as_ref()),
        action_kinds_json(&node.actions)
    );
}

fn selection_json(selection: Option<&AutomationSelection>) -> String {
    let Some(selection) = selection else {
        return "null".to_owned();
    };
    let options = selection
        .options
        .iter()
        .map(|option| json_string(option))
        .collect::<Vec<_>>()
        .join(", ");
    let selected_indices = selection
        .selected_indices
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let disabled_indices = selection
        .disabled_indices
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{{\"options\": [{options}], \"selected_indices\": [{selected_indices}], \"disabled_indices\": [{disabled_indices}], \"multiple\": {}, \"expanded\": {}}}",
        selection.multiple, selection.expanded
    )
}

fn action_kinds_json(actions: &[AutomationActionKind]) -> String {
    let values = actions
        .iter()
        .map(|action| json_string(action.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn rect_json(rect: Rect) -> String {
    format!(
        "{{\"x\": {}, \"y\": {}, \"w\": {}, \"h\": {}}}",
        json_number(rect.x as f64),
        json_number(rect.y as f64),
        json_number(rect.w as f64),
        json_number(rect.h as f64)
    )
}

fn accessibility_state_json(state: &AccessibilityState) -> String {
    format!(
        "{{\"disabled\": {}, \"checked\": {}, \"expanded\": {}, \"selected\": {}, \"value_text\": {}, \"value_now\": {}, \"value_min\": {}, \"value_max\": {}, \"multiline\": {}, \"password\": {}, \"required\": {}}}",
        state.disabled,
        json_optional_bool(state.checked),
        json_optional_bool(state.expanded),
        json_optional_bool(state.selected),
        json_optional_string(state.value_text.as_deref()),
        json_optional_number(state.value_now),
        json_optional_number(state.value_min),
        json_optional_number(state.value_max),
        state.multiline,
        state.password,
        state.required
    )
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch <= '\u{1f}' => {
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn json_optional_string(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_string())
}

fn json_optional_bool(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn json_optional_number(value: Option<f64>) -> String {
    value.map(json_number).unwrap_or_else(|| "null".to_string())
}

fn json_number(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "null".to_string()
    }
}
