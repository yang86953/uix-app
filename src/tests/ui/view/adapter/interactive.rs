use super::*;

#[test]
fn reconcile_tree_preserves_selection_and_syncs_nodes() {
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
        SnapshotFields::Tree {
            nodes,
            selected_key,
            selected_keys,
            expanded_keys,
            multiple: true,
        }
            if nodes.len() == 1
                && nodes[0].title == "Root 2"
                && nodes[0].children[0].title == "Child 2"
                && selected_key == "child"
                && selected_keys == ["child"]
                && expanded_keys == ["root"]
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
    use crate::ui::widgets::NavItem;

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
                    pos: crate::core::Point::new(10.0, 30.0),
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                },
            ),
            EventResult::Handled
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
        assert_eq!(transfer.source_count(), 0);
        assert_eq!(transfer.target_count(), 1);
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
    use crate::ui::AnimationConfig;

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
                .arrow(false)
                .leave_animation(AnimationConfig::fade_out(0.5)),
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

    let popover = tree
        .get_mut(root_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Popover>()
        .unwrap();
    popover.close();
    assert!(crate::ui::traits::WidgetAnimation::update_animation(
        popover, 0.25
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
    use crate::ui::widgets::Modal;

    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Modal::new("old").visible(true)));
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
