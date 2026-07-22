use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::navigation::steps::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn render_steps(steps: &Steps, frame: Rect) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(800, 320));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::painting::DisplayList::new();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            font,
            &fonts,
            &images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            800,
            320,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(steps, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn key_event(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    }
}

#[test]
fn steps_are_focusable_and_keyboard_navigation_follows_direction() {
    let mut steps = Steps::new(vec![Step::new("Start"), Step::new("Done")]);
    let event = key_event(KeyCode::Right);

    assert_eq!(WidgetComponent::tab_index(&steps), 1);
    assert_eq!(steps.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(steps.on_event(&event), EventResult::Handled);
    assert_eq!(steps.get_current(), 1);
    assert_eq!(
        steps
            .semantic_event(ComponentId::new(2), &event)
            .and_then(|event| { event.text_payload().map(str::to_owned) }),
        Some("1".to_string())
    );
    assert_eq!(
        steps.on_event(&key_event(KeyCode::Down)),
        EventResult::NotHandled
    );
}

#[test]
fn vertical_steps_render_contract_and_pointer_selection_are_reachable() {
    let mut steps = Steps::new(vec![
        Step::new("Start"),
        Step::new("Middle").description("Working"),
        Step::new("Done"),
    ])
    .vertical();

    assert_eq!(
        steps.measure(Constraints::loose(Size::new(500.0, 500.0))),
        Size::new(200.0, 240.0)
    );
    assert_eq!(
        steps.on_event(&SystemEvent::PointerDown {
            pos: Point::new(30.0, 100.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(steps.get_current(), 1);
    assert_eq!(
        steps.on_event(&key_event(KeyCode::Right)),
        EventResult::NotHandled
    );
    assert_eq!(
        steps.on_event(&key_event(KeyCode::Down)),
        EventResult::Handled
    );
    assert_eq!(steps.get_current(), 2);
}

#[test]
fn steps_snapshot_and_accessibility_expose_current_step() {
    let steps = Steps::new(vec![Step::new("Start"), Step::new("Review")]).current(1);
    let fields = steps.snapshot_fields();

    assert!(matches!(
        fields,
        SnapshotFields::Steps {
            current: 1,
            direction: StepsDirection::Horizontal,
            ..
        }
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.role, crate::ui::AccessibilityRole::Navigation);
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Review"));
    assert_eq!(accessibility.state.value_now, Some(2.0));
}

#[test]
fn empty_steps_are_not_focusable_and_current_is_clamped() {
    let empty = Steps::new(Vec::new()).current(99);
    assert_eq!(WidgetComponent::tab_index(&empty), 0);
    assert_eq!(empty.get_current(), 0);

    let steps = Steps::new(vec![Step::new("Only")]).current(99);
    assert_eq!(steps.get_current(), 0);
}

#[test]
fn every_step_status_has_distinct_paint_and_is_preserved_in_snapshot() {
    let statuses = [
        StepStatus::Wait,
        StepStatus::Process,
        StepStatus::Finish,
        StepStatus::Error,
    ];
    let rendered = statuses
        .iter()
        .map(|status| {
            render_steps(
                &Steps::new(vec![Step::new("Status").status(*status)]),
                Rect::new(0.0, 0.0, 240.0, 96.0),
            )
        })
        .collect::<Vec<_>>();

    for left in 0..rendered.len() {
        for right in left + 1..rendered.len() {
            assert_ne!(
                rendered[left], rendered[right],
                "status {:?} and {:?} must not paint identically",
                statuses[left], statuses[right]
            );
        }
    }

    let configured = Steps::new(
        statuses
            .iter()
            .enumerate()
            .map(|(index, status)| {
                let title = format!("Step {index}");
                Step::new(&title).status(*status)
            })
            .collect(),
    );
    let SnapshotFields::Steps { steps, .. } = configured.snapshot_fields() else {
        panic!("expected steps snapshot");
    };
    assert_eq!(
        steps.iter().map(|step| step.status).collect::<Vec<_>>(),
        statuses
    );
}

#[test]
fn clickable_steps_change_once_and_invoke_the_configured_callback() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let callback_observed = Rc::clone(&observed);
    let mut steps =
        Steps::new(vec![Step::new("First"), Step::new("Second")]).on_step(move |index, step| {
            callback_observed
                .borrow_mut()
                .push((index, step.title.clone()));
        });
    render_steps(&steps, Rect::new(40.0, 20.0, 600.0, 96.0));
    let second = SystemEvent::PointerDown {
        pos: Point::new(350.0, 28.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(steps.on_event(&second), EventResult::Handled);
    assert_eq!(steps.get_current(), 1);
    assert_eq!(observed.borrow().as_slice(), &[(1, "Second".to_string())]);
    assert_eq!(
        steps
            .semantic_event(ComponentId::new(9), &second)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("1".to_string())
    );

    assert_eq!(steps.on_event(&second), EventResult::Handled);
    assert_eq!(observed.borrow().len(), 1, "unchanged selection is silent");
    assert!(steps.semantic_event(ComponentId::new(9), &second).is_none());
}

#[test]
fn non_clickable_steps_ignore_pointer_selection_and_callback() {
    let calls = Rc::new(Cell::new(0));
    let callback_calls = Rc::clone(&calls);
    let mut steps = Steps::new(vec![Step::new("First"), Step::new("Second")])
        .clickable(false)
        .on_step(move |_, _| callback_calls.set(callback_calls.get() + 1));
    render_steps(&steps, Rect::new(0.0, 0.0, 400.0, 96.0));

    assert_eq!(
        steps.on_event(&SystemEvent::PointerDown {
            pos: Point::new(300.0, 28.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(steps.get_current(), 0);
    assert_eq!(calls.get(), 0);
}

#[test]
fn dot_and_custom_icon_change_real_step_geometry() {
    let numbered = render_steps(
        &Steps::new(vec![Step::new("Pay").status(StepStatus::Process)]),
        Rect::new(0.0, 0.0, 240.0, 96.0),
    );
    let dotted = render_steps(
        &Steps::new(vec![Step::new("Pay").status(StepStatus::Process)]).dot(true),
        Rect::new(0.0, 0.0, 240.0, 96.0),
    );
    let icon = render_steps(
        &Steps::new(vec![Step::new("Pay")
            .status(StepStatus::Process)
            .icon("credit-card")]),
        Rect::new(0.0, 0.0, 240.0, 96.0),
    );

    assert!(
        dotted.matches("FillCircle").count() > numbered.matches("FillCircle").count(),
        "dot mode must add a real inner circle: {dotted}"
    );
    assert_ne!(icon, numbered, "custom icon must change recorded geometry");
    assert!(
        numbered.contains("TextCenter { text: \"1\""),
        "numbered process step should paint its ordinal: {numbered}"
    );
    assert!(
        !icon.contains("TextCenter { text: \"1\""),
        "custom icon should replace the ordinal: {icon}"
    );
}
