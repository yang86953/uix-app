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
    let interactive = label("target")
        .focusable(true)
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
