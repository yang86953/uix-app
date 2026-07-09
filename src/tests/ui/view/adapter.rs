use super::*;
use crate::draw::Color;
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::ViewNode;
use crate::ui::widgets::{Button, Container};
use crate::ui::{EventResult, SnapshotFields, SystemEvent};

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
    use crate::core::EdgeInsets;
    use crate::ui::style::FlexDirection;
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
    use crate::core::EdgeInsets;
    use crate::ui::style::FlexDirection;
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
    use std::sync::{Arc, Mutex};

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
    assert!(tree.has_render_work());
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
    use crate::core::Rect;
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
fn reconcile_reuses_keyed_children_and_updates_label_text() {
    use crate::ui::view::{column, label};
    use crate::ui::widgets::Label;
    use crate::ui::AppState;

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
    use crate::core::{Point, Rect};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::view::button;
    use crate::ui::SystemEvent;
    use std::cell::Cell;
    use std::rc::Rc;

    let old_hits = Rc::new(Cell::new(0));
    let new_hits = Rc::new(Cell::new(0));
    let old_for_handler = old_hits.clone();
    let new_for_handler = new_hits.clone();

    let mut tree = ViewAdapter::build(button("Old").on_click(move || {
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
        button("New").on_click(move || {
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
    use crate::ui::widgets::Checkbox;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Checkbox::new("old")));
    let root_id = tree.root_id().expect("checkbox root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Checkbox>()
        .unwrap() as *const Checkbox;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Checkbox::new("new").checked(true).disabled(true)),
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
    use crate::ui::widgets::Switch;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Switch::new()));
    let root_id = tree.root_id().expect("switch root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Switch>()
        .unwrap() as *const Switch;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Switch::new().checked(true).disabled(true)),
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
    use crate::ui::widgets::{Radio, RadioDirection};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Radio::new().options(vec!["A", "B"]).selected(0),
    ));
    let root_id = tree.root_id().expect("radio root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Radio>()
        .unwrap() as *const Radio;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Radio::new()
                .options(vec!["C", "D", "E"])
                .selected(2)
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
            options: vec!["C".to_string(), "D".to_string(), "E".to_string()],
            selected: 2,
            disabled: true,
            direction: RadioDirection::Vertical,
            item_h: 24.0,
        }
    );
}

#[test]
fn reconcile_slider_patches_instance_and_syncs_snapshot_fields() {
    use crate::ui::widgets::Slider;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Slider::new().range(0.0, 100.0).step(1.0).value(10.0),
    ));
    let root_id = tree.root_id().expect("slider root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Slider>()
        .unwrap() as *const Slider;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Slider::new().range(-10.0, 10.0).step(0.5).value(3.0)),
    );

    let slider = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Slider>()
        .unwrap();
    assert_eq!(slider as *const Slider, before_ptr);
    assert_eq!(slider.get_value(), 3.0);
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
    use crate::ui::widgets::InputNumber;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        InputNumber::new("old")
            .min(0.0)
            .max(10.0)
            .step(1.0)
            .value(4.0),
    ));
    let root_id = tree.root_id().expect("input number root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<InputNumber>()
        .unwrap() as *const InputNumber;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            InputNumber::new("new")
                .min(-5.0)
                .max(8.0)
                .step(0.25)
                .value(2.5)
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
    assert_eq!(input_number.get_value(), 2.5);
    assert_eq!(
        input_number.snapshot_fields(),
        SnapshotFields::InputNumber {
            value: 2.5,
            min: -5.0,
            max: 8.0,
            step: 0.25,
            placeholder: "new".to_string(),
            disabled: true,
        }
    );
}

#[test]
fn reconcile_rate_patches_instance_and_syncs_snapshot_fields() {
    use crate::ui::widgets::Rate;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Rate::new().count(5).value(1)));
    let root_id = tree.root_id().expect("rate root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Rate>()
        .unwrap() as *const Rate;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Rate::new()
                .count(7)
                .value(3)
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
    use crate::ui::widgets::Segmented;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Segmented::new()
            .options(vec!["Daily", "Weekly"])
            .selected(1),
    ));
    let root_id = tree.root_id().expect("segmented root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Segmented>()
        .unwrap() as *const Segmented;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Segmented::new()
                .options(vec!["Day", "Week", "Month"])
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

#[test]
fn reconcile_table_preserves_runtime_selection_and_syncs_config() {
    use crate::core::Point;
    use crate::ui::widgets::{Table, TableColumn};
    use crate::ui::{KeyMod, MouseButton, SnapshotTableColumn};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Table::new()
            .columns(vec![TableColumn::new("Name", 120.0).sortable(true)])
            .rows(vec![
                vec!["Ada".to_string()],
                vec!["Grace".to_string()],
                vec!["Lin".to_string()],
            ])
            .page_size(2),
    ));
    let root_id = tree.root_id().expect("table root should exist");
    {
        let table = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Table>()
            .unwrap();
        table.set_selected_row(Some(1));
        assert_eq!(
            crate::ui::EventHandler::on_event(
                table,
                &SystemEvent::PointerDown {
                    pos: Point::new(8.0, 34.0),
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                },
            ),
            EventResult::Handled
        );
    }
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Table>()
        .unwrap() as *const Table;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Table::new()
                .columns(vec![TableColumn::new("City", 96.0).filterable(true)])
                .rows(vec![vec!["Paris".to_string()], vec!["London".to_string()]])
                .row_height(36.0)
                .empty_text("No rows")
                .expandable(72.0, |_idx, _ctx, _rect| {})
                .page_size(8),
        ),
    );

    let table = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Table>()
        .unwrap();
    assert_eq!(table as *const Table, before_ptr);
    assert_eq!(table.selected_row(), Some(1));
    assert_eq!(table.checked_rows(), &[0]);
    assert_eq!(
        table.snapshot_fields(),
        SnapshotFields::Table {
            columns: vec![SnapshotTableColumn {
                title: "City".to_string(),
                width: 96.0,
                sortable: false,
                filterable: true,
                filters: Vec::new(),
            }],
            rows: vec![vec!["Paris".to_string()], vec!["London".to_string()]],
            row_h: 36.0,
            header_h: 32.0,
            expand_height: 72.0,
            empty_text: "No rows".to_string(),
            page_size: 8,
        }
    );
}

#[test]
fn reconcile_progress_bar_preserves_animation_phase_and_syncs_config() {
    use crate::draw::Color;
    use crate::ui::traits::WidgetAnimation;
    use crate::ui::widgets::{ProgressBar, ProgressMode, ProgressType};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(ProgressBar::new().indeterminate()));
    let root_id = tree.root_id().expect("progress root should exist");
    {
        let progress = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<ProgressBar>()
            .unwrap();
        assert!(WidgetAnimation::update_animation(progress, 0.25));
    }
    let before = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ProgressBar>()
        .unwrap()
        .animation_phase();
    assert!(before > 0.0);

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            ProgressBar::new()
                .progress(0.6)
                .stroke_color(Color::red())
                .track_color(Color::blue())
                .size(96.0, 12.0)
                .circle(),
        ),
    );

    let progress = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ProgressBar>()
        .unwrap();
    assert_eq!(progress.animation_phase(), before);
    assert_eq!(
        progress.snapshot_fields(),
        SnapshotFields::ProgressBar {
            progress: 0.6,
            mode: ProgressMode::Determinate(0.6),
            stroke_color: Some(Color::red()),
            track_color: Some(Color::blue()),
            height: 12.0,
            width: 96.0,
            round: true,
            progress_type: ProgressType::Circle,
        }
    );
}

#[test]
fn reconcile_spin_preserves_phase_and_syncs_config() {
    use crate::draw::Color;
    use crate::ui::traits::WidgetAnimation;
    use crate::ui::widgets::{Spin, SpinSize};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Spin::new()));
    let root_id = tree.root_id().expect("spin root should exist");
    {
        let spin = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Spin>()
            .unwrap();
        assert!(WidgetAnimation::update_animation(spin, 0.25));
    }
    let before = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Spin>()
        .unwrap()
        .phase();
    assert!(before > 0.0);

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Spin::new()
                .large()
                .color(Color::green())
                .spinning(false)
                .tip("Loading")
                .wrapper_mode(),
        ),
    );

    let spin = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Spin>()
        .unwrap();
    assert_eq!(spin.phase(), before);
    assert_eq!(
        spin.snapshot_fields(),
        SnapshotFields::Spin {
            size: SpinSize::Large,
            color: Some(Color::green()),
            spinning: false,
            tip: "Loading".to_string(),
            wrapper_mode: true,
        }
    );
}

#[test]
fn reconcile_float_button_patches_instance_and_syncs_config() {
    use crate::ui::widgets::FloatButton;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(FloatButton::new("+")));
    let root_id = tree.root_id().expect("float button root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<FloatButton>()
        .unwrap() as *const FloatButton;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            FloatButton::new("up")
                .tooltip("Top")
                .badge(9)
                .size(48.0)
                .position(12.0, 24.0),
        ),
    );

    let float_button = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<FloatButton>()
        .unwrap();
    assert_eq!(float_button as *const FloatButton, before_ptr);
    assert_eq!(
        float_button.snapshot_fields(),
        SnapshotFields::FloatButton {
            icon: "up".to_string(),
            tooltip: "Top".to_string(),
            badge_count: 9,
            size: 48.0,
            x: 12.0,
            y: 24.0,
        }
    );
}

#[test]
fn reconcile_back_top_preserves_visibility_and_syncs_threshold() {
    use crate::ui::widgets::BackTop;

    let mut tree =
        ViewAdapter::build_nodes(ViewNode::leaf(BackTop::new().visibility_height(100.0)));
    let root_id = tree.root_id().expect("back top root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<BackTop>()
        .unwrap()
        .update_visibility(200.0);

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(BackTop::new().visibility_height(240.0)),
    );

    let back_top = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<BackTop>()
        .unwrap();
    assert!(back_top.is_visible());
    assert_eq!(
        back_top.snapshot_fields(),
        SnapshotFields::BackTop {
            visibility_height: 240.0,
        }
    );
}

