use super::*;

#[test]
fn test_build_with_children() {
    let child1 = ViewNode::leaf(Container::new());
    let child2 = ViewNode::leaf(Container::new());
    let node = ViewNode::new(Container::new(), vec![child1, child2]);
    let tree = ViewAdapter::build_nodes(node);
    let root_id = tree.root_id().expect("root node should exist");
    let root = tree.get(root_id).expect("root node should be present");
    let child_ids = root.children().to_vec();
    assert_eq!(child_ids.len(), 2);
}

#[test]
fn test_expand_key_and_zindex() {
    let node = ViewNode::leaf(Container::new())
        .key("my-container")
        .z_index(10);
    let wnode = ViewAdapter::expand(node);
    assert_eq!(wnode.key, Some("my-container".into()));
    assert_eq!(wnode.z_index, 10);
}

#[test]
fn expand_carries_view_visibility_into_widget_metadata() {
    let node = ViewNode::leaf(Container::new()).visible(false);

    let wnode = ViewAdapter::expand(node);

    assert!(!wnode.visible);
}

#[test]
fn test_apply_style_container() {
    let mut style = Style::default();
    style.background = Some(crate::ui::style::ColorValue::Custom(Color::red()));
    let widget: Box<dyn WidgetComponent> = Box::new(Container::new());
    let styled = ViewAdapter::apply_style(widget, &style, None, None);
    if let Some(c) = styled.as_any().downcast_ref::<Container>() {
        assert_eq!(
            c.style.background,
            Some(crate::ui::style::ColorValue::Custom(Color::red()))
        );
    } else {
        panic!("expected Container");
    }
}

#[test]
fn container_view_style_preserves_builder_layout_style() {
    use crate::ui::view::{column, label};

    let tree = ViewAdapter::build_nodes(column([label("A")]).padding(8.0));
    let root_id = tree.root_id().expect("root node should exist");
    let root = tree.get(root_id).expect("root node should be present");
    let container = root
        .component()
        .as_any()
        .downcast_ref::<Container>()
        .expect("root should be Container");

    assert_eq!(container.style.flex_direction, FlexDirection::Column);
    assert_eq!(container.style.flex_grow, 1.0);
    assert_eq!(container.style.padding, EdgeInsets::uniform(8.0));
}

#[test]
fn reconcile_container_view_style_preserves_builder_layout_style() {
    use crate::ui::view::{column, label};

    let mut tree = ViewAdapter::build_nodes(column([label("A")]));

    ViewAdapter::reconcile_nodes(&mut tree, column([label("A")]).padding(8.0));

    let root_id = tree.root_id().expect("root node should exist");
    let root = tree.get(root_id).expect("root node should be present");
    let container = root
        .component()
        .as_any()
        .downcast_ref::<Container>()
        .expect("root should be Container");

    assert_eq!(container.style.flex_direction, FlexDirection::Column);
    assert_eq!(container.style.flex_grow, 1.0);
    assert_eq!(container.style.padding, EdgeInsets::uniform(8.0));
}

#[test]
fn reconcile_view_visibility_preserves_identity_and_child_gate() {
    use crate::ui::view::{button, column, EventExt};

    let mut tree =
        ViewAdapter::build(column([button("Child").focusable(true).visible(false)]).visible(false));
    let root = tree.root_id().expect("root");
    let child = tree.get(root).expect("root").children()[0];
    tree.get_mut(root)
        .expect("root")
        .set_frame(Rect::new(0.0, 0.0, 160.0, 60.0));
    tree.get_mut(child)
        .expect("child")
        .set_frame(Rect::new(0.0, 0.0, 120.0, 40.0));

    assert!(!tree.get(root).expect("root").visible());
    assert!(!tree.get(child).expect("child").visible());
    assert_eq!(tree.hit_test(Point::new(20.0, 20.0)), None);
    assert!(tree.collect_focusable().is_empty());

    ViewAdapter::reconcile(
        &mut tree,
        column([button("Child").focusable(true).visible(false)]).visible(true),
    );

    assert_eq!(tree.root_id(), Some(root));
    assert_eq!(tree.get(root).expect("root").children(), &[child]);
    assert!(tree.get(root).expect("root").visible());
    assert!(!tree.get(child).expect("child").visible());
    assert_eq!(tree.hit_test(Point::new(20.0, 20.0)), Some(root));
    assert!(tree.collect_focusable().is_empty());
}

