use crate::tests::common::*;
use crate::ui::widgets::display::{Collapse, CollapsePanel, Table, TableColumn, Tree, TreeNode};
use crate::ui::widgets::feedback::{Drawer, Modal, Popover, Tooltip, TriggerMode};
use crate::ui::widgets::general::Label;
use crate::ui::widgets::navigation::{Anchor, AnchorItem, Dropdown};

fn secondary_down() -> SystemEvent {
    SystemEvent::PointerDown {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    }
}

#[test]
fn built_in_widgets_reserve_secondary_pointer_for_context_menus() {
    let mut widgets: Vec<(&str, Box<dyn EventHandler>)> = vec![
        ("modal", Box::new(Modal::new("Dialog"))),
        ("drawer", Box::new(Drawer::new("Drawer"))),
        ("popover", Box::new(Popover::new("details"))),
        (
            "tooltip",
            Box::new(Tooltip::new("help").trigger(TriggerMode::Click)),
        ),
        (
            "anchor",
            Box::new(Anchor::new(vec![AnchorItem::new("Intro", "#intro")])),
        ),
        (
            "dropdown",
            Box::new(Dropdown::new("Actions").items(vec!["Edit"])),
        ),
        (
            "collapse",
            Box::new(Collapse::new().panels(vec![CollapsePanel::new("A", "body")])),
        ),
        (
            "table",
            Box::new(
                Table::new()
                    .columns(vec![TableColumn::new("Name", 80.0)])
                    .rows(vec![vec!["Ada".to_owned()]])
                    .selection(true),
            ),
        ),
        (
            "tree",
            Box::new(Tree::new(vec![TreeNode::new("Root", "root")])),
        ),
        (
            "selectable label",
            Box::new(Label::new("copy").selectable()),
        ),
    ];
    let event = secondary_down();

    for (name, widget) in &mut widgets {
        assert_eq!(
            widget.on_event(&event),
            EventResult::NotHandled,
            "{name} must not treat the secondary pointer as ordinary activation"
        );
    }
}

#[test]
fn selectable_text_ignores_secondary_pointer_release() {
    let event = SystemEvent::PointerUp {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    };

    assert_eq!(
        Label::new("copy").selectable().on_event(&event),
        EventResult::NotHandled
    );
}