#[test]
fn reconcile_alert_patches_instance_and_syncs_config() {
    use crate::native::traits::system::StatusLevel;
    use crate::ui::widgets::Alert;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Alert::new("old")));
    let root_id = tree.root_id().expect("alert root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Alert>()
        .unwrap() as *const Alert;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Alert::new("new")
                .description("details")
                .type_(StatusLevel::Warning)
                .closable(),
        ),
    );

    let alert = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Alert>()
        .unwrap();
    assert_eq!(alert as *const Alert, before_ptr);
    assert_eq!(
        alert.snapshot_fields(),
        SnapshotFields::Alert {
            message: "new".to_string(),
            description: "details".to_string(),
            type_: StatusLevel::Warning,
            closable: true,
            show_icon: true,
        }
    );
}

#[test]
fn reconcile_tag_patches_instance_and_syncs_config() {
    use crate::draw::Color;
    use crate::ui::widgets::{Tag, TagColor};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Tag::new("old")));
    let root_id = tree.root_id().expect("tag root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Tag>()
        .unwrap() as *const Tag;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Tag::new("new")
                .color(TagColor::Success)
                .custom_color(Color::green())
                .closable()
                .checkable(true)
                .font_size(14.0),
        ),
    );

    let tag = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Tag>()
        .unwrap();
    assert_eq!(tag as *const Tag, before_ptr);
    assert_eq!(
        tag.snapshot_fields(),
        SnapshotFields::Tag {
            text: "new".to_string(),
            color: TagColor::Success,
            closable: true,
            font_size: 14.0,
            custom_color: Some(Color::green()),
            checkable: true,
        }
    );
}

#[test]
fn reconcile_empty_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Empty;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Empty::new().description("old")));
    let root_id = tree.root_id().expect("empty root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Empty>()
        .unwrap() as *const Empty;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Empty::new().description("new").icon("search").image("file")),
    );

    let empty = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Empty>()
        .unwrap();
    assert_eq!(empty as *const Empty, before_ptr);
    assert_eq!(
        empty.snapshot_fields(),
        SnapshotFields::Empty {
            description: "new".to_string(),
            icon_name: "search".to_string(),
            image: "file".to_string(),
        }
    );
}

#[test]
fn reconcile_skeleton_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{Skeleton, SkeletonShape};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Skeleton::new()));
    let root_id = tree.root_id().expect("skeleton root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Skeleton>()
        .unwrap() as *const Skeleton;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Skeleton::new()
                .shape(SkeletonShape::Circle)
                .size(64.0, 64.0),
        ),
    );

    let skeleton = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Skeleton>()
        .unwrap();
    assert_eq!(skeleton as *const Skeleton, before_ptr);
    assert_eq!(
        skeleton.snapshot_fields(),
        SnapshotFields::Skeleton {
            shape: SkeletonShape::Circle,
            width: 64.0,
            height: 64.0,
        }
    );
}

#[test]
fn reconcile_card_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Card;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Card::new().title("old")));
    let root_id = tree.root_id().expect("card root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Card>()
        .unwrap() as *const Card;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Card::new()
                .title("new")
                .bordered(false)
                .hoverable()
                .size(240.0, 120.0)
                .padding(20.0)
                .elevation(3)
                .flex_grow(1.5)
                .actions(vec!["Edit", "Delete"]),
        ),
    );

    let card = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Card>()
        .unwrap();
    assert_eq!(card as *const Card, before_ptr);
    assert_eq!(
        card.snapshot_fields(),
        SnapshotFields::Card {
            title: Some("new".to_string()),
            bordered: false,
            hoverable: true,
            fixed_width: Some(240.0),
            fixed_height: Some(120.0),
            padding: 20.0,
            elevation: 3,
            flex_grow: 1.5,
            actions: vec!["Edit".to_string(), "Delete".to_string()],
        }
    );
}

#[test]
fn reconcile_image_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Image;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Image::new(40.0, 20.0).src("old.png")));
    let root_id = tree.root_id().expect("image root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Image>()
        .unwrap() as *const Image;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Image::new(80.0, 60.0)
                .src("new.png")
                .alt("Preview")
                .fallback("fallback")
                .radius(8.0)
                .preview(false)
                .fit(false),
        ),
    );

    let image = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Image>()
        .unwrap();
    assert_eq!(image as *const Image, before_ptr);
    assert_eq!(
        image.snapshot_fields(),
        SnapshotFields::Image {
            src: "new.png".to_string(),
            alt: "Preview".to_string(),
            fallback: "fallback".to_string(),
            width: 80.0,
            height: 60.0,
            radius: 8.0,
            preview: false,
            fit: false,
        }
    );
}

#[test]
fn reconcile_calendar_preserves_selection_and_syncs_config() {
    use crate::core::Point;
    use crate::ui::widgets::Calendar;
    use crate::ui::{KeyMod, MouseButton};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Calendar::new()));
    let root_id = tree.root_id().expect("calendar root should exist");
    {
        let calendar = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Calendar>()
            .unwrap();
        assert_eq!(
            crate::ui::EventHandler::on_event(
                calendar,
                &SystemEvent::PointerDown {
                    pos: Point::new(5.0, 85.0),
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                },
            ),
            EventResult::Handled
        );
    }
    let selected_before = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Calendar>()
        .unwrap()
        .selected_day();
    assert!(selected_before.is_some());

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Calendar::new().cell_size(32.0).year_jump(true)),
    );

    let calendar = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Calendar>()
        .unwrap();
    assert_eq!(calendar.selected_day(), selected_before);
    assert_eq!(
        calendar.snapshot_fields(),
        SnapshotFields::Calendar {
            cell_size: 32.0,
            year_jump: true,
        }
    );
}

#[test]
fn reconcile_descriptions_patches_instance_and_syncs_config() {
    use crate::native::traits::input::ControlSize;
    use crate::ui::widgets::{Descriptions, DescriptionsItem};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Descriptions::new().title("old")));
    let root_id = tree.root_id().expect("descriptions root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Descriptions>()
        .unwrap() as *const Descriptions;
    let items = vec![DescriptionsItem::new("Name", "Ada").span(2)];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Descriptions::new()
                .title("Profile")
                .items(items.clone())
                .bordered(true)
                .column(2)
                .label_width(88.0)
                .size(ControlSize::Small),
        ),
    );

    let descriptions = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Descriptions>()
        .unwrap();
    assert_eq!(descriptions as *const Descriptions, before_ptr);
    assert_eq!(
        descriptions.snapshot_fields(),
        SnapshotFields::Descriptions {
            title: "Profile".to_string(),
            items,
            bordered: true,
            column: 2,
            label_width: 88.0,
            descriptions_size: ControlSize::Small,
        }
    );
}

#[test]
fn reconcile_result_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{Result as ResultWidget, ResultType};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(ResultWidget::new(ResultType::Info)));
    let root_id = tree.root_id().expect("result root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ResultWidget>()
        .unwrap() as *const ResultWidget;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            ResultWidget::new(ResultType::Success)
                .title("Done")
                .subtitle("All set")
                .extra_text("Continue"),
        ),
    );

    let result = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ResultWidget>()
        .unwrap();
    assert_eq!(result as *const ResultWidget, before_ptr);
    assert_eq!(
        result.snapshot_fields(),
        SnapshotFields::Result {
            result_type: ResultType::Success,
            title: "Done".to_string(),
            subtitle: "All set".to_string(),
            extra_text: "Continue".to_string(),
        }
    );
}

#[test]
fn reconcile_list_patches_instance_and_syncs_config() {
    use crate::native::traits::input::ControlSize;
    use crate::ui::widgets::List;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(List::new().items(vec!["old"])));
    let root_id = tree.root_id().expect("list root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<List>()
        .unwrap() as *const List;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            List::new()
                .header("Header")
                .footer("Footer")
                .bordered(false)
                .size(ControlSize::Large)
                .items(vec!["Ada", "Grace"])
                .load_more("More"),
        ),
    );

    let list = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<List>()
        .unwrap();
    assert_eq!(list as *const List, before_ptr);
    assert_eq!(
        list.snapshot_fields(),
        SnapshotFields::List {
            header: "Header".to_string(),
            footer: "Footer".to_string(),
            bordered: false,
            list_size: ControlSize::Large,
            items: vec!["Ada".to_string(), "Grace".to_string()],
            load_more_text: "More".to_string(),
        }
    );
}

#[test]
fn reconcile_layout_shell_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{Content, Footer, Header, Layout, Sider};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Layout::new()));
    let root_id = tree.root_id().expect("layout root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Layout>()
        .unwrap() as *const Layout;

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Layout::new().bg(Color::red())));

    let layout = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Layout>()
        .unwrap();
    assert_eq!(layout as *const Layout, before_ptr);
    assert_eq!(
        layout.snapshot_fields(),
        SnapshotFields::Layout {
            bg_color: Some(Color::red()),
        }
    );

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Header::new(32.0)));
    let root_id = tree.root_id().expect("header root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Header>()
        .unwrap() as *const Header;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Header::new(48.0).bg(Color::blue())),
    );

    let header = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Header>()
        .unwrap();
    assert_eq!(header as *const Header, before_ptr);
    assert_eq!(
        header.snapshot_fields(),
        SnapshotFields::Header {
            height: 48.0,
            bg_color: Some(Color::blue()),
        }
    );

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Sider::new(160.0)));
    let root_id = tree.root_id().expect("sider root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Sider>()
        .unwrap() as *const Sider;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Sider::new(240.0)
                .bg(Color::green())
                .collapsible(true)
                .collapsed(true)
                .collapsed_width(72.0),
        ),
    );

    let sider = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Sider>()
        .unwrap();
    assert_eq!(sider as *const Sider, before_ptr);
    assert_eq!(
        sider.snapshot_fields(),
        SnapshotFields::Sider {
            width: 240.0,
            bg_color: Some(Color::green()),
            collapsible: true,
            collapsed: true,
            collapsed_width: 72.0,
        }
    );

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Content::new()));
    let root_id = tree.root_id().expect("content root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Content>()
        .unwrap() as *const Content;

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Content::new().bg(Color::red())));

    let content = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Content>()
        .unwrap();
    assert_eq!(content as *const Content, before_ptr);
    assert_eq!(
        content.snapshot_fields(),
        SnapshotFields::Content {
            bg_color: Some(Color::red()),
        }
    );

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Footer::new(24.0)));
    let root_id = tree.root_id().expect("footer root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Footer>()
        .unwrap() as *const Footer;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Footer::new(56.0).bg(Color::blue())),
    );

    let footer = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Footer>()
        .unwrap();
    assert_eq!(footer as *const Footer, before_ptr);
    assert_eq!(
        footer.snapshot_fields(),
        SnapshotFields::Footer {
            height: 56.0,
            bg_color: Some(Color::blue()),
        }
    );
}