#[test]
fn reconcile_same_view_keeps_invalidation_empty() {
    use crate::ui::view::label;

    let mut tree = ViewAdapter::build(label("same"));
    tree.reset_invalidation();

    ViewAdapter::reconcile(&mut tree, label("same"));

    assert!(!tree.has_render_work());
    assert!(!tree
        .invalidation
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .has_layout());
}

#[test]
fn unused_state_created_during_view_build_does_not_bind_root() {
    use crate::ui::state::State;
    use crate::ui::widgets::Label;

    let captured: Arc<Mutex<Option<State<i32>>>> = Arc::new(Mutex::new(None));
    let captured_for_build = captured.clone();
    let root = ViewAdapter::capture_root(move || {
        let state = State::new(1);
        *captured_for_build.lock().unwrap_or_else(|e| e.into_inner()) = Some(state.clone());
        ViewNode::leaf(Label::new("static"))
    });
    let mut tree = ViewAdapter::build_nodes(root);
    let state = captured
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .expect("state should be captured");

    tree.reset_invalidation();
    assert!(!tree.take_reconcile_requested());

    state.set(2);

    assert!(!tree.take_reconcile_requested());
    assert!(!tree.has_render_work());
}

#[test]
fn state_get_during_view_build_binds_root_reconcile() {
    use crate::ui::state::State;
    use crate::ui::widgets::Label;

    let state = State::new(1);
    let state_for_build = state.clone();
    let root = ViewAdapter::capture_root(move || {
        let _ = state_for_build.get();
        ViewNode::leaf(Label::new("static"))
    });
    let mut tree = ViewAdapter::build_nodes(root);

    tree.reset_invalidation();
    assert!(!tree.take_reconcile_requested());

    state.set(2);

    assert!(tree.take_reconcile_requested());
    // Reconcile 请求本身不推 Paint；真正脏区在后续 reconcile_nodes 后产生。
    assert!(!tree.has_render_work());
}

#[test]
fn state_get_before_dynamic_label_keeps_root_reconcile() {
    use crate::ui::state::State;
    use crate::ui::view::{column, dynamic_label, label};

    let page = State::new(0usize);
    let page_for_build = page.clone();
    let page_for_label = page.clone();
    let root = ViewAdapter::capture_root(move || {
        let idx = page_for_build.get();
        column([
            label(format!("page-{idx}")),
            dynamic_label(move || format!("detail-{}", page_for_label.get())),
        ])
    });
    let mut tree = ViewAdapter::build_nodes(root);

    tree.reset_invalidation();
    assert!(!tree.take_reconcile_requested());

    page.set(1);

    assert!(tree.take_reconcile_requested());
}

#[test]
fn grid_style_tracks_drive_layout() {
    use crate::ui::layout::GridTrack;
    use crate::ui::view::{grid, label};

    let mut tree = ViewAdapter::build(
        grid([label("A"), label("B")])
            .columns(vec![GridTrack::Px(50.0), GridTrack::Px(70.0)])
            .gap(10.0),
    );
    let root_id = tree.root_id().expect("grid root should exist");
    tree.get_mut(root_id)
        .expect("grid root should be present")
        .set_frame(Rect::new(0.0, 0.0, 140.0, 40.0));

    tree.push_layout_invalidation(root_id);
    tree.layout();

    let children = tree
        .get(root_id)
        .expect("grid root should remain present")
        .children()
        .to_vec();
    assert_eq!(children.len(), 2);
    let first = tree.get(children[0]).unwrap().frame();
    let second = tree.get(children[1]).unwrap().frame();
    assert_eq!(first.x, 0.0);
    assert_eq!(first.y, 0.0);
    assert_eq!(first.h, 40.0);
    assert_eq!(second.x, 60.0);
    assert_eq!(second.y, 0.0);
    assert_eq!(second.h, 40.0);
}

