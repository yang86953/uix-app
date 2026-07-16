use crate::prelude::*;

component! {
    struct UsagePaintProbe {
        radius: f32,
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext) {
        let color = ctx.tokens().color_primary();
        let center = Point::new(frame.x + frame.w * 0.5, frame.y + frame.h * 0.5);
        ctx.fill_rect(frame, color.with_alpha(24), Some(Radius::uniform(8.0)));
        ctx.stroke_arc(
            center.x,
            center.y,
            self.radius,
            0.0,
            std::f32::consts::PI,
            color,
            2.0,
        );
        ctx.draw_text("usage", Point::new(frame.x, frame.y), color, 14.0);
    }
}

component! {
    struct UsageEventProbe {
        #[snapshot(skip)]
        last_pointer: Option<Point>,
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext) {}

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                self.last_pointer = Some(*pos);
                EventResult::Handled
            }
            SystemEvent::KeyDown {
                key: KeyCode::Enter,
                ..
            }
            | SystemEvent::FocusIn => EventResult::Handled,
            _ => EventResult::NotHandled,
        }
    }
}

#[test]
fn documented_config_and_chart_builders_compile_from_prelude() {
    let _config = Config::new()
        .component_size(ControlSize::Medium)
        .disabled(false);
    let _provider = ConfigProvider::new()
        .component_size(ControlSize::Small)
        .child(|| input().placeholder("name"));

    let _bar = BarChart::new()
        .data(vec![
            BarData::new("Q1", 120.0, Color::BLUE),
            BarData::new("Q2", 200.0, Color::GREEN),
        ])
        .width(400.0)
        .height(300.0);
    let _line = LineChart::new()
        .data(vec![
            LineData::new("Jan", 100.0),
            LineData::new("Feb", 150.0),
        ])
        .width(400.0)
        .height(300.0);
    let _pie = PieChart::new()
        .data(vec![
            PieData::new("A", 40.0, Color::BLUE),
            PieData::new("B", 60.0, Color::GREEN),
        ])
        .size(300.0);
}

#[test]
fn documented_semantic_event_builders_compile_from_prelude() {
    let count = State::new(0);
    let _button = button("+1").on_click(&count, |state| state.update(|value| *value += 1));
    let _semantic = label("submit").on_semantic(SemanticKind::Submit, |_| {});
    let _paint = UsagePaintProbe { radius: 12.0 };
    let _event = UsageEventProbe { last_pointer: None };
}

#[test]
fn documented_raw_event_and_accessibility_builders_compile_from_prelude() {
    let focus = FocusHandle::new();
    let interactive = label("target")
        .focusable(true)
        .focus_handle(&focus)
        .on_pointer(|event| match event {
            SystemEvent::PointerDown { .. } => EventResult::Handled,
            _ => EventResult::NotHandled,
        })
        .on_key(|_| EventResult::NotHandled)
        .on_focus(|_| EventResult::NotHandled)
        .on_scroll(|_| EventResult::NotHandled)
        .role(AccessibilityRole::Button)
        .accessible_name("Target")
        .accessibility_state(AccessibilityState {
            expanded: Some(false),
            selected: Some(true),
            ..AccessibilityState::default()
        })
        .aria("aria-description", "Public usage probe");
    let _capture = column((interactive,)).on_key_capture(|_| EventResult::NotHandled);
    let _hidden = label("decorative").role(AccessibilityRole::None);
    let _snapshot = AccessibilitySnapshot::named(AccessibilityRole::Group, "Example")
        .with_attribute(AriaAttribute::new("aria-live", "polite"));
    let _focus_result: Result<(), FocusHandleError> = focus.focus();
}