#[test]
fn reconcile_bar_chart_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{BarChart, BarData};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(BarChart::new()));
    let root_id = tree.root_id().expect("bar chart root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<BarChart>()
        .unwrap() as *const BarChart;
    let data = vec![
        BarData::new("A", 3.0, Color::red()),
        BarData::new("B", 8.0, Color::blue()),
    ];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            BarChart::new()
                .data(data.clone())
                .width(260.0)
                .height(180.0)
                .max_value(10.0)
                .show_value(false)
                .bar_radius(6.0),
        ),
    );

    let chart = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<BarChart>()
        .unwrap();
    assert_eq!(chart as *const BarChart, before_ptr);
    assert_eq!(
        chart.snapshot_fields(),
        SnapshotFields::BarChart {
            data,
            fixed_width: 260.0,
            fixed_height: 180.0,
            max_value: 10.0,
            show_value: false,
            bar_radius: 6.0,
        }
    );
}

#[test]
fn reconcile_line_chart_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{LineChart, LineData};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(LineChart::new()));
    let root_id = tree.root_id().expect("line chart root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<LineChart>()
        .unwrap() as *const LineChart;
    let data = vec![LineData::new("Mon", 2.0), LineData::new("Tue", 5.0)];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            LineChart::new()
                .data(data.clone())
                .width(320.0)
                .height(140.0)
                .line_color(Color::green())
                .max_value(8.0)
                .auto_min(true)
                .show_grid(false)
                .show_dots(false)
                .line_width(4.0),
        ),
    );

    let chart = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<LineChart>()
        .unwrap();
    assert_eq!(chart as *const LineChart, before_ptr);
    assert_eq!(
        chart.snapshot_fields(),
        SnapshotFields::LineChart {
            data,
            fixed_width: 320.0,
            fixed_height: 140.0,
            line_color: Some(Color::green()),
            max_value: 8.0,
            auto_min: true,
            show_grid: false,
            show_dots: false,
            line_width: 4.0,
            dot_radius: 3.0,
        }
    );
}

#[test]
fn reconcile_pie_chart_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{PieChart, PieData};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(PieChart::new()));
    let root_id = tree.root_id().expect("pie chart root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<PieChart>()
        .unwrap() as *const PieChart;
    let data = vec![
        PieData::new("Used", 70.0, Color::red()),
        PieData::new("Free", 30.0, Color::green()),
    ];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(PieChart::new().data(data.clone()).size(220.0).donut(0.45)),
    );

    let chart = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<PieChart>()
        .unwrap();
    assert_eq!(chart as *const PieChart, before_ptr);
    assert_eq!(
        chart.snapshot_fields(),
        SnapshotFields::PieChart {
            data,
            fixed_size: 220.0,
            hole_radius: 0.45,
        }
    );
}

#[test]
fn reconcile_qrcode_patches_instance_and_syncs_config() {
    use crate::ui::widgets::QRCode;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(QRCode::new("old")));
    let root_id = tree.root_id().expect("qrcode root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<QRCode>()
        .unwrap() as *const QRCode;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(QRCode::new("new").size(96.0).error_level(0)),
    );

    let qrcode = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<QRCode>()
        .unwrap();
    assert_eq!(qrcode as *const QRCode, before_ptr);
    assert_eq!(
        qrcode.snapshot_fields(),
        SnapshotFields::QRCode {
            value: "new".to_string(),
            size: 96.0,
            error_level: 0,
        }
    );
}

#[test]
fn reconcile_watermark_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Watermark;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Watermark::new("old")));
    let root_id = tree.root_id().expect("watermark root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Watermark>()
        .unwrap() as *const Watermark;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Watermark::new("new")
                .color(Color::blue())
                .font_size(18.0)
                .opacity(0.3)
                .rotate(-12.0)
                .gap(120.0, 90.0)
                .offset(8.0, 16.0),
        ),
    );

    let watermark = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Watermark>()
        .unwrap();
    assert_eq!(watermark as *const Watermark, before_ptr);
    assert_eq!(
        watermark.snapshot_fields(),
        SnapshotFields::Watermark {
            text: "new".to_string(),
            color: Color::blue(),
            font_size: 18.0,
            opacity: 0.3,
            rotate: -12.0,
            gap_x: 120.0,
            gap_y: 90.0,
            x_offset: 8.0,
            y_offset: 16.0,
        }
    );
}

#[test]
fn reconcile_affix_preserves_position_state_and_syncs_offset() {
    use crate::core::Size;
    use crate::ui::traits::WidgetLayout;
    use crate::ui::widgets::Affix;

    let mut affix = Affix::new(10.0);
    affix.set_child_bounds(100.0, 36.0);
    affix.update_scroll(24.0);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(affix));
    let root_id = tree.root_id().expect("affix root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Affix>()
        .unwrap() as *const Affix;

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Affix::new(40.0)));

    let affix = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Affix>()
        .unwrap();
    assert_eq!(affix as *const Affix, before_ptr);
    assert!(affix.is_affixed());
    assert_eq!(affix.child_y(), 40.0);
    assert_eq!(
        affix.measure(crate::core::Constraints::loose(Size::new(120.0, 80.0))),
        Size::new(0.0, 36.0)
    );
    assert_eq!(
        affix.snapshot_fields(),
        SnapshotFields::Affix { offset_top: 40.0 }
    );
}

#[test]
fn reconcile_breadcrumb_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{Breadcrumb, BreadcrumbItem};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Breadcrumb::new()));
    let root_id = tree.root_id().expect("breadcrumb root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Breadcrumb>()
        .unwrap() as *const Breadcrumb;
    let items = vec![
        BreadcrumbItem::new("Home"),
        BreadcrumbItem::new("Docs").active(),
    ];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Breadcrumb::new().items(items.clone()).separator(">")),
    );

    let breadcrumb = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Breadcrumb>()
        .unwrap();
    assert_eq!(breadcrumb as *const Breadcrumb, before_ptr);
    assert_eq!(
        breadcrumb.snapshot_fields(),
        SnapshotFields::Breadcrumb {
            items,
            separator: ">".to_string(),
        }
    );
}

#[test]
fn reconcile_timeline_patches_instance_and_syncs_config() {
    use crate::ui::widgets::{Timeline, TimelineItem};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Timeline::new()));
    let root_id = tree.root_id().expect("timeline root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Timeline>()
        .unwrap() as *const Timeline;
    let items = vec![
        TimelineItem::new("Started").description("alpha"),
        TimelineItem::new("Done").color(Color::green()),
    ];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Timeline::new()
                .items(items.clone())
                .pending(true)
                .reverse(true),
        ),
    );

    let timeline = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Timeline>()
        .unwrap();
    assert_eq!(timeline as *const Timeline, before_ptr);
    assert_eq!(
        timeline.snapshot_fields(),
        SnapshotFields::Timeline {
            items,
            pending: true,
            reverse: true,
        }
    );
}

#[test]
fn reconcile_message_preserves_queue_and_syncs_placement() {
    use crate::ui::widgets::{Message, MessagePlacement};

    let message = Message::new();
    message.success("kept");
    let queue = message.queue();
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(message));
    let root_id = tree.root_id().expect("message root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Message>()
        .unwrap() as *const Message;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Message::new().placement(MessagePlacement::TopRight)),
    );

    let message = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Message>()
        .unwrap();
    assert_eq!(message as *const Message, before_ptr);
    assert_eq!(queue.borrow().len(), 1);
    assert_eq!(queue.borrow()[0].content, "kept");
    assert_eq!(
        message.snapshot_fields(),
        SnapshotFields::Message {
            placement: MessagePlacement::TopRight,
        }
    );
}

#[test]
fn reconcile_notification_preserves_queue_and_syncs_placement() {
    use crate::native::traits::system::StatusLevel;
    use crate::ui::widgets::{NotifPlacement, Notification};

    let notification = Notification::new();
    notification.open("kept", "body", StatusLevel::Info);
    let queue = notification.queue();
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(notification));
    let root_id = tree.root_id().expect("notification root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Notification>()
        .unwrap() as *const Notification;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Notification::new().placement(NotifPlacement::BottomLeft)),
    );

    let notification = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Notification>()
        .unwrap();
    assert_eq!(notification as *const Notification, before_ptr);
    assert_eq!(queue.borrow().len(), 1);
    assert_eq!(queue.borrow()[0].title, "kept");
    assert_eq!(
        notification.snapshot_fields(),
        SnapshotFields::Notification {
            placement: NotifPlacement::BottomLeft,
        }
    );
}

#[test]
fn reconcile_collapse_preserves_expanded_state_and_syncs_config() {
    use crate::core::{Point, Size};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::traits::WidgetLayout;
    use crate::ui::widgets::{Collapse, CollapsePanel};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Collapse::new().panels(vec![CollapsePanel::new("Old", "old")]),
    ));
    let root_id = tree.root_id().expect("collapse root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Collapse>()
        .unwrap() as *const Collapse;

    assert_eq!(
        crate::ui::EventHandler::on_event(
            tree.get_mut(root_id)
                .unwrap()
                .component_mut()
                .as_any_mut()
                .downcast_mut::<Collapse>()
                .unwrap(),
            &SystemEvent::PointerDown {
                pos: Point::new(1.0, 1.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Collapse::new()
                .panels(vec![CollapsePanel::new("New", "new body")])
                .accordion(),
        ),
    );

    let collapse = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Collapse>()
        .unwrap();
    assert_eq!(collapse as *const Collapse, before_ptr);
    assert_eq!(
        collapse.measure(crate::core::Constraints::loose(Size::new(200.0, 200.0))),
        Size::new(0.0, 70.0)
    );
    assert!(matches!(
        collapse.snapshot_fields(),
        SnapshotFields::Collapse { panels, accordion: true }
            if panels.len() == 1 && panels[0].header == "New" && panels[0].content == "new body"
    ));
}

#[test]
fn reconcile_carousel_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Carousel;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Carousel::new()));
    let root_id = tree.root_id().expect("carousel root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Carousel>()
        .unwrap() as *const Carousel;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Carousel::new().show_dots(false).show_arrows(false)),
    );

    let carousel = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Carousel>()
        .unwrap();
    assert_eq!(carousel as *const Carousel, before_ptr);
    assert_eq!(carousel.current_index(), 0);
    assert_eq!(
        carousel.snapshot_fields(),
        SnapshotFields::Carousel {
            show_dots: false,
            show_arrows: false,
        }
    );
}

