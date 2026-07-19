use std::cell::RefCell;
use std::rc::Rc;

use crate::core::Rect;
use crate::prelude::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::semantic_action::{SemanticAction, SemanticActionError, SemanticActionKind};
use crate::ui::view::ViewAdapter;

fn laid_out_tree(view: impl View) -> crate::ui::WidgetTree {
    let mut tree = ViewAdapter::build(view);
    tree.root_mut()
        .expect("test tree root")
        .set_frame(Rect::new(0.0, 0.0, 320.0, 160.0));
    tree.layout();
    tree
}

fn find_automation_id(tree: &crate::ui::WidgetTree, automation_id: &str) -> ComponentId {
    tree.traverse()
        .iter()
        .copied()
        .find(|&id| {
            tree.get(id)
                .is_some_and(|node| node.automation_id() == Some(automation_id))
        })
        .expect("automation id in test tree")
}

#[test]
fn semantic_action_capabilities_are_role_driven_and_stable() {
    let button_tree = laid_out_tree(button("Run"));
    let button = button_tree.root_id().unwrap();
    assert_eq!(
        button_tree.supported_semantic_actions(button),
        vec![SemanticActionKind::Invoke, SemanticActionKind::Focus]
    );

    let input_tree = laid_out_tree(input().placeholder("Name"));
    let input = input_tree.root_id().unwrap();
    assert_eq!(
        input_tree.supported_semantic_actions(input),
        vec![
            SemanticActionKind::Focus,
            SemanticActionKind::SetValue,
            SemanticActionKind::InsertText,
        ]
    );

    let checkbox_tree = laid_out_tree(embed(Checkbox::new("Enabled")));
    let checkbox = checkbox_tree.root_id().unwrap();
    assert_eq!(
        checkbox_tree.supported_semantic_actions(checkbox),
        vec![SemanticActionKind::Focus, SemanticActionKind::Toggle]
    );
}

#[test]
fn invoke_and_toggle_use_normal_system_and_semantic_event_paths() {
    let invoked = Rc::new(RefCell::new(0usize));
    let invoked_for_handler = invoked.clone();
    let mut button_tree = laid_out_tree(button("Run").on_click_fn(move || {
        *invoked_for_handler.borrow_mut() += 1;
    }));
    let button = button_tree.root_id().unwrap();

    button_tree
        .perform_semantic_action(button, &SemanticAction::Invoke)
        .unwrap();

    assert_eq!(*invoked.borrow(), 1);
    assert_eq!(
        button_tree.managers().focus.focused_component(),
        Some(button)
    );

    let changes = Rc::new(RefCell::new(Vec::<String>::new()));
    let changes_for_handler = changes.clone();
    let mut checkbox_tree = laid_out_tree(embed(Checkbox::new("Enabled")));
    let checkbox = checkbox_tree.root_id().unwrap();
    checkbox_tree
        .handler_table()
        .on_change(checkbox, move |value| {
            changes_for_handler.borrow_mut().push(value.to_owned());
        });

    checkbox_tree
        .perform_semantic_action(checkbox, &SemanticAction::Toggle)
        .unwrap();

    assert!(checkbox_tree
        .get(checkbox)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Checkbox>()
        .unwrap()
        .is_checked());
    assert_eq!(&*changes.borrow(), &["true".to_string()]);
}

#[test]
fn set_value_and_insert_text_share_input_focus_and_text_routing() {
    let mut tree = laid_out_tree(embed(Input::new("Name").with_value("old")));
    let input = tree.root_id().unwrap();

    tree.perform_semantic_action(input, &SemanticAction::SetValue("new".to_owned()))
        .unwrap();
    tree.perform_semantic_action(input, &SemanticAction::InsertText("!".to_owned()))
        .unwrap();

    let input_component = tree
        .get(input)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Input>()
        .unwrap();
    assert_eq!(input_component.current_value(), "new!");
    assert_eq!(tree.managers().focus.focused_component(), Some(input));
    assert!(!format!("{:?}", SemanticAction::SetValue("secret".to_owned())).contains("secret"));
}

