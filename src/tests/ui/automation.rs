use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::WindowId;
use crate::prelude::*;
use crate::ui::automation::AutomationRecorder;
use crate::ui::test_harness::{
    AutomationAction, AutomationActionKind, AutomationError, AutomationErrorCode, AutomationTarget,
    TestApp, AUTOMATION_SCHEMA,
};
use crate::ui::{EventHandler, OverlayKind, WidgetCapabilities, WidgetLayout};

static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct ResizeSpy {
    observed: Rc<Cell<Option<(f32, f32)>>>,
}

impl ResizeSpy {
    fn new(observed: Rc<Cell<Option<(f32, f32)>>>) -> Self {
        Self { observed }
    }
}

impl WidgetComponent for ResizeSpy {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT | WidgetCapabilities::EVENT)
    }

    crate::wc_upcast!(ResizeSpy; WidgetLayout);
    crate::wc_upcast!(ResizeSpy; EventHandler);
}

impl WidgetLayout for ResizeSpy {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(1.0, 1.0))
    }
}

impl EventHandler for ResizeSpy {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::Resize { width, height } = *event {
            self.observed.set(Some((width, height)));
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }
}

#[test]
fn snapshot_exposes_stable_selector_semantics_and_bounds() {
    let app = TestApp::new((320.0, 120.0), || {
        column([button("Increment").automation_id("counter.increment")])
    });

    let snapshot = app.snapshot();
    let node = snapshot.find("counter.increment").unwrap();
    assert_eq!(node.accessibility.role, AccessibilityRole::Button);
    assert_eq!(node.accessibility.name.as_deref(), Some("Increment"));
    assert!(node.is_visible());
    assert!(node.is_enabled());
    assert!(node.center().is_some());
    assert_eq!(
        node.actions,
        vec![AutomationActionKind::Invoke, AutomationActionKind::Focus]
    );
    assert_eq!(snapshot.schema, AUTOMATION_SCHEMA);
}

#[test]
fn click_uses_hit_testing_and_settles_reconcile() {
    let count = State::new(0i32);
    let count_for_root = count.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        let count = count_for_root.clone();
        column([
            count
                .clone()
                .map_text(|value| format!("Count: {value}"))
                .automation_id("counter.value"),
            button("Increment")
                .on_click(&count, |count| count.update(|value| *value += 1))
                .automation_id("counter.increment"),
        ])
    });
    let before_id = app.snapshot().find("counter.increment").unwrap().id;

    app.click("counter.increment").unwrap();

    assert_eq!(count.get(), 1);
    assert_eq!(app.settle().unwrap(), 0);
    assert_eq!(app.text("counter.value").unwrap(), "Count: 1");
    assert_eq!(
        app.snapshot().find("counter.increment").unwrap().id,
        before_id,
        "automation metadata must not change reconciliation identity"
    );
    assert_eq!(
        app.snapshot()
            .find("counter.value")
            .unwrap()
            .accessibility
            .name
            .as_deref(),
        Some("Count: 1")
    );
}

#[test]
fn click_reports_modal_blocking_without_dispatching_to_background_target() {
    let count = State::new(0i32);
    let count_for_root = count.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        let count = count_for_root.clone();
        column((
            button("Background")
                .on_click(&count, |count| count.update(|value| *value += 1))
                .automation_id("background"),
            button("Modal owner").automation_id("modal"),
        ))
    });
    let modal = app.snapshot().find("modal").unwrap().id;
    app.tree_mut()
        .overlay_stack_mut()
        .push(modal, OverlayKind::Modal);

    let error = app.click("background").unwrap_err();

    assert_eq!(
        error,
        AutomationError::Blocked {
            automation_id: "background".to_owned(),
            blocker: modal,
        }
    );
    assert_eq!(error.code(), AutomationErrorCode::Blocked);
    assert_eq!(count.get(), 0);
}

#[test]
fn duplicate_selector_is_reported_instead_of_picking_arbitrarily() {
    let app = TestApp::new((320.0, 160.0), || {
        column([
            button("First").automation_id("duplicate"),
            button("Second").automation_id("duplicate"),
        ])
    });

    assert_eq!(
        app.snapshot().find("duplicate").unwrap_err(),
        AutomationError::Ambiguous {
            automation_id: "duplicate".to_string(),
            count: 2,
        }
    );
    assert_eq!(
        app.text("duplicate").unwrap_err(),
        AutomationError::Ambiguous {
            automation_id: "duplicate".to_string(),
            count: 2,
        }
    );
}