#[test]
fn reconcile_tree_preserves_selection_and_syncs_nodes() {
    use crate::core::{Point, Size};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::traits::WidgetLayout;
    use crate::ui::widgets::{Tree, TreeNode};

    let nodes = vec![TreeNode::new("Root", "root").children(vec![TreeNode::new("Child", "child")])];
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Tree::new(nodes)));
    let root_id = tree.root_id().expect("tree root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Tree>()
        .unwrap() as *const Tree;
    {
        let tree_widget = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Tree>()
            .unwrap();
        tree_widget.set_selected_key("child");
        assert_eq!(
            crate::ui::EventHandler::on_event(
                tree_widget,
                &SystemEvent::PointerDown {
                    pos: Point::new(25.0, 1.0),
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                },
            ),
            EventResult::Handled
        );
    }

    let next_nodes =
        vec![TreeNode::new("Root 2", "root").children(vec![TreeNode::new("Child 2", "child")])];
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Tree::new(next_nodes).multiple(true)),
    );

    let tree_widget = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Tree>()
        .unwrap();
    assert_eq!(tree_widget as *const Tree, before_ptr);
    assert_eq!(tree_widget.selected_key(), "child");
    assert_eq!(
        tree_widget.measure(crate::core::Constraints::loose(Size::new(300.0, 100.0))),
        Size::new(200.0, 56.0)
    );
    assert!(matches!(
        tree_widget.snapshot_fields(),
        SnapshotFields::Tree { nodes, multiple: true }
            if nodes.len() == 1 && nodes[0].title == "Root 2" && nodes[0].children[0].title == "Child 2"
    ));
}

#[test]
fn reconcile_splitter_patches_instance_and_syncs_config() {
    use crate::ui::widgets::Splitter;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Splitter::new()));
    let root_id = tree.root_id().expect("splitter root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Splitter>()
        .unwrap() as *const Splitter;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Splitter::new().panels(3).vertical(true).min_size(1, 80.0)),
    );

    let splitter = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Splitter>()
        .unwrap();
    assert_eq!(splitter as *const Splitter, before_ptr);
    assert_eq!(
        splitter.snapshot_fields(),
        SnapshotFields::Splitter {
            vertical: true,
            panel_count: 3,
            min_sizes: vec![50.0, 80.0, 50.0],
            handle_size: 6.0,
        }
    );
}

#[test]
fn reconcile_selectable_list_preserves_active_index_and_syncs_config() {
    use crate::ui::widgets::{SelectableItem, SelectableList};

    let mut list = SelectableList::new();
    list.items = vec![SelectableItem::new("a", "A"), SelectableItem::new("b", "B")];
    list.active_index = 1;
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(list));
    let root_id = tree.root_id().expect("selectable list root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SelectableList>()
        .unwrap() as *const SelectableList;

    let mut next = SelectableList::new();
    next.items = vec![SelectableItem::new("c", "C").icon("*")];
    next.header_button_text = "Add".to_string();
    next.footer_text = "Footer".to_string();
    next.item_height = 44.0;
    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(next));

    let list = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<SelectableList>()
        .unwrap();
    assert_eq!(list as *const SelectableList, before_ptr);
    assert_eq!(list.active_index, 0);
    assert_eq!(
        list.snapshot_fields(),
        SnapshotFields::SelectableList {
            items: vec![SelectableItem::new("c", "C").icon("*")],
            header_button_text: "Add".to_string(),
            footer_text: "Footer".to_string(),
            item_height: 44.0,
        }
    );
}

#[test]
fn reconcile_theme_toggle_preserves_current_dark_state_and_syncs_initial() {
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::widgets::ThemeToggle;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(ThemeToggle::new()));
    let root_id = tree.root_id().expect("theme toggle root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ThemeToggle>()
        .unwrap() as *const ThemeToggle;
    assert_eq!(
        crate::ui::EventHandler::on_event(
            tree.get_mut(root_id)
                .unwrap()
                .component_mut()
                .as_any_mut()
                .downcast_mut::<ThemeToggle>()
                .unwrap(),
            &SystemEvent::PointerDown {
                pos: crate::core::Point::new(1.0, 1.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(ThemeToggle::new().dark(false)));

    let theme_toggle = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ThemeToggle>()
        .unwrap();
    assert_eq!(theme_toggle as *const ThemeToggle, before_ptr);
    assert!(theme_toggle.is_dark());
    assert_eq!(
        theme_toggle.snapshot_fields(),
        SnapshotFields::ThemeToggle { dark: false }
    );
}

#[test]
fn reconcile_nav_item_preserves_shared_active_and_syncs_config() {
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::widgets::NavItem;
    use std::cell::Cell;
    use std::rc::Rc;

    let active = Rc::new(Cell::new(0));
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(NavItem::new("old", 1, active.clone())));
    let root_id = tree.root_id().expect("nav item root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<NavItem>()
        .unwrap() as *const NavItem;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            NavItem::new("new", 3, Rc::new(Cell::new(99)))
                .icon("home")
                .width(64.0)
                .height(40.0)
                .compact(true),
        ),
    );

    let nav_item = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<NavItem>()
        .unwrap();
    assert_eq!(nav_item as *const NavItem, before_ptr);
    assert_eq!(
        nav_item.snapshot_fields(),
        SnapshotFields::NavItem {
            label: "new".to_string(),
            icon: "home".to_string(),
            fixed_width: 64.0,
            fixed_height: 40.0,
            index: 3,
            compact: true,
        }
    );

    assert_eq!(
        crate::ui::EventHandler::on_event(
            tree.get_mut(root_id)
                .unwrap()
                .component_mut()
                .as_any_mut()
                .downcast_mut::<NavItem>()
                .unwrap(),
            &SystemEvent::PointerDown {
                pos: crate::core::Point::new(1.0, 1.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );
    assert_eq!(active.get(), 3);
}

#[test]
fn reconcile_rich_text_clears_stale_layout_and_syncs_config() {
    use crate::ui::widgets::{RichText, RichTextSegment, RichTextStyle};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(RichText::new().content(vec![
        RichTextSegment::Text {
            content: "old".to_string(),
            style: RichTextStyle::default(),
        },
    ])));
    let root_id = tree.root_id().expect("rich text root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<RichText>()
        .unwrap() as *const RichText;
    let segments = vec![RichTextSegment::Link {
        content: "docs".to_string(),
        url: "https://example.test".to_string(),
    }];

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            RichText::new()
                .content(segments.clone())
                .font_size(18.0)
                .color(Color::green()),
        ),
    );

    let rich_text = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<RichText>()
        .unwrap();
    assert_eq!(rich_text as *const RichText, before_ptr);
    assert_eq!(rich_text.selected_text(), None);
    assert_eq!(
        rich_text.snapshot_fields(),
        SnapshotFields::RichText {
            segments,
            default_font_size: 18.0,
            default_font_size_unit: None,
            default_color: Color::green(),
        }
    );
}

#[test]
fn reconcile_transfer_preserves_live_membership_and_syncs_initial_items() {
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::widgets::{Transfer, TransferItem};
    use crate::ui::SnapshotTransferItem;

    let transfer = Transfer::new().source(vec![TransferItem {
        key: "a".to_string(),
        title: "A".to_string(),
        selected: false,
    }]);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(transfer));
    let root_id = tree.root_id().expect("transfer root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Transfer>()
        .unwrap() as *const Transfer;

    {
        let transfer = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Transfer>()
            .unwrap();
        assert_eq!(
            crate::ui::EventHandler::on_event(
                transfer,
                &SystemEvent::PointerDown {
                    pos: crate::core::Point::new(1.0, 1.0),
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                },
            ),
            EventResult::NotHandled
        );
        assert_eq!(
            crate::ui::EventHandler::on_event(
                transfer,
                &SystemEvent::PointerDown {
                    pos: crate::core::Point::new(240.0, 90.0),
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                },
            ),
            EventResult::Handled
        );
    }

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Transfer::new().source(vec![TransferItem {
            key: "b".to_string(),
            title: "B".to_string(),
            selected: false,
        }])),
    );

    let transfer = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Transfer>()
        .unwrap();
    assert_eq!(transfer as *const Transfer, before_ptr);
    assert_eq!(transfer.source_count(), 0);
    assert_eq!(transfer.target_count(), 1);
    assert_eq!(
        transfer.snapshot_fields(),
        SnapshotFields::Transfer {
            source: vec![SnapshotTransferItem {
                key: "b".to_string(),
                title: "B".to_string(),
            }],
            target: vec![],
        }
    );
}

#[test]
fn reconcile_upload_preserves_files_and_syncs_config() {
    use crate::ui::widgets::Upload;

    let mut upload = Upload::new();
    upload.add_file("kept.txt");
    upload.update_progress(0, 0.5);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(upload));
    let root_id = tree.root_id().expect("upload root should exist");
    let before_ptr = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Upload>()
        .unwrap() as *const Upload;

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Upload::new()
                .accept(".png")
                .multiple(true)
                .drag(false)
                .max_count(5),
        ),
    );

    let upload = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Upload>()
        .unwrap();
    assert_eq!(upload as *const Upload, before_ptr);
    assert_eq!(upload.file_count(), 1);
    assert_eq!(
        upload.snapshot_fields(),
        SnapshotFields::Upload {
            accept: ".png".to_string(),
            multiple: true,
            drag: false,
            max_count: 5,
        }
    );
}