#[test]
fn select_exposes_one_index_contract_and_reuses_keyboard_change_events() {
    let changes = Rc::new(RefCell::new(Vec::<String>::new()));
    let changes_for_handler = changes.clone();
    let mut radio_tree = laid_out_tree(embed(
        Radio::new().options(vec!["Small", "Medium", "Large"]),
    ));
    let radio = radio_tree.root_id().unwrap();
    radio_tree.handler_table().on_change(radio, move |value| {
        changes_for_handler.borrow_mut().push(value.to_owned())
    });
    assert_eq!(
        radio_tree.supported_semantic_actions(radio),
        vec![SemanticActionKind::Focus, SemanticActionKind::Select]
    );
    radio_tree
        .perform_semantic_action(radio, &SemanticAction::Select("1".to_owned()))
        .unwrap();
    let radio_snapshot =
        ComponentConfigSnapshot::from_component(radio, radio_tree.get(radio).unwrap().component());
    assert_eq!(
        radio_snapshot.selection().unwrap().selected_indices,
        vec![1]
    );
    assert_eq!(
        radio_snapshot.accessibility().state.value_text.as_deref(),
        Some("Medium")
    );
    assert_eq!(&*changes.borrow(), &["1".to_owned()]);

    let mut segmented_tree = laid_out_tree(embed(
        Segmented::new(["Day", "Week", "Month"]).disable_option(1),
    ));
    let segmented = segmented_tree.root_id().unwrap();
    segmented_tree
        .perform_semantic_action(segmented, &SemanticAction::Select("2".to_owned()))
        .unwrap();
    let segmented_snapshot = ComponentConfigSnapshot::from_component(
        segmented,
        segmented_tree.get(segmented).unwrap().component(),
    );
    let selection = segmented_snapshot.selection().unwrap();
    assert_eq!(selection.selected_indices, vec![2]);
    assert_eq!(selection.disabled_indices, vec![1]);
    assert_eq!(
        segmented_snapshot
            .accessibility()
            .state
            .value_text
            .as_deref(),
        Some("Month")
    );
    assert_eq!(
        segmented_tree.perform_semantic_action(segmented, &SemanticAction::Select("1".to_owned())),
        Err(SemanticActionError::SelectionDisabled {
            target: segmented,
            index: 1,
        })
    );

    let mut select_tree = laid_out_tree(embed(
        Select::new()
            .options(vec!["Alpha", "Beta", "Gamma"])
            .placeholder("Choice"),
    ));
    let select = select_tree.root_id().unwrap();
    select_tree
        .perform_semantic_action(select, &SemanticAction::Select("1".to_owned()))
        .unwrap();
    let select_snapshot = ComponentConfigSnapshot::from_component(
        select,
        select_tree.get(select).unwrap().component(),
    );
    let selection = select_snapshot.selection().unwrap();
    assert_eq!(selection.selected_indices, vec![1]);
    assert!(!selection.expanded);
    assert_eq!(
        select_snapshot.accessibility().state.value_text.as_deref(),
        Some("Beta")
    );

    assert_eq!(
        select_tree
            .perform_semantic_action(select, &SemanticAction::Select("not-an-index".to_owned())),
        Err(SemanticActionError::InvalidValue {
            target: select,
            action: SemanticActionKind::Select,
        })
    );

    let mut loading_select_tree = laid_out_tree(embed(
        Select::new().options(["Alpha", "Beta"]).loading(true),
    ));
    let loading_select = loading_select_tree.root_id().unwrap();
    let loading_snapshot = ComponentConfigSnapshot::from_component(
        loading_select,
        loading_select_tree.get(loading_select).unwrap().component(),
    );
    assert_eq!(
        loading_snapshot.selection().unwrap().disabled_indices,
        vec![0, 1]
    );
    assert_eq!(
        loading_select_tree
            .perform_semantic_action(loading_select, &SemanticAction::Select("0".to_owned()),),
        Err(SemanticActionError::SelectionDisabled {
            target: loading_select,
            index: 0,
        })
    );

    let multiple_tree = laid_out_tree(embed(Select::multiple().options(vec!["Alpha", "Beta"])));
    let multiple = multiple_tree.root_id().unwrap();
    assert!(!multiple_tree
        .supported_semantic_actions(multiple)
        .contains(&SemanticActionKind::Select));
    assert!(
        multiple_tree
            .semantic_snapshot_body()
            .nodes
            .into_iter()
            .find(|node| node.id == multiple)
            .unwrap()
            .selection
            .unwrap()
            .multiple
    );
}