#[test]
fn reconcile_grid_syncs_responsive_columns_and_reflows_children() {
    use crate::ui::view::{grid, label};
    use crate::ui::widgets::Col;

    let mut tree = ViewAdapter::build(
        grid([label("A"), label("B")])
            .responsive()
            .cols(vec![Col::new().span(12), Col::new().span(12)]),
    );
    let root_id = tree.root_id().expect("responsive grid root");
    tree.get_mut(root_id)
        .expect("responsive grid root node")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 200.0));
    tree.push_layout_invalidation(root_id);
    tree.layout();
    let children = tree
        .get(root_id)
        .expect("responsive grid before reconcile")
        .children()
        .to_vec();
    assert!(tree.get(children[1]).expect("second child").frame().x > 0.0);

    ViewAdapter::reconcile(
        &mut tree,
        grid([label("A"), label("B")])
            .responsive()
            .cols(vec![Col::new(), Col::new()]),
    );
    tree.layout();

    assert_eq!(tree.root_id(), Some(root_id));
    assert_eq!(tree.get(children[0]).expect("first child").frame().x, 0.0);
    assert_eq!(tree.get(children[1]).expect("second child").frame().x, 0.0);
    assert!(
        tree.get(children[1]).expect("second child").frame().y
            > tree.get(children[0]).expect("first child").frame().y
    );
}

#[test]
fn reconcile_reuses_keyed_children_and_updates_label_text() {
    use crate::ui::view::{column, label};
    use crate::ui::widgets::Label;

    let mut tree = ViewAdapter::build_nodes(column(vec![label("A").key("a"), label("B").key("b")]));
    let app_state = AppState::new();
    tree.set_app_state(app_state.clone());
    tree.layout();
    let root_id = tree.root_id().expect("root should exist");
    let old_children = tree.get(root_id).unwrap().children().to_vec();
    let old_b = old_children[1];
    let old_b_handle = app_state.get_handle(old_b).unwrap();
    assert_eq!(old_b_handle.text().as_deref(), Some("B"));

    ViewAdapter::reconcile_nodes(
        &mut tree,
        column(vec![label("B2").key("b"), label("A2").key("a")]),
    );

    let new_children = tree.get(root_id).unwrap().children().to_vec();
    assert_eq!(new_children, vec![old_children[1], old_children[0]]);
    assert_eq!(old_b_handle.text().as_deref(), Some("B2"));
    let first = tree
        .get(new_children[0])
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap();
    let second = tree
        .get(new_children[1])
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap();
    assert_eq!(first.text(), "B2");
    assert_eq!(second.text(), "A2");
}

#[test]
fn reconcile_reregisters_root_handlers() {
    use crate::ui::view::button;

    let old_hits = Rc::new(Cell::new(0));
    let new_hits = Rc::new(Cell::new(0));
    let old_for_handler = old_hits.clone();
    let new_for_handler = new_hits.clone();

    let mut tree = ViewAdapter::build(button("Old").on_click_fn(move || {
        old_for_handler.set(old_for_handler.get() + 1);
    }));
    let root_id = tree.root_id().expect("button root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 80.0, 32.0));

    let pos = Point::new(4.0, 4.0);
    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    ViewAdapter::reconcile(
        &mut tree,
        button("New").on_click_fn(move || {
            new_for_handler.set(new_for_handler.get() + 1);
        }),
    );

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(old_hits.get(), 1);
    assert_eq!(new_hits.get(), 1);
    let button = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Button>()
        .unwrap();
    assert_eq!(button.text(), "New");
}

#[test]
fn reconcile_same_type_select_preserves_open_state() {
    use crate::ui::widgets::Select;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Select::new().options(vec!["A", "B"]).placeholder("old"),
    ));
    let root_id = tree.root_id().expect("select root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Select>()
        .unwrap()
        .open();

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Select::new()
                .options(vec!["A", "B", "C"])
                .placeholder("new"),
        ),
    );

    let select = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Select>()
        .unwrap();
    assert!(select.is_open());
    assert!(matches!(
        select.snapshot_fields(),
        SnapshotFields::Select {
            placeholder,
            options,
            ..
        } if placeholder == "new" && options == vec!["A", "B", "C"]
    ));
}