#[test]
fn reconcile_same_type_tooltip_preserves_pending_timer_state() {
    use crate::ui::widgets::{Tooltip, TooltipPlacement};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Tooltip::new("old").delay_ms(120).timer_id(7),
    ));
    let root_id = tree.root_id().expect("tooltip root should exist");
    assert_eq!(
        crate::ui::EventHandler::on_event(
            tree.get_mut(root_id)
                .unwrap()
                .component_mut()
                .as_any_mut()
                .downcast_mut::<Tooltip>()
                .unwrap(),
            &SystemEvent::PointerEnter,
        ),
        EventResult::Handled
    );
    assert_eq!(
        tree.get(root_id).unwrap().active_timer(),
        Some((7, std::time::Duration::from_millis(120)))
    );

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Tooltip::new("new")
                .placement(TooltipPlacement::Bottom)
                .delay_ms(250)
                .timer_id(9),
        ),
    );

    assert_eq!(
        tree.get(root_id).unwrap().active_timer(),
        Some((9, std::time::Duration::from_millis(250)))
    );
    assert!(matches!(
        tree.get(root_id).unwrap().component().snapshot_fields(),
        SnapshotFields::Tooltip {
            text,
            placement: TooltipPlacement::Bottom,
            delay_ms: 250,
            timer_id: 9,
            ..
        } if text == "new"
    ));
}

#[test]
fn reconcile_popover_preserves_visibility_and_syncs_config() {
    use crate::ui::widgets::{Popover, PopoverPlacement, PopoverTrigger};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Popover::new("old").title("old title")));
    let root_id = tree.root_id().expect("popover root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Popover>()
        .unwrap()
        .open();

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Popover::new("new")
                .title("new title")
                .placement(PopoverPlacement::BottomRight)
                .trigger(PopoverTrigger::Hover)
                .arrow(false),
        ),
    );

    let popover = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Popover>()
        .unwrap();
    assert!(popover.is_visible());
    assert!(matches!(
        popover.snapshot_fields(),
        SnapshotFields::Popover {
            title,
            content,
            placement: PopoverPlacement::BottomRight,
            trigger: PopoverTrigger::Hover,
            arrow: false,
        } if title == "new title" && content == "new"
    ));
}

#[test]
fn reconcile_popconfirm_preserves_visibility_and_syncs_config() {
    use crate::ui::widgets::{Popconfirm, PopconfirmPlacement};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Popconfirm::new().title("old")));
    let root_id = tree.root_id().expect("popconfirm root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Popconfirm>()
        .unwrap()
        .open();

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Popconfirm::new()
                .title("new")
                .confirm_text("Yes")
                .cancel_text("No")
                .placement(PopconfirmPlacement::BottomRight)
                .arrow(false)
                .icon(false),
        ),
    );

    let popconfirm = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Popconfirm>()
        .unwrap();
    assert!(popconfirm.is_visible());
    assert!(matches!(
        popconfirm.snapshot_fields(),
        SnapshotFields::Popconfirm {
            title,
            confirm_text,
            cancel_text,
            placement: PopconfirmPlacement::BottomRight,
            arrow: false,
            icon: false,
        } if title == "new" && confirm_text == "Yes" && cancel_text == "No"
    ));
}

#[test]
fn reconcile_modal_preserves_present_state_and_syncs_config() {
    use crate::native::traits::input::ControlSize;
    use crate::ui::widgets::Modal;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Modal::new("old").show()));
    let root_id = tree.root_id().expect("modal root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Modal>()
        .unwrap()
        .close();

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Modal::new("new")
                .modal_size(ControlSize::Large)
                .size(640.0, 360.0)
                .closable(false)
                .mask_closable(false)
                .footer_visible(false)
                .centered(false)
                .overlay(true),
        ),
    );

    let modal = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Modal>()
        .unwrap();
    assert!(modal.is_present());
    assert!(matches!(
        modal.snapshot_fields(),
        SnapshotFields::Modal {
            title,
            width: 640.0,
            height: 360.0,
            modal_size: ControlSize::Large,
            closable: false,
            mask_closable: false,
            footer_visible: false,
            centered: false,
            overlay: true,
        } if title == "new"
    ));
}

#[test]
fn reconcile_drawer_preserves_present_state_and_syncs_config() {
    use crate::native::traits::input::ControlSize;
    use crate::ui::widgets::{Drawer, DrawerPlacement};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Drawer::new("old").show()));
    let root_id = tree.root_id().expect("drawer root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Drawer>()
        .unwrap()
        .close();

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Drawer::new("new")
                .drawer_size(ControlSize::Large)
                .size(680.0, 460.0)
                .placement(DrawerPlacement::Left)
                .closable(false)
                .mask_closable(false)
                .mask(false)
                .footer_visible(true)
                .extra("extra"),
        ),
    );

    let drawer = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Drawer>()
        .unwrap();
    assert!(drawer.is_present());
    assert!(matches!(
        drawer.snapshot_fields(),
        SnapshotFields::Drawer {
            title,
            width: 680.0,
            height: 460.0,
            drawer_size: ControlSize::Large,
            placement: DrawerPlacement::Left,
            closable: false,
            mask_closable: false,
            mask: false,
            footer_visible: true,
            extra,
        } if title == "new" && extra == "extra"
    ));
}

#[test]
fn reconcile_dropdown_preserves_open_state_and_syncs_items() {
    use crate::ui::widgets::Dropdown;

    let mut tree =
        ViewAdapter::build_nodes(ViewNode::leaf(Dropdown::new("old").items(vec!["A", "B"])));
    let root_id = tree.root_id().expect("dropdown root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Dropdown>()
        .unwrap()
        .open();

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Dropdown::new("new").items(vec!["C", "D", "E"])),
    );

    let dropdown = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Dropdown>()
        .unwrap();
    assert!(dropdown.is_open());
    assert!(matches!(
        dropdown.snapshot_fields(),
        SnapshotFields::Dropdown { label, items } if label == "new" && items == vec!["C", "D", "E"]
    ));
}

#[test]
fn reconcile_menu_preserves_active_key_and_syncs_items() {
    use crate::ui::widgets::{Menu, MenuItem, MenuMode};

    let old_items = vec![
        MenuItem {
            key: "a".to_string(),
            label: "A".to_string(),
            icon: String::new(),
            disabled: false,
        },
        MenuItem {
            key: "b".to_string(),
            label: "B".to_string(),
            icon: String::new(),
            disabled: false,
        },
    ];
    let new_items = vec![MenuItem {
        key: "c".to_string(),
        label: "C".to_string(),
        icon: "home".to_string(),
        disabled: true,
    }];
    let mut tree =
        ViewAdapter::build_nodes(ViewNode::leaf(Menu::new().items(old_items).active_key("a")));
    let root_id = tree.root_id().expect("menu root should exist");
    tree.get_mut(root_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Menu>()
        .unwrap()
        .set_active_key("b");

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Menu::new()
                .items(new_items)
                .mode(MenuMode::Vertical)
                .item_height(44.0),
        ),
    );

    let menu = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Menu>()
        .unwrap();
    assert_eq!(menu.get_active_key(), "b");
    assert!(matches!(
        menu.snapshot_fields(),
        SnapshotFields::Menu {
            items,
            mode: MenuMode::Vertical,
            item_h: 44.0,
        } if items.len() == 1 && items[0].key == "c" && items[0].disabled
    ));
}

#[test]
fn reconcile_tabs_preserves_active_index_and_syncs_tabs() {
    use crate::ui::widgets::{Tab, TabPosition, Tabs};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Tabs::new().tab("One", "one").tab("Two", "two").active(1),
    ));
    let root_id = tree.root_id().expect("tabs root should exist");

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Tabs::new()
                .tabs(vec![
                    Tab {
                        label: "First".to_string(),
                        key: "first".to_string(),
                    },
                    Tab {
                        label: "Second".to_string(),
                        key: "second".to_string(),
                    },
                    Tab {
                        label: "Third".to_string(),
                        key: "third".to_string(),
                    },
                ])
                .position(TabPosition::Bottom)
                .size(520.0, 260.0),
        ),
    );

    let tabs = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Tabs>()
        .unwrap();
    assert_eq!(tabs.active_index(), 1);
    assert!(matches!(
        tabs.snapshot_fields(),
        SnapshotFields::Tabs {
            tabs,
            position: TabPosition::Bottom,
            fixed_width: Some(520.0),
            fixed_height: Some(260.0),
            ..
        } if tabs.len() == 3 && tabs[1].key == "second"
    ));
}

#[test]
fn reconcile_pagination_preserves_current_page_and_syncs_config() {
    use crate::ui::widgets::Pagination;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Pagination::new(100, 10).current(2)));
    let root_id = tree.root_id().expect("pagination root should exist");
    tree.get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Pagination>()
        .unwrap()
        .set_current(4);

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Pagination::new(240, 20)
                .show_size_changer(true)
                .show_total(false)
                .item_size(36.0)
                .page_size_options(vec![10, 20, 40]),
        ),
    );

    let pagination = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Pagination>()
        .unwrap();
    assert_eq!(pagination.get_current(), 4);
    assert_eq!(pagination.total_pages(), 12);
    assert!(matches!(
        pagination.snapshot_fields(),
        SnapshotFields::Pagination {
            total: 240,
            page_size: 20,
            show_size_changer: true,
            show_total: false,
            size: 36.0,
            page_size_options,
        } if page_size_options == vec![10, 20, 40]
    ));
}

#[test]
fn reconcile_anchor_preserves_active_index_and_syncs_items() {
    use crate::ui::widgets::{Anchor, AnchorItem};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Anchor::new(vec![
        AnchorItem::new("A", "#a"),
        AnchorItem::new("B", "#b"),
    ])));
    let root_id = tree.root_id().expect("anchor root should exist");
    {
        let anchor = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Anchor>()
            .unwrap();
        anchor.set_positions(vec![0.0, 100.0]);
        anchor.update_active(150.0);
        assert_eq!(anchor.active_index(), 1);
    }

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Anchor::new(vec![
                AnchorItem::new("First", "#first"),
                AnchorItem::new("Second", "#second"),
            ])
            .set_offset_top(24.0)
            .bg(Color::red()),
        ),
    );

    let anchor = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Anchor>()
        .unwrap();
    assert_eq!(anchor.active_index(), 1);
    assert_eq!(anchor.active_href(), "#second");
    assert!(matches!(
        anchor.snapshot_fields(),
        SnapshotFields::Anchor {
            items,
            offset_top: 24.0,
            bg_color: Some(bg),
        } if items.len() == 2 && items[1].label == "Second" && bg == Color::red()
    ));
}