#[test]
fn type_text_updates_accessible_value_and_redacts_passwords() {
    let mut plain = TestApp::new((320.0, 120.0), || {
        input().placeholder("Name").automation_id("profile.name")
    });
    plain.type_text("profile.name", "Belldandy").unwrap();
    let plain_snapshot = plain.snapshot();
    let plain_node = plain_snapshot.find("profile.name").unwrap();
    assert!(plain_node.focused);
    assert_eq!(
        plain_node.accessibility.state.value_text.as_deref(),
        Some("Belldandy")
    );
    assert_eq!(plain.text("profile.name").unwrap(), "Belldandy");

    let mut password = TestApp::new((320.0, 120.0), || {
        embed(Input::password().placeholder("Password")).automation_id("profile.password")
    });
    password
        .type_text("profile.password", "not-exported")
        .unwrap();
    let password_snapshot = password.snapshot();
    let password_node = password_snapshot.find("profile.password").unwrap();
    assert!(password_node.accessibility.state.password);
    assert_eq!(password_node.accessibility.state.value_text, None);
    assert!(!password_snapshot.to_json().contains("not-exported"));
    assert!(!password
        .text("profile.password")
        .unwrap()
        .contains("not-exported"));
}

#[test]
fn shared_semantic_actions_resolve_both_selector_and_node_id() {
    let count = State::new(0i32);
    let count_for_root = count.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        let count = count_for_root.clone();
        button("Increment")
            .on_click(&count, |count| count.update(|value| *value += 1))
            .automation_id("counter.increment")
    });
    let node_id = app.snapshot().find("counter.increment").unwrap().id;

    app.invoke("counter.increment").unwrap();
    app.perform(AutomationTarget::NodeId(node_id), AutomationAction::Invoke)
        .unwrap();

    assert_eq!(count.get(), 2);
    assert_eq!(
        app.snapshot().find("counter.increment").unwrap().id,
        node_id
    );
}

#[test]
fn press_key_settles_pending_reconcile_before_dispatch() {
    let disabled = State::new(false);
    let clicks = State::new(0i32);
    let disabled_for_root = disabled.clone();
    let clicks_for_root = clicks.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        let clicks = clicks_for_root.clone();
        embed(Button::new("Target").disabled(disabled_for_root.get()))
            .on_click(&clicks, |clicks| clicks.update(|count| *count += 1))
            .automation_id("target")
    });
    app.focus("target").unwrap();
    assert!(app.snapshot().find("target").unwrap().focused);

    disabled.set(true);
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();

    let target = app.snapshot().find("target").unwrap().clone();
    assert!(!target.is_enabled());
    assert!(!target.focused);
    assert_eq!(clicks.get(), 0);
}

#[test]
fn resize_settles_pending_reconcile_before_dispatch() {
    let show_spy = State::new(false);
    let show_spy_for_root = show_spy.clone();
    let observed = Rc::new(Cell::new(None));
    let observed_for_root = observed.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        if show_spy_for_root.get() {
            embed(ResizeSpy::new(observed_for_root.clone()))
        } else {
            label("waiting")
        }
    });

    show_spy.set(true);
    app.resize(640.0, 480.0).unwrap();

    assert_eq!(observed.get(), Some((640.0, 480.0)));
}

#[test]
fn semantic_set_value_insert_text_and_errors_are_protocol_typed() {
    let mut input_app = TestApp::new((320.0, 120.0), || {
        input().placeholder("Name").automation_id("profile.name")
    });
    input_app.set_value("profile.name", "Ada").unwrap();
    input_app.insert_text("profile.name", " Lovelace").unwrap();
    assert_eq!(
        input_app
            .snapshot()
            .find("profile.name")
            .unwrap()
            .accessibility
            .state
            .value_text
            .as_deref(),
        Some("Ada Lovelace")
    );

    let error = input_app
        .perform("profile.name", AutomationAction::Toggle)
        .unwrap_err();
    assert_eq!(error.code(), AutomationErrorCode::UnsupportedAction);
    assert_eq!(error.code().as_str(), "unsupported_action");

    let mut disabled_app = TestApp::new((240.0, 80.0), || {
        embed(Button::new("Disabled").disabled(true)).automation_id("disabled")
    });
    let error = disabled_app.invoke("disabled").unwrap_err();
    assert_eq!(error.code(), AutomationErrorCode::NotInteractable);
    assert_eq!(error.code().as_str(), "not_interactable");

    let debug = format!(
        "{:?}",
        AutomationAction::SetValue("must-not-appear".to_owned())
    );
    assert!(!debug.contains("must-not-appear"));
}