#[test]
fn reconcile_checkbox_patches_instance_and_syncs_snapshot_fields() {
    use crate::ui::state::State;
    use crate::ui::widgets::Checkbox;

    let checked = State::new(false);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Checkbox::new("old").checked(&checked)));
    let root_id = tree.root_id().expect("checkbox root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Checkbox>()
        .unwrap() as *const Checkbox;

    checked.set(true);
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Checkbox::new("new").checked(&checked).disabled(true)),
    );

    let checkbox = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Checkbox>()
        .unwrap();
    assert_eq!(checkbox as *const Checkbox, before_ptr);
    assert!(checkbox.is_checked());
    assert!(matches!(
        checkbox.snapshot_fields(),
        SnapshotFields::Checkbox {
            checked: true,
            disabled: true,
            label,
        } if label == "new"
    ));
}

#[test]
fn reconcile_switch_patches_instance_and_syncs_snapshot_fields() {
    use crate::ui::state::State;
    use crate::ui::widgets::Switch;

    let checked = State::new(false);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Switch::new().checked(&checked)));
    let root_id = tree.root_id().expect("switch root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Switch>()
        .unwrap() as *const Switch;

    checked.set(true);
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Switch::new().checked(&checked).disabled(true)),
    );

    let switch = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Switch>()
        .unwrap();
    assert_eq!(switch as *const Switch, before_ptr);
    assert!(switch.is_checked());
    assert_eq!(
        switch.snapshot_fields(),
        SnapshotFields::Switch {
            checked: true,
            disabled: true,
            size: 22.0,
        }
    );
}

#[test]
fn reconcile_radio_patches_instance_and_syncs_snapshot_fields() {
    use crate::ui::state::State;
    use crate::ui::widgets::{Radio, RadioDirection};

    let value = State::new("A".to_string());
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Radio::new()
            .group_name("old")
            .options(vec!["A", "B"])
            .value(&value),
    ));
    let root_id = tree.root_id().expect("radio root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Radio>()
        .unwrap() as *const Radio;

    value.set("E".to_string());
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Radio::new()
                .group_name("size")
                .options(vec!["C", "D", "E"])
                .value(&value)
                .disabled(true)
                .vertical(),
        ),
    );

    let radio = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Radio>()
        .unwrap();
    assert_eq!(radio as *const Radio, before_ptr);
    assert_eq!(
        radio.snapshot_fields(),
        SnapshotFields::Radio {
            group_name: "size".to_string(),
            options: vec!["C".to_string(), "D".to_string(), "E".to_string()],
            selected: 2,
            disabled: true,
            direction: RadioDirection::Vertical,
            item_h: crate::ui::config::control_height(ControlSize::Medium),
        }
    );
}

#[test]
fn reconcile_slider_patches_instance_and_syncs_snapshot_fields() {
    use crate::ui::state::State;
    use crate::ui::widgets::Slider;

    let value = State::new(10.0);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Slider::new(0.0..=100.0).step(1.0).value(&value),
    ));
    let root_id = tree.root_id().expect("slider root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Slider>()
        .unwrap() as *const Slider;

    value.set(3.0);
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Slider::new(-10.0..=10.0).step(0.5).value(&value)),
    );

    let slider = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Slider>()
        .unwrap();
    assert_eq!(slider as *const Slider, before_ptr);
    assert_eq!(slider.current_value(), 3.0);
    assert_eq!(
        slider.snapshot_fields(),
        SnapshotFields::Slider {
            min: -10.0,
            max: 10.0,
            step: 0.5,
            value: 3.0,
        }
    );
}

#[test]
fn reconcile_input_number_patches_instance_and_syncs_snapshot_fields() {
    use crate::ui::state::State;
    use crate::ui::widgets::InputNumber;

    let value = State::new(4.0);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        InputNumber::new()
            .placeholder("old")
            .min(0.0)
            .max(10.0)
            .step(1.0)
            .value(&value),
    ));
    let root_id = tree.root_id().expect("input number root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<InputNumber>()
        .unwrap() as *const InputNumber;

    value.set(2.5);
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            InputNumber::new()
                .placeholder("new")
                .min(-5.0)
                .max(8.0)
                .step(0.25)
                .value(&value)
                .disabled(true),
        ),
    );

    let input_number = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<InputNumber>()
        .unwrap();
    assert_eq!(input_number as *const InputNumber, before_ptr);
    assert_eq!(input_number.current_value(), 2.5);
    assert_eq!(
        input_number.snapshot_fields(),
        SnapshotFields::InputNumber {
            value: 2.5,
            min: -5.0,
            max: 8.0,
            step: 0.25,
            placeholder: "new".to_string(),
            disabled: true,
            keyboard: true,
            formatted: false,
            display_value: Some("2.5".to_string()),
        }
    );
}