#[test]
fn reconcile_steps_preserves_current_step_and_syncs_steps() {
    use crate::ui::widgets::{Step, StepStatus, Steps};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Steps::new(vec![Step::new("One"), Step::new("Two")]).current(1),
    ));
    let root_id = tree.root_id().expect("steps root should exist");
    tree.get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Steps>()
        .unwrap()
        .set_current(1);

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Steps::new(vec![
            Step::new("Start").status(StepStatus::Finish),
            Step::new("Middle").description("in progress"),
            Step::new("Done"),
        ])),
    );

    let steps = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Steps>()
        .unwrap();
    assert_eq!(steps.get_current(), 1);
    assert_eq!(steps.step_count(), 3);
    assert!(matches!(
        steps.snapshot_fields(),
        SnapshotFields::Steps { steps, direction: true }
            if steps.len() == 3
                && steps[0].status == StepStatus::Finish
                && steps[1].description == "in progress"
    ));
}

#[test]
fn reconcile_divider_syncs_config() {
    use crate::ui::widgets::{Divider, DividerDirection, DividerOrientation};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Divider::new()));
    let root_id = tree.root_id().expect("divider root should exist");

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Divider::new()
                .with_text("Section")
                .orientation(DividerOrientation::Right)
                .vertical()
                .color(Color::red())
                .dashed(),
        ),
    );

    assert!(matches!(
        tree.get(root_id).unwrap().component().snapshot_fields(),
        SnapshotFields::Divider {
            text: Some(text),
            orientation: DividerOrientation::Right,
            direction: DividerDirection::Vertical,
            color: Some(color),
            dashed: true,
            ..
        } if text == "Section" && color == Color::red()
    ));
}

#[test]
fn reconcile_icon_syncs_name_and_size() {
    use crate::ui::widgets::Icon;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Icon::new("search").size(16.0)));
    let root_id = tree.root_id().expect("icon root should exist");

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Icon::new("settings").size(28.0)));

    assert!(matches!(
        tree.get(root_id).unwrap().component().snapshot_fields(),
        SnapshotFields::Icon { name, size: 28.0 } if name == "settings"
    ));
}

#[test]
fn reconcile_typography_syncs_text_and_flags() {
    use crate::ui::widgets::{Typography, TypographyType};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Typography::text("old")));
    let root_id = tree.root_id().expect("typography root should exist");

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Typography::heading("new", 2)
                .disabled(true)
                .mark()
                .code()
                .underline()
                .delete()
                .strong()
                .italic()
                .copyable(true)
                .color(Color::green()),
        ),
    );

    assert!(matches!(
        tree.get(root_id).unwrap().component().snapshot_fields(),
        SnapshotFields::Typography {
            content,
            type_: TypographyType::Heading2,
            disabled: true,
            mark: true,
            code: true,
            underline: true,
            delete: true,
            strong: true,
            italic: true,
            copyable: true,
            color_override: Some(color),
        } if content == "new" && color == Color::green()
    ));
}

#[test]
fn reconcile_avatar_syncs_visual_config() {
    use crate::ui::widgets::Avatar;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Avatar::new("A")));
    let root_id = tree.root_id().expect("avatar root should exist");

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Avatar::new("B")
                .size(48.0)
                .bg(Color::blue())
                .text_color(Color::white())
                .square(true)
                .src("avatar.png"),
        ),
    );

    assert!(matches!(
        tree.get(root_id).unwrap().component().snapshot_fields(),
        SnapshotFields::Avatar {
            text,
            size: 48.0,
            bg_color: Some(bg),
            text_color: Some(fg),
            square: true,
            src,
        } if text == "B" && bg == Color::blue() && fg == Color::white() && src == "avatar.png"
    ));
}

#[test]
fn reconcile_badge_syncs_count_status_and_offsets() {
    use crate::draw::spatial::PhysicalUnit;
    use crate::ui::widgets::{Badge, BadgeStatus};

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Badge::new().count(1)));
    let root_id = tree.root_id().expect("badge root should exist");

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Badge::new()
                .count(120)
                .max(88)
                .dot()
                .color(Color::green())
                .status(BadgeStatus::Warning)
                .show_zero(true)
                .text("warn")
                .offset(4.0, 6.0)
                .offset_unit(PhysicalUnit::Mm(2.0), PhysicalUnit::Pt(3.0)),
        ),
    );

    assert!(matches!(
        tree.get(root_id).unwrap().component().snapshot_fields(),
        SnapshotFields::Badge {
            count: 1,
            max: 88,
            dot: true,
            color: Some(color),
            status: Some(BadgeStatus::Warning),
            show_zero: true,
            text,
            offset_x: 4.0,
            offset_y: 6.0,
            offset_unit: Some((PhysicalUnit::Mm(2.0), PhysicalUnit::Pt(3.0))),
            ..
        } if color == Color::green() && text == "warn"
    ));
}

#[test]
fn reconcile_same_type_scroll_view_preserves_offset() {
    use crate::native::traits::input::ScrollDirection;
    use crate::ui::widgets::ScrollView;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        ScrollView::new(ScrollDirection::Vertical)
            .size(300.0, 200.0)
            .scroll_to(0.0, 40.0)
            .show_scrollbar(true),
    ));
    let root_id = tree.root_id().expect("scroll root should exist");

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            ScrollView::new(ScrollDirection::Both)
                .size(320.0, 240.0)
                .show_scrollbar(false),
        ),
    );

    let scroll = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ScrollView>()
        .unwrap();
    assert_eq!(scroll.scroll_y(), 40.0);
    assert!(matches!(
        scroll.snapshot_fields(),
        SnapshotFields::ScrollView {
            direction: ScrollDirection::Both,
            fixed_width: Some(320.0),
            fixed_height: Some(240.0),
            show_scrollbar: false,
            ..
        }
    ));
}

#[test]
fn reconcile_tree_select_preserves_open_selection_and_syncs_options() {
    use crate::ui::widgets::{TreeNode, TreeSelect};

    let old_nodes = vec![TreeNode::new("Old", "old")];
    let new_nodes = vec![TreeNode::new("New", "new")];
    let mut tree =
        ViewAdapter::build_nodes(ViewNode::leaf(TreeSelect::new().nodes(old_nodes.clone())));
    let root_id = tree.root_id().expect("tree select root should exist");
    {
        let tree_select = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<TreeSelect>()
            .unwrap();
        tree_select.open();
        assert_eq!(
            crate::ui::EventHandler::on_event(
                tree_select,
                &SystemEvent::PointerDown {
                    pos: crate::core::Point::new(4.0, 36.0),
                    button: crate::ui::MouseButton::Left,
                    mods: crate::ui::KeyMod::NONE,
                },
            ),
            EventResult::Handled
        );
        tree_select.open();
        assert_eq!(tree_select.value_key(), "old");
    }

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            TreeSelect::new()
                .placeholder("new placeholder")
                .nodes(new_nodes),
        ),
    );

    let tree_select = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<TreeSelect>()
        .unwrap();
    assert!(tree_select.is_open());
    assert_eq!(tree_select.value_key(), "old");
    assert!(matches!(
        tree_select.snapshot_fields(),
        SnapshotFields::TreeSelect {
            placeholder,
            nodes,
        } if placeholder == "new placeholder" && nodes.len() == 1 && nodes[0].key == "new"
    ));
}

#[test]
fn reconcile_cascader_preserves_popup_state_and_syncs_options() {
    use crate::ui::widgets::{Cascader, CascaderOption};

    let old_options = vec![
        CascaderOption::new("Old", "old").children(vec![CascaderOption::new("Child", "child")])
    ];
    let new_options = vec![CascaderOption::new("New", "new")];
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Cascader::new(old_options, "old")));
    let root_id = tree.root_id().expect("cascader root should exist");
    {
        let cascader = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Cascader>()
            .unwrap();
        cascader.open();
        cascader.select_option(0, 0);
        assert!(cascader.is_open());
        assert_eq!(cascader.selected().values.as_slice(), &["old"]);
    }

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Cascader::new(new_options, "new placeholder")),
    );

    let cascader = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Cascader>()
        .unwrap();
    assert!(cascader.is_open());
    assert_eq!(cascader.selected().values.as_slice(), &["old"]);
    assert!(matches!(
        cascader.snapshot_fields(),
        SnapshotFields::Cascader {
            placeholder,
            options,
        } if placeholder == "new placeholder" && options.len() == 1 && options[0].value == "new"
    ));
}

#[test]
fn reconcile_color_picker_preserves_open_color_and_syncs_presets() {
    use crate::ui::widgets::ColorPicker;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(ColorPicker::new(Color::red())));
    let root_id = tree.root_id().expect("color picker root should exist");
    {
        let color_picker = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<ColorPicker>()
            .unwrap();
        color_picker.open();
        color_picker.set_value(Color::green());
    }

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(ColorPicker::new(Color::blue())));

    let color_picker = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ColorPicker>()
        .unwrap();
    assert!(color_picker.is_open());
    assert_eq!(color_picker.value(), Color::green());
    assert!(matches!(
        color_picker.snapshot_fields(),
        SnapshotFields::ColorPicker { preset_colors } if !preset_colors.is_empty()
    ));
}

#[test]
fn reconcile_autocomplete_preserves_open_value_and_syncs_options() {
    use crate::ui::widgets::AutoComplete;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        AutoComplete::new()
            .placeholder("old")
            .options(vec!["Apple", "Banana"]),
    ));
    let root_id = tree.root_id().expect("autocomplete root should exist");
    {
        let autocomplete = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<AutoComplete>()
            .unwrap();
        autocomplete.set_value("A");
        autocomplete.open();
    }

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            AutoComplete::new()
                .placeholder("new")
                .options(vec!["Apricot", "Avocado"]),
        ),
    );

    let autocomplete = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<AutoComplete>()
        .unwrap();
    assert!(autocomplete.is_open());
    assert_eq!(autocomplete.value(), "A");
    assert!(matches!(
        autocomplete.snapshot_fields(),
        SnapshotFields::AutoComplete {
            placeholder,
            options,
        } if placeholder == "new" && options == vec!["Apricot", "Avocado"]
    ));
}