#[test]
fn selection_snapshot_and_convenience_action_use_zero_based_indices() {
    let mut app = TestApp::new((320.0, 80.0), || {
        embed(Segmented::new(["Day", "Week", "Month"]).disable_option(1)).automation_id("period")
    });

    let before = app.snapshot();
    let node = before.find("period").unwrap();
    let selection = node.selection.as_ref().unwrap();
    assert_eq!(selection.options, vec!["Day", "Week", "Month"]);
    assert_eq!(selection.selected_indices, vec![0]);
    assert_eq!(selection.disabled_indices, vec![1]);
    assert!(!selection.multiple);
    assert!(!selection.expanded);
    assert!(node.supports(AutomationActionKind::Select));

    app.select("period", 2).unwrap();
    let after = app.snapshot();
    let node = after.find("period").unwrap();
    assert_eq!(node.selection.as_ref().unwrap().selected_indices, vec![2]);
    assert_eq!(
        node.accessibility.state.value_text.as_deref(),
        Some("Month")
    );
    let json = after.to_json();
    assert!(json.contains("\"selected_indices\": [2]"));
    assert!(json.contains("\"disabled_indices\": [1]"));

    let error = app.select("period", 1).unwrap_err();
    assert_eq!(error.code(), AutomationErrorCode::NotInteractable);
    assert_eq!(
        error,
        AutomationError::SelectionDisabled {
            automation_id: "period".to_owned(),
            index: 1,
        }
    );
    let error = app
        .perform("period", AutomationAction::Select("invalid".to_owned()))
        .unwrap_err();
    assert_eq!(error.code(), AutomationErrorCode::InvalidValue);
    assert_eq!(error.code().as_str(), "invalid_value");
}

#[test]
fn snapshot_json_escapes_selector_and_has_machine_readable_schema() {
    let app = TestApp::new((240.0, 80.0), || {
        button("Quoted")
            .automation_id("quote\"line\nnext")
            .padding(8.0)
    });

    let json = app.snapshot().to_json();
    assert!(json.contains("\"schema\": \"uix.automation.v1\""));
    assert!(json.contains("\"generation\": 1"));
    assert!(json.contains("\"presented_revision\": 0"));
    assert!(json.contains("quote\\\"line\\nnext"));
    assert!(json.contains("\"actions\": [\"invoke\", \"focus\"]"));
    assert!(json.ends_with("}\n"));
}

#[test]
fn recorder_writes_changed_snapshots_once_and_marks_window_closed() {
    let sequence = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "uix-automation-test-{}-{sequence}",
        std::process::id()
    ));
    let path = directory.join(format!(
        "uix-{}-window-{}.json",
        std::process::id(),
        WindowId::ROOT.raw()
    ));
    let app = TestApp::new((240.0, 80.0), || {
        button("Exported").automation_id("exported.button")
    });
    let mut snapshot = app.snapshot();
    snapshot.revision = 1;
    let mut recorder = AutomationRecorder::new(WindowId::ROOT, directory.clone());

    recorder.publish(snapshot.clone()).unwrap();
    let first = std::fs::read_to_string(&path).unwrap();
    assert!(first.contains("\"revision\": 1"));
    assert!(first.contains("exported.button"));

    recorder.publish(snapshot).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), first);

    recorder.close(1, 2, 1).unwrap();
    let closed = std::fs::read_to_string(&path).unwrap();
    assert!(closed.contains("\"revision\": 2"));
    assert!(closed.contains("\"presented_revision\": 1"));
    assert!(closed.contains("\"closed\": true"));
    assert!(closed.contains("\"nodes\": []"));

    std::fs::remove_dir_all(directory).unwrap();
}
