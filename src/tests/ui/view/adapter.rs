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
    let styled = ViewAdapter::apply_style(widget, &style);
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
    tree.reset_dirty();

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

    tree.reset_dirty();
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

    tree.reset_dirty();
    assert!(!tree.take_reconcile_requested());

    state.set(2);

    assert!(tree.take_reconcile_requested());
    assert!(tree.has_render_work());
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
fn test_state_auto_dirty() {
    use crate::ui::state::State;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let state = State::new(42);
    let dirty_called = Arc::new(AtomicBool::new(false));
    let dirty_called_clone = dirty_called.clone();
    state.set_dirty_fn(move || dirty_called_clone.store(true, Ordering::SeqCst));
    assert!(!dirty_called.load(Ordering::SeqCst));
    state.set(100);
    assert!(dirty_called.load(Ordering::SeqCst));
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
    tree.reset_dirty();

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