#[test]
fn reconcile_date_picker_preserves_open_value_and_syncs_placeholder() {
    use crate::ui::widgets::{DatePicker, DateValue};

    let selected = DateValue::new(2026, 7, 7);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(DatePicker::new("old").value(selected)));
    let root_id = tree.root_id().expect("date picker root should exist");
    assert_eq!(
        crate::ui::EventHandler::on_event(
            tree.get_mut(root_id)
                .unwrap()
                .component_mut()
                .as_any_mut()
                .downcast_mut::<DatePicker>()
                .unwrap(),
            &SystemEvent::PointerDown {
                pos: crate::core::Point::new(4.0, 4.0),
                button: crate::ui::MouseButton::Left,
                mods: crate::ui::KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(DatePicker::new("new")));

    let date_picker = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<DatePicker>()
        .unwrap();
    assert!(date_picker.is_open());
    assert_eq!(date_picker.selected(), selected);
    assert!(matches!(
        date_picker.snapshot_fields(),
        SnapshotFields::DatePicker { placeholder } if placeholder == "new"
    ));
}

#[test]
fn reconcile_time_picker_preserves_open_value_and_syncs_placeholder() {
    use crate::ui::widgets::{TimePicker, TimeValue};

    let selected = TimeValue::new(9, 30);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(TimePicker::new("old").value(selected)));
    let root_id = tree.root_id().expect("time picker root should exist");
    assert_eq!(
        crate::ui::EventHandler::on_event(
            tree.get_mut(root_id)
                .unwrap()
                .component_mut()
                .as_any_mut()
                .downcast_mut::<TimePicker>()
                .unwrap(),
            &SystemEvent::PointerDown {
                pos: crate::core::Point::new(4.0, 4.0),
                button: crate::ui::MouseButton::Left,
                mods: crate::ui::KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(TimePicker::new("new")));

    let time_picker = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<TimePicker>()
        .unwrap();
    assert!(time_picker.is_open());
    assert_eq!(time_picker.selected(), selected);
    assert!(matches!(
        time_picker.snapshot_fields(),
        SnapshotFields::TimePicker { placeholder } if placeholder == "new"
    ));
}

#[test]
fn reconcile_mentions_preserves_suggestion_state_and_syncs_options() {
    use crate::ui::widgets::Mentions;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Mentions::new("old").options(vec!["Alice", "Bob"]),
    ));
    let root_id = tree.root_id().expect("mentions root should exist");
    {
        let mentions = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Mentions>()
            .unwrap();
        assert_eq!(
            crate::ui::EventHandler::on_event(
                mentions,
                &SystemEvent::TextInput {
                    text: "@".to_string(),
                },
            ),
            EventResult::Handled
        );
        assert_eq!(
            crate::ui::EventHandler::on_event(
                mentions,
                &SystemEvent::TextInput {
                    text: "a".to_string(),
                },
            ),
            EventResult::Handled
        );
    }

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Mentions::new("new").options(vec!["Ann", "Cara"])),
    );

    let mentions = tree
        .get_mut(root_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Mentions>()
        .unwrap();
    assert!(mentions.is_suggesting());
    assert_eq!(mentions.value(), "@");
    assert!(matches!(
        mentions.snapshot_fields(),
        SnapshotFields::Mentions {
            placeholder,
            options,
        } if placeholder == "new" && options == vec!["Ann", "Cara"]
    ));
    assert_eq!(
        crate::ui::EventHandler::on_event(
            mentions,
            &SystemEvent::KeyDown {
                key: crate::ui::KeyCode::Enter,
                mods: crate::ui::KeyMod::NONE,
            },
        ),
        EventResult::Handled
    );
    assert_eq!(mentions.value(), "@Ann ");
}