#[test]
fn increment_decrement_and_scroll_use_the_existing_widget_event_contracts() {
    let mut slider_tree = laid_out_tree(embed(Slider::new(0.0..=10.0).default_value(4.0)));
    let slider = slider_tree.root_id().unwrap();
    slider_tree
        .perform_semantic_action(slider, &SemanticAction::Increment)
        .unwrap();
    slider_tree
        .perform_semantic_action(slider, &SemanticAction::Decrement)
        .unwrap();
    assert_eq!(
        slider_tree
            .get(slider)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<Slider>()
            .unwrap()
            .current_value(),
        4.0
    );

    let value = State::new(4.0f64);
    let mut pointer_only_tree =
        laid_out_tree(embed(InputNumber::new().value(&value).keyboard(false)));
    let pointer_only = pointer_only_tree.root_id().unwrap();
    let pointer_only_changes = Rc::new(RefCell::new(Vec::<String>::new()));
    let changes_for_handler = pointer_only_changes.clone();
    pointer_only_tree
        .handler_table()
        .on_change(pointer_only, move |next| {
            changes_for_handler.borrow_mut().push(next.to_owned())
        });
    let actions = pointer_only_tree.supported_semantic_actions(pointer_only);
    assert!(!actions.contains(&SemanticActionKind::InsertText));
    assert!(actions.contains(&SemanticActionKind::Increment));
    pointer_only_tree
        .perform_semantic_action(pointer_only, &SemanticAction::Increment)
        .unwrap();
    assert_eq!(value.get(), 5.0);
    assert_eq!(&*pointer_only_changes.borrow(), &["5".to_owned()]);

    let mut scroll_tree = laid_out_tree(embed(
        ScrollView::new(ScrollDirection::Vertical)
            .size(120.0, 80.0)
            .child(Space::new().width(120.0).height(320.0)),
    ));
    let scroll = scroll_tree.root_id().unwrap();
    assert!(scroll_tree
        .supported_semantic_actions(scroll)
        .contains(&SemanticActionKind::Scroll));
    scroll_tree
        .perform_semantic_action(
            scroll,
            &SemanticAction::Scroll {
                delta: Point::new(0.0, 1.0),
            },
        )
        .unwrap();
    assert!(
        scroll_tree
            .get(scroll)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<ScrollView>()
            .unwrap()
            .scroll_y()
            > 0.0
    );
}

#[test]
fn semantic_scroll_targets_the_resolved_outer_viewport() {
    let inner = ScrollView::new(ScrollDirection::Vertical)
        .size(120.0, 320.0)
        .child(Space::new().width(120.0).height(640.0));
    let mut tree = laid_out_tree(embed(
        ScrollView::new(ScrollDirection::Vertical)
            .size(120.0, 80.0)
            .child(inner),
    ));
    let outer = tree.root_id().unwrap();
    let inner = tree.get(outer).unwrap().children()[0];

    tree.perform_semantic_action(
        outer,
        &SemanticAction::Scroll {
            delta: Point::new(0.0, 1.0),
        },
    )
    .unwrap();

    let scroll_y = |tree: &crate::ui::WidgetTree, id| {
        tree.get(id)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<ScrollView>()
            .unwrap()
            .scroll_y()
    };
    assert!(scroll_y(&tree, outer) > 0.0);
    assert_eq!(scroll_y(&tree, inner), 0.0);
}

#[test]
fn semantic_action_errors_distinguish_capability_interactivity_and_modal_blocking() {
    let mut tree = laid_out_tree(column([
        button("Outside").automation_id("outside"),
        button("Modal owner").automation_id("modal"),
    ]));
    let outside = find_automation_id(&tree, "outside");
    let modal = find_automation_id(&tree, "modal");

    assert_eq!(
        tree.perform_semantic_action(outside, &SemanticAction::Increment),
        Err(SemanticActionError::UnsupportedAction {
            target: outside,
            action: SemanticActionKind::Increment,
        })
    );

    tree.overlay_stack
        .push(modal, crate::ui::OverlayKind::Modal);
    assert_eq!(
        tree.perform_semantic_action(outside, &SemanticAction::Invoke),
        Err(SemanticActionError::Blocked {
            target: outside,
            blocker: modal,
        })
    );
    tree.overlay_stack.clear();

    tree.set_visible(outside, false);
    assert_eq!(
        tree.perform_semantic_action(outside, &SemanticAction::Invoke),
        Err(SemanticActionError::NotVisible(outside))
    );
}