#[test]
fn reconcile_rate_patches_instance_and_syncs_snapshot_fields() {
    use crate::ui::state::State;
    use crate::ui::widgets::Rate;

    let value = State::new(1u32);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Rate::new().count(5).value(&value)));
    let root_id = tree.root_id().expect("rate root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Rate>()
        .unwrap() as *const Rate;

    value.set(3);
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Rate::new()
                .count(7)
                .value(&value)
                .allow_half()
                .disabled(true)
                .clearable()
                .character("#"),
        ),
    );

    let rate = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Rate>()
        .unwrap();
    assert_eq!(rate as *const Rate, before_ptr);
    assert_eq!(
        rate.snapshot_fields(),
        SnapshotFields::Rate {
            count: 7,
            value: 3,
            half: true,
            disabled: true,
            clearable: true,
            character: "#".to_string(),
        }
    );
}

#[test]
fn reconcile_segmented_patches_instance_and_syncs_static_config() {
    use crate::ui::state::State;
    use crate::ui::widgets::Segmented;

    let value = State::new("Weekly".to_string());
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Segmented::new(["Daily", "Weekly"]).value(&value),
    ));
    let root_id = tree.root_id().expect("segmented root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Segmented>()
        .unwrap() as *const Segmented;

    value.set("Week".to_string());
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Segmented::new(["Day", "Week", "Month"])
                .value(&value)
                .disable_option(2)
                .disabled(true),
        ),
    );

    let segmented = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Segmented>()
        .unwrap();
    assert_eq!(segmented as *const Segmented, before_ptr);
    assert_eq!(
        segmented.snapshot_fields(),
        SnapshotFields::Segmented {
            options: vec!["Day".to_string(), "Week".to_string(), "Month".to_string()],
            selected: 1,
            disabled: true,
            disabled_options: vec![false, false, true],
        }
    );
}

#[test]
fn reconcile_form_item_preserves_status_and_syncs_config() {
    use crate::ui::widgets::{FormItem, FormLayout, ValidateStatus};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        FormItem::new("Email")
            .name("email")
            .status(ValidateStatus::Error),
    ));
    let root_id = tree.root_id().expect("form item root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<FormItem>()
        .unwrap() as *const FormItem;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            FormItem::new("Name")
                .name("name")
                .required(true)
                .help("Required")
                .label_width(120.0)
                .layout(FormLayout::Vertical),
        ),
    );

    let form_item = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<FormItem>()
        .unwrap();
    assert_eq!(form_item as *const FormItem, before_ptr);
    assert_eq!(form_item.get_status(), ValidateStatus::Error);
    assert_eq!(
        form_item.snapshot_fields(),
        SnapshotFields::FormItem {
            label: "Name".to_string(),
            name: "name".to_string(),
            required: true,
            help: "Required".to_string(),
            label_width: 120.0,
            layout: FormLayout::Vertical,
        }
    );
}

#[test]
fn reconcile_form_patches_instance_and_syncs_layout_config() {
    use crate::ui::widgets::{Form, FormLayout};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Form::new()));
    let root_id = tree.root_id().expect("form root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Form>()
        .unwrap() as *const Form;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Form::new()
                .label_width(104.0)
                .gap(12.0)
                .layout(FormLayout::Inline),
        ),
    );

    let form = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Form>()
        .unwrap();
    assert_eq!(form as *const Form, before_ptr);
    assert_eq!(
        form.snapshot_fields(),
        SnapshotFields::Form {
            label_width: 104.0,
            gap: 12.0,
            layout: FormLayout::Inline,
        }
    );
}