#[test]
fn reconcile_preserves_consumed_once_handler_when_signature_is_unchanged() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_generation(7),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_generation(7),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn reconcile_reregisters_plain_handler_without_stable_signature() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers = vec![HandlerRegistration::with_options(
        SemanticKind::Click,
        HandlerOptions::once(),
        {
            let first_hits = first_hits.clone();
            Box::new(move |_| first_hits.set(first_hits.get() + 1))
        },
    )];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers = vec![HandlerRegistration::with_options(
        SemanticKind::Click,
        HandlerOptions::once(),
        {
            let second_hits = second_hits.clone();
            Box::new(move |_| second_hits.set(second_hits.get() + 1))
        },
    )];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn reconcile_reregisters_handler_when_generation_changes() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_generation(7),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_generation(8),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn reconcile_preserves_consumed_once_handler_when_state_capture_fingerprint_is_unchanged() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_state_capture(&state),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    state.set(2);
    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_state_capture(&state),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn reconcile_reregisters_handler_when_state_capture_fingerprint_changes() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_state = State::new(1);
    let second_state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_state_capture(&first_state),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_state_capture(&second_state),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn reconcile_reregisters_handler_when_options_change_with_same_capture() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_state_capture(&state),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers = vec![HandlerRegistration::new(SemanticKind::Click, {
        let second_hits = second_hits.clone();
        Box::new(move |_| second_hits.set(second_hits.get() + 1))
    })
    .with_state_capture(&state)];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn reconcile_preserves_consumed_once_handler_when_window_capture_fingerprint_is_unchanged() {
    use crate::core::{Point, WindowId};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let window_id = WindowId::new(7);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_window_capture(window_id),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_window_capture(window_id),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn reconcile_reregisters_handler_when_window_capture_fingerprint_changes() {
    use crate::core::{Point, WindowId};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_window = WindowId::new(7);
    let second_window = WindowId::new(8);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_window_capture(first_window),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_window_capture(second_window),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn reconcile_reregisters_handler_when_one_of_multiple_captures_changes() {
    use crate::core::{Point, WindowId};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::event::{
        ClickEvent, HandlerOptions, HandlerRegistration, SemanticEvent, SemanticKind,
    };
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::view::View;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_state = State::new(1);
    let second_state = State::new(1);
    let window_id = WindowId::new(7);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let mut initial = button("First").build();
    initial.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let first_hits = first_hits.clone();
                Box::new(move |_| first_hits.set(first_hits.get() + 1))
            })
            .with_state_capture(&first_state)
            .with_window_capture(window_id),
        ];
    let mut tree = ViewAdapter::build_nodes(initial);
    let root_id = tree.root_id().unwrap();

    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(0.0, 0.0),
        modifiers: KeyMod::NONE,
    };
    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);

    let mut next = button("Second").build();
    next.handlers =
        vec![
            HandlerRegistration::with_options(SemanticKind::Click, HandlerOptions::once(), {
                let second_hits = second_hits.clone();
                Box::new(move |_| second_hits.set(second_hits.get() + 1))
            })
            .with_state_capture(&second_state)
            .with_window_capture(window_id),
        ];
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let _ = tree.dispatch_semantic(SemanticEvent::click(root_id, click));
    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn button_dsl_state_capture_preserves_handler_when_fingerprint_is_unchanged() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::state::State;
    use crate::ui::view::button;
    use crate::ui::SystemEvent;
    use std::cell::Cell;
    use std::rc::Rc;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree = ViewAdapter::build(button("First").on_click_capture(&state, move || {
        first_for_handler.set(first_for_handler.get() + 1);
    }));
    let pos = Point::new(4.0, 4.0);

    state.set(2);
    ViewAdapter::reconcile(
        &mut tree,
        button("Second").on_click_capture(&state, move || {
            second_for_handler.set(second_for_handler.get() + 1);
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

    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn button_dsl_window_capture_preserves_handler_when_fingerprint_is_unchanged() {
    use crate::core::{Point, WindowId};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::view::button;
    use crate::ui::SystemEvent;
    use std::cell::Cell;
    use std::rc::Rc;

    let window_id = WindowId::new(7);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree =
        ViewAdapter::build(button("First").on_click_window_capture(window_id, move || {
            first_for_handler.set(first_for_handler.get() + 1);
        }));
    let pos = Point::new(4.0, 4.0);

    ViewAdapter::reconcile(
        &mut tree,
        button("Second").on_click_window_capture(window_id, move || {
            second_for_handler.set(second_for_handler.get() + 1);
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

    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn input_dsl_state_capture_reregisters_handler_when_fingerprint_changes() {
    use crate::core::{Point, Rect};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::state::State;
    use crate::ui::view::input;
    use crate::ui::SystemEvent;
    use std::cell::RefCell;
    use std::rc::Rc;

    let first_state = State::new(1);
    let second_state = State::new(1);
    let first_value = Rc::new(RefCell::new(String::new()));
    let second_value = Rc::new(RefCell::new(String::new()));
    let first_for_handler = first_value.clone();
    let second_for_handler = second_value.clone();

    let mut tree = ViewAdapter::build(input().on_change_capture(&first_state, move |next| {
        *first_for_handler.borrow_mut() = next.to_string();
    }));
    let root = tree.root_id().unwrap();
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 120.0, 32.0));

    ViewAdapter::reconcile(
        &mut tree,
        input().on_change_capture(&second_state, move |next| {
            *second_for_handler.borrow_mut() = next.to_string();
        }),
    );

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::TextInput {
        text: "A".to_string(),
    });

    assert_eq!(&*first_value.borrow(), "");
    assert_eq!(&*second_value.borrow(), "A");
}

#[test]
fn input_dsl_window_capture_reregisters_handler_when_fingerprint_changes() {
    use crate::core::{Point, Rect, WindowId};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::view::input;
    use crate::ui::SystemEvent;
    use std::cell::RefCell;
    use std::rc::Rc;

    let first_window = WindowId::new(7);
    let second_window = WindowId::new(8);
    let first_value = Rc::new(RefCell::new(String::new()));
    let second_value = Rc::new(RefCell::new(String::new()));
    let first_for_handler = first_value.clone();
    let second_for_handler = second_value.clone();

    let mut tree =
        ViewAdapter::build(input().on_change_window_capture(first_window, move |next| {
            *first_for_handler.borrow_mut() = next.to_string();
        }));
    let root = tree.root_id().unwrap();
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 120.0, 32.0));

    ViewAdapter::reconcile(
        &mut tree,
        input().on_change_window_capture(second_window, move |next| {
            *second_for_handler.borrow_mut() = next.to_string();
        }),
    );

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::TextInput {
        text: "A".to_string(),
    });

    assert_eq!(&*first_value.borrow(), "");
    assert_eq!(&*second_value.borrow(), "A");
}

#[test]
fn view_node_state_capture_preserves_handler_when_fingerprint_is_unchanged() {
    use crate::ui::event::{SemanticEvent, SemanticKind};
    use crate::ui::state::State;
    use crate::ui::view::label;
    use std::cell::Cell;
    use std::rc::Rc;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree = ViewAdapter::build(label("First").on_semantic_capture(
        SemanticKind::Change,
        &state,
        move |_| {
            first_for_handler.set(first_for_handler.get() + 1);
        },
    ));
    let root_id = tree.root_id().unwrap();

    ViewAdapter::reconcile(
        &mut tree,
        label("Second").on_semantic_capture(SemanticKind::Change, &state, move |_| {
            second_for_handler.set(second_for_handler.get() + 1);
        }),
    );

    let _ = tree.dispatch_semantic(SemanticEvent::change(root_id, "next"));

    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn semantic_handler_macro_preserves_handler_when_fingerprint_is_unchanged() {
    use crate::ui::event::{SemanticEvent, SemanticKind};
    use crate::ui::state::State;
    use crate::ui::view::label;
    use std::cell::Cell;
    use std::rc::Rc;

    let state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();
    let mut first = label("First").build();
    first.handlers = vec![crate::semantic_handler!(
        SemanticKind::Change,
        state[state],
        |_event| {
            first_for_handler.set(first_for_handler.get() + 1);
        }
    )];
    let mut tree = ViewAdapter::build_nodes(first);
    let root_id = tree.root_id().unwrap();

    let mut second = label("Second").build();
    second.handlers = vec![crate::semantic_handler!(
        SemanticKind::Change,
        state[state],
        |_event| {
            second_for_handler.set(second_for_handler.get() + 1);
        }
    )];
    ViewAdapter::reconcile_nodes(&mut tree, second);

    let _ = tree.dispatch_semantic(SemanticEvent::change(root_id, "next"));

    assert_eq!(first_hits.get(), 1);
    assert_eq!(second_hits.get(), 0);
}

#[test]
fn view_node_state_capture_reregisters_handler_when_fingerprint_changes() {
    use crate::ui::event::{SemanticEvent, SemanticKind};
    use crate::ui::state::State;
    use crate::ui::view::label;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_state = State::new(1);
    let second_state = State::new(1);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree = ViewAdapter::build(label("First").on_semantic_capture(
        SemanticKind::Change,
        &first_state,
        move |_| {
            first_for_handler.set(first_for_handler.get() + 1);
        },
    ));
    let root_id = tree.root_id().unwrap();

    ViewAdapter::reconcile(
        &mut tree,
        label("Second").on_semantic_capture(SemanticKind::Change, &second_state, move |_| {
            second_for_handler.set(second_for_handler.get() + 1);
        }),
    );

    let _ = tree.dispatch_semantic(SemanticEvent::change(root_id, "next"));

    assert_eq!(first_hits.get(), 0);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn view_node_window_capture_reregisters_handler_when_fingerprint_changes() {
    use crate::core::WindowId;
    use crate::ui::event::{SemanticEvent, SemanticKind};
    use crate::ui::view::label;
    use std::cell::Cell;
    use std::rc::Rc;

    let first_window = WindowId::new(7);
    let second_window = WindowId::new(8);
    let first_hits = Rc::new(Cell::new(0));
    let second_hits = Rc::new(Cell::new(0));
    let first_for_handler = first_hits.clone();
    let second_for_handler = second_hits.clone();

    let mut tree = ViewAdapter::build(label("First").on_semantic_window_capture(
        SemanticKind::Change,
        first_window,
        move |_| {
            first_for_handler.set(first_for_handler.get() + 1);
        },
    ));
    let root_id = tree.root_id().unwrap();

    ViewAdapter::reconcile(
        &mut tree,
        label("Second").on_semantic_window_capture(
            SemanticKind::Change,
            second_window,
            move |_| {
                second_for_handler.set(second_for_handler.get() + 1);
            },
        ),
    );

    let _ = tree.dispatch_semantic(SemanticEvent::change(root_id, "next"));

    assert_eq!(first_hits.get(), 0);
    assert_eq!(second_hits.get(), 1);
}

#[test]
fn test_state_auto_reconcile_invalidation() {
    use crate::ui::state::State;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let state = State::new(42);
    let reconcile_called = Arc::new(AtomicBool::new(false));
    let reconcile_called_clone = reconcile_called.clone();
    state.set_reconcile_invalidation_fn(move || {
        reconcile_called_clone.store(true, Ordering::SeqCst)
    });
    assert!(!reconcile_called.load(Ordering::SeqCst));
    state.set(100);
    assert!(reconcile_called.load(Ordering::SeqCst));
}

#[test]
fn captured_state_set_requests_reconcile_and_preserves_paint_invalidation() {
    use crate::core::Rect;
    use crate::ui::state::State;
    use crate::ui::view::dynamic_label;

    let state = State::new(1);
    let mut tree = ViewAdapter::build({
        let state = state.clone();
        dynamic_label(move || format!("value-{}", state.get()))
    });
    let root_id = tree.root_id().expect("root should exist");
    tree.get_mut(root_id)
        .expect("root should be present")
        .set_frame(Rect::new(0.0, 0.0, 80.0, 24.0));
    tree.layout();
    tree.reset_invalidation();

    assert!(!tree.take_reconcile_requested());
    assert!(!tree.has_render_work());

    state.set(2);

    assert!(tree.take_reconcile_requested());
    assert!(tree.has_render_work());
}

#[test]
fn button_on_click_is_registered_as_semantic_handler() {
    use crate::core::Point;
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::view::button;
    use crate::ui::SystemEvent;
    use std::cell::Cell;
    use std::rc::Rc;

    let clicks = Rc::new(Cell::new(0));
    let clicks_for_handler = clicks.clone();
    let mut tree = ViewAdapter::build(button("OK").on_click(move || {
        clicks_for_handler.set(clicks_for_handler.get() + 1);
    }));

    let pos = Point::new(2.0, 2.0);
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

    assert_eq!(clicks.get(), 1);
}

#[test]
fn input_on_change_is_registered_as_semantic_handler() {
    use crate::core::{Point, Rect};
    use crate::native::traits::input::{KeyMod, MouseButton};
    use crate::ui::view::input;
    use crate::ui::SystemEvent;
    use std::cell::RefCell;
    use std::rc::Rc;

    let value = Rc::new(RefCell::new(String::new()));
    let value_for_handler = value.clone();
    let mut tree = ViewAdapter::build(input().on_change(move |next| {
        *value_for_handler.borrow_mut() = next.to_string();
    }));

    let root = tree.root_id().expect("input root should exist");
    tree.get_mut(root)
        .expect("input root should be present")
        .set_frame(Rect::new(0.0, 0.0, 120.0, 32.0));

    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::TextInput {
        text: "A".to_string(),
    });

    assert_eq!(&*value.borrow(), "A");
}

#[test]
fn static_display_widgets_are_picture_eligible() {
    use crate::draw::compositor::PicturePolicy;
    use crate::ui::widgets::{
        Alert, Avatar, Badge, BarChart, Container, Content, Descriptions, Divider, Empty, Footer,
        Grid, Header, Icon, Layout, LineChart, List, PieChart, QRCode, Result as ResultWidget,
        ResultType, Sider, Skeleton, Space, Tag, Timeline, Watermark,
    };

    let widgets: Vec<Box<dyn WidgetComponent>> = vec![
        Box::new(Space::new()),
        Box::new(Container::new()),
        Box::new(Grid::new()),
        Box::new(Divider::new()),
        Box::new(Icon::new("search")),
        Box::new(Avatar::new("A")),
        Box::new(Badge::new().count(8)),
        Box::new(Layout::new()),
        Box::new(Header::new(48.0)),
        Box::new(Sider::new(200.0)),
        Box::new(Content::new()),
        Box::new(Footer::new(40.0)),
        Box::new(Empty::new()),
        Box::new(Tag::new("stable")),
        Box::new(Descriptions::new()),
        Box::new(ResultWidget::new(ResultType::Info)),
        Box::new(Alert::new("stable")),
        Box::new(Timeline::new()),
        Box::new(Skeleton::new()),
        Box::new(List::new()),
        Box::new(BarChart::new()),
        Box::new(LineChart::new()),
        Box::new(PieChart::new()),
        Box::new(QRCode::new("stable")),
        Box::new(Watermark::new("stable")),
    ];

    for widget in widgets {
        assert_eq!(widget.picture_policy(), PicturePolicy::Eligible);
    }
}

#[test]
fn layout_bootstraps_after_invalidation_cleared_with_nonzero_measure_children() {
    use crate::core::Rect;
    use crate::ui::view::{column, label, row};
    use crate::ui::widgets::Container;

    // 与 demo shell 同构：水平 row = 侧栏 + 内容
    let root = row([
        column([label("侧栏项")]).width(120.0).flex_grow(0.0),
        column([label("内容"), label("页脚")]).flex_grow(1.0),
    ])
    .flex_grow(1.0);
    let mut tree = ViewAdapter::build_nodes(root);
    if let Some(r) = tree.root_mut() {
        r.set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
    }
    // 模拟 bind_invalidation / 帧末 reset：清空队列后子树仍为 zero frame
    tree.reset_invalidation();
    assert!(tree.layout_traverse().is_empty());

    tree.layout();

    let root_id = tree.root_id().expect("root");
    let main_children = tree.get(root_id).unwrap().children().to_vec();
    assert_eq!(main_children.len(), 2);
    let sidebar = tree.get(main_children[0]).unwrap().frame();
    let content = tree.get(main_children[1]).unwrap().frame();
    assert!(
        sidebar.w > 0.0 && sidebar.h > 0.0,
        "sidebar still zero: {sidebar:?}"
    );
    assert!(
        content.w > 0.0 && content.h > 0.0,
        "content still zero: {content:?}"
    );
    assert!(
        content.x >= sidebar.w - 0.5,
        "content should sit right of sidebar: sidebar={sidebar:?} content={content:?}"
    );

    let sidebar_container = tree
        .get(main_children[0])
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Container>()
        .expect("sidebar column is Container");
    assert_eq!(
        sidebar_container.style.flex_grow, 0.0,
        "explicit flex_grow(0) must survive Style::apply"
    );
}