#[test]
fn documented_form_validation_returns_typed_values_from_prelude() {
    let form = Form::new()
        .field("user", "Username")
        .default("")
        .required("username required")
        .custom(|value| {
            (value.len() >= 3)
                .then_some(())
                .ok_or_else(|| "username too short".to_string())
        })
        .field("email", "Email")
        .default("")
        .required("email required")
        .validate_email("invalid email")
        .field("age", "Age")
        .default(0_i32)
        .validate_range(1..=120, "invalid age")
        .build();

    let errors = form.validate().expect_err("empty form must fail");
    assert_eq!(
        errors
            .iter()
            .map(|error| (error.field(), error.message()))
            .collect::<Vec<_>>(),
        vec![
            ("user", "username required"),
            ("email", "email required"),
            ("age", "invalid age"),
        ]
    );

    assert!(form.set_value("user", "Ada"));
    assert!(form.set_value("email", "ada@example.test"));
    assert!(form.set_value("age", 36_i32));
    let values = form.validate().expect("valid form");

    assert_eq!(
        values.get::<String>("user").map(String::as_str),
        Some("Ada")
    );
    assert_eq!(
        values.get::<String>("email").map(String::as_str),
        Some("ada@example.test")
    );
    assert_eq!(values.get::<i32>("age"), Some(&36));
}

#[cfg(feature = "test-harness")]
#[test]
fn documented_test_app_flow_compiles_and_runs_from_prelude() {
    let count = State::new(0);
    let root_count = count.clone();
    let mut app = TestApp::new((400.0, 300.0), move || {
        let count = root_count.clone();
        row((
            count
                .map_text(|value| format!("{value}"))
                .automation_id("counter.label"),
            button("+1")
                .on_click(&count, |state| state.update(|value| *value += 1))
                .automation_id("counter.inc"),
        ))
    });

    app.click("counter.inc").expect("counter click");
    assert_eq!(app.text("counter.label").as_deref(), Ok("1"));
}

#[cfg(feature = "test-harness")]
#[test]
fn documented_focus_handle_flow_compiles_and_runs_from_prelude() {
    let focus = FocusHandle::new();
    let build_focus = focus.clone();
    let mut app = TestApp::new((400.0, 300.0), move || {
        input().placeholder("email").focus_handle(&build_focus)
    });

    focus.focus().expect("bound focus handle");
    app.settle().expect("focus command should settle");
    focus.blur().expect("bound blur handle");
    app.settle().expect("blur command should settle");
}

#[cfg(feature = "test-harness")]
#[test]
fn documented_test_app_control_actions_run_from_prelude() {
    let name = State::new(String::new());
    let period = State::new("Day".to_string());
    let agreed = State::new(false);
    let level = State::new(10.0_f64);
    let build_name = name.clone();
    let build_period = period.clone();
    let build_agreed = agreed.clone();
    let build_level = level.clone();
    let mut app = TestApp::new((480.0, 320.0), move || {
        column((
            input()
                .placeholder("Name")
                .value(&build_name)
                .automation_id("profile.name"),
            embed(Segmented::new(["Day", "Week", "Month"]).value(&build_period))
                .automation_id("profile.period"),
            embed(Checkbox::new("Accept").checked(&build_agreed)).automation_id("profile.agreed"),
            embed(Slider::new(0.0..=20.0).step(5.0).value(&build_level))
                .automation_id("profile.level"),
        ))
    });

    app.type_text("profile.name", "Ada").expect("type text");
    app.select("profile.period", 2).expect("select period");
    app.toggle("profile.agreed").expect("toggle agreement");
    app.increment("profile.level").expect("increment level");
    app.decrement("profile.level").expect("decrement level");

    assert_eq!(name.get(), "Ada");
    assert_eq!(period.get(), "Month");
    assert!(agreed.get());
    assert_eq!(level.get(), 10.0);
    assert_eq!(app.text("profile.name").as_deref(), Ok("Ada"));
}

