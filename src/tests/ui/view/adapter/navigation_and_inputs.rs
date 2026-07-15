use super::*;

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
            active_key,
            mode: MenuMode::Vertical,
            item_h: 44.0,
        } if items.len() == 1
            && items[0].key == "c"
            && items[0].disabled
            && active_key == "b"
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
            value,
            value_key,
            open,
        } if placeholder == "new placeholder"
            && nodes.len() == 1
            && nodes[0].key == "new"
            && value == "Old"
            && value_key == "old"
            && open
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
            selected_values,
            open,
            ..
        } if placeholder == "new placeholder"
            && options.len() == 1
            && options[0].value == "new"
            && selected_values == ["old"]
            && open
    ));
}

#[test]
fn reconcile_color_picker_preserves_open_state_and_syncs_controlled_color() {
    use crate::ui::state::State;
    use crate::ui::widgets::ColorPicker;

    let selected = State::new(Color::red());
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(ColorPicker::new().value(&selected)));
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
    }

    selected.set(Color::green());
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(ColorPicker::new().value(&selected)),
    );

    let color_picker = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ColorPicker>()
        .unwrap();
    assert!(color_picker.is_open());
    assert_eq!(color_picker.current_value(), Color::green());
    assert!(matches!(
        color_picker.snapshot_fields(),
        SnapshotFields::ColorPicker {
            value,
            preset_colors,
        } if value == Color::green() && !preset_colors.is_empty()
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
            value,
            open,
        } if placeholder == "new"
            && options == vec!["Apricot", "Avocado"]
            && value == "A"
            && open
    ));
}

#[test]
fn reconcile_input_preserves_typed_value_and_syncs_placeholder() {
    use crate::ui::widgets::Input;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Input::new("old")));
    let root_id = tree.root_id().expect("input root should exist");
    {
        let input = tree
            .get_mut(root_id)
            .unwrap()
            .component_mut()
            .as_any_mut()
            .downcast_mut::<Input>()
            .unwrap();
        input.set_value("typed");
        input.set_focused(true);
    }

    ViewAdapter::reconcile_nodes(&mut tree, ViewNode::leaf(Input::new("new")));

    let input = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Input>()
        .unwrap();
    assert_eq!(input.current_value(), "typed");
    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::Input {
            placeholder,
            ..
        } if placeholder == "new"
    ));
}

#[test]
fn reconcile_date_picker_preserves_open_value_and_syncs_placeholder() {
    use crate::ui::widgets::{Date, DatePicker};

    let selected = Date::new(2026, 7, 7);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        DatePicker::new().placeholder("old").default_value(selected),
    ));
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

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            DatePicker::new()
                .placeholder("new")
                .default_value(Date::new(2027, 8, 8)),
        ),
    );

    let date_picker = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<DatePicker>()
        .unwrap();
    assert!(date_picker.is_open());
    assert_eq!(date_picker.current_value(), selected);
    assert!(matches!(
        date_picker.snapshot_fields(),
        SnapshotFields::DatePicker { placeholder, .. } if placeholder == "new"
    ));
}

#[test]
fn reconcile_time_picker_preserves_open_value_and_syncs_placeholder() {
    use crate::ui::state::State;
    use crate::ui::widgets::{Time, TimePicker};

    let selected = State::new(Time::new(9, 30));
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        TimePicker::new().placeholder("old").value(&selected),
    ));
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

    selected.set(Time::new(10, 45));
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(TimePicker::new().placeholder("new").value(&selected)),
    );

    let time_picker = tree
        .get(root_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<TimePicker>()
        .unwrap();
    assert!(time_picker.is_open());
    assert_eq!(time_picker.current_value(), Time::new(10, 45));
    assert!(matches!(
        time_picker.snapshot_fields(),
        SnapshotFields::TimePicker { placeholder, value }
            if placeholder == "new" && value.as_deref() == Some("10:45")
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
    assert_eq!(mentions.value(), "@a");
    assert!(matches!(
        mentions.snapshot_fields(),
        SnapshotFields::Mentions {
            placeholder,
            options,
            value,
            suggesting,
        } if placeholder == "new"
            && options == vec!["Ann", "Cara"]
            && value == "@a"
            && suggesting
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