#[cfg(feature = "test-harness")]
#[test]
fn documented_test_app_scroll_moves_content_through_the_viewport() {
    let mut app = TestApp::new((240.0, 160.0), || {
        scroll(
            column((
                label("Top").height(80.0).automation_id("scroll.top"),
                label("Middle").height(80.0),
                label("Bottom").height(80.0).automation_id("scroll.bottom"),
            ))
            .overflow_content(),
        )
        .vertical()
        .size(120.0, 80.0)
        .show_scrollbar(false)
        .automation_id("scroll.viewport")
    });

    let before = app.snapshot();
    assert!(before
        .find("scroll.viewport")
        .expect("scroll viewport")
        .supports(crate::ui::test_harness::AutomationActionKind::Scroll));
    assert!(before.find("scroll.top").expect("top row").is_visible());
    assert!(!before
        .find("scroll.bottom")
        .expect("bottom row")
        .is_visible());

    app.scroll("scroll.viewport", Point::new(0.0, 8.0))
        .expect("scroll viewport");

    let after = app.snapshot();
    assert!(!after.find("scroll.top").expect("top row").is_visible());
    assert!(after
        .find("scroll.bottom")
        .expect("bottom row")
        .is_visible());
}

#[cfg(feature = "test-harness")]
#[test]
fn documented_test_app_keyboard_and_resize_update_the_snapshot() {
    let mut app = TestApp::new((240.0, 160.0), || {
        column((
            button("First").automation_id("focus.first"),
            button("Second").automation_id("focus.second"),
        ))
        .automation_id("layout.root")
    });

    let before = app.snapshot();
    assert_eq!(
        before.find("layout.root").expect("root layout").frame,
        Rect::new(0.0, 0.0, 240.0, 160.0)
    );

    app.press_key(KeyCode::Tab, KeyMod::NONE)
        .expect("focus first button");
    let first_focus = app.snapshot();
    assert!(
        first_focus
            .find("focus.first")
            .expect("first button")
            .focused
    );
    assert!(
        !first_focus
            .find("focus.second")
            .expect("second button")
            .focused
    );

    app.press_key(KeyCode::Tab, KeyMod::NONE)
        .expect("focus second button");
    let second_focus = app.snapshot();
    assert!(
        !second_focus
            .find("focus.first")
            .expect("first button")
            .focused
    );
    assert!(
        second_focus
            .find("focus.second")
            .expect("second button")
            .focused
    );

    app.resize(360.0, 220.0).expect("resize viewport");
    assert_eq!(
        app.snapshot()
            .find("layout.root")
            .expect("resized root layout")
            .frame,
        Rect::new(0.0, 0.0, 360.0, 220.0)
    );
}

#[cfg(feature = "test-harness")]
#[test]
fn documented_test_app_reports_typed_errors_and_redacts_passwords() {
    use crate::ui::test_harness::AutomationErrorCode;

    let mut app = TestApp::new((400.0, 240.0), || {
        column((
            input().placeholder("Name").automation_id("profile.name"),
            embed(Input::password().placeholder("Password")).automation_id("profile.password"),
            embed(Button::new("Disabled").disabled(true)).automation_id("disabled"),
            label("First duplicate").automation_id("duplicate"),
            label("Second duplicate").automation_id("duplicate"),
        ))
    });

    assert_eq!(
        app.text("missing").expect_err("missing selector").code(),
        AutomationErrorCode::NodeNotFound
    );
    assert_eq!(
        app.text("duplicate")
            .expect_err("duplicate selector")
            .code(),
        AutomationErrorCode::AmbiguousTarget
    );
    assert_eq!(
        app.toggle("profile.name")
            .expect_err("input does not toggle")
            .code(),
        AutomationErrorCode::UnsupportedAction
    );
    assert_eq!(
        app.invoke("disabled")
            .expect_err("disabled button is not interactable")
            .code(),
        AutomationErrorCode::NotInteractable
    );

    app.type_text("profile.password", "must-not-be-exported")
        .expect("type password");
    let snapshot = app.snapshot();
    let password = snapshot.find("profile.password").expect("password input");
    assert!(password.accessibility.state.password);
    assert_eq!(password.accessibility.state.value_text, None);
    assert!(!snapshot.to_json().contains("must-not-be-exported"));
    assert!(!app
        .text("profile.password")
        .expect("redacted password text")
        .contains("must-not-be-exported"));
}
