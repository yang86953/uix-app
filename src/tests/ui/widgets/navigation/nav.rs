use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::ViewAdapter;
use crate::ui::widgets::navigation::nav::*;
use crate::ui::widgets::Button;
use crate::ui::ClickEvent;

#[test]
fn nav_item_is_focusable_and_keyboard_activation_emits_change() {
    let active = Rc::new(Cell::new(0));
    let mut item = NavItem::new("Settings", 1, active.clone());
    let event = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&item), 1);
    assert_eq!(item.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(item.on_event(&event), EventResult::Handled);
    assert_eq!(active.get(), 1);
    assert!(item.is_active());

    let semantic = item
        .semantic_event(ComponentId::new(5), &event)
        .expect("activation should emit change");
    assert_eq!(semantic.text_payload(), Some("1"));
}

#[test]
fn active_nav_item_does_not_emit_duplicate_change() {
    let active = Rc::new(Cell::new(2));
    let mut item = NavItem::new("Reports", 2, active);
    let event = SystemEvent::KeyDown {
        key: KeyCode::Space,
        mods: KeyMod::NONE,
    };

    assert_eq!(item.on_event(&event), EventResult::Handled);
    assert!(item.semantic_event(ComponentId::new(6), &event).is_none());
}

#[test]
fn nav_item_snapshot_and_accessibility_expose_selection() {
    let item = NavItem::new("Home", 0, Rc::new(Cell::new(0))).key("home");
    let fields = item.snapshot_fields();

    assert!(matches!(
        fields,
        SnapshotFields::NavItem {
            active: true,
            index: 0,
            ref key,
            ..
        } if key == "home"
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.name.as_deref(), Some("Home"));
    assert_eq!(accessibility.state.selected, Some(true));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestPage {
    Home,
    Settings,
    Missing,
}

impl std::fmt::Display for TestPage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Home => "home",
            Self::Settings => "settings",
            Self::Missing => "missing",
        })
    }
}

#[test]
fn navigation_reads_typed_state_and_item_activation_writes_it_back() {
    let page = State::new(TestPage::Settings);
    let navigation = Navigation::new("Test")
        .item("Home", TestPage::Home)
        .item("Settings", TestPage::Settings)
        .active_page(&page)
        .show_version(false);

    assert_eq!(navigation.active().get(), 1);
    assert_eq!(navigation.active_key(), Some(&TestPage::Settings));

    let theme = Theme::default();
    let mut root = navigation.build(theme.tokens());
    let home = root
        .children
        .iter_mut()
        .find_map(|child| child.widget.as_any_mut().downcast_mut::<NavItem>())
        .expect("navigation should build a NavItem");
    let event = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };
    home.on_event(&event);

    assert_eq!(page.get(), TestPage::Home);
    assert_eq!(
        home.semantic_event(ComponentId::new(7), &event)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("home".to_string())
    );
}

#[test]
fn navigation_exposes_no_active_key_for_an_unmatched_typed_state() {
    let page = State::new(TestPage::Missing);
    let navigation = Navigation::new("Test")
        .item("Home", TestPage::Home)
        .item("Settings", TestPage::Settings)
        .active_page(&page);

    assert_eq!(navigation.active().get(), usize::MAX);
    assert_eq!(navigation.active_key(), None);
}

#[test]
fn navigation_collapse_toggle_updates_state_and_invokes_callback() {
    let collapsed = State::new(false);
    let callback_values = Rc::new(RefCell::new(Vec::new()));
    let callback_values_for_handler = callback_values.clone();
    let theme = Theme::default();
    let navigation = Navigation::new("Test")
        .item("Home", TestPage::Home)
        .collapsed(&collapsed)
        .on_collapse(move |value| callback_values_for_handler.borrow_mut().push(value))
        .show_version(false);

    let mut tree =
        ViewAdapter::build_nodes(crate::ui::view::embed(navigation.build(theme.tokens())));
    let toggle = tree
        .find_by_type::<Button>()
        .expect("collapsed navigation should expose a toggle button");
    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(8.0, 8.0),
        modifiers: KeyMod::NONE,
    };

    assert_eq!(
        tree.dispatch_semantic(SemanticEvent::click(toggle, click)),
        EventResult::Handled
    );
    assert!(collapsed.get());
    assert_eq!(*callback_values.borrow(), vec![true]);

    assert_eq!(
        tree.dispatch_semantic(SemanticEvent::click(toggle, click)),
        EventResult::Handled
    );
    assert!(!collapsed.get());
    assert_eq!(*callback_values.borrow(), vec![true, false]);
}

#[test]
fn navigation_collapse_callback_alone_creates_internal_toggle_state() {
    let callback_values = Rc::new(RefCell::new(Vec::new()));
    let callback_values_for_handler = callback_values.clone();
    let theme = Theme::default();
    let navigation = Navigation::new("Test")
        .item("Home", TestPage::Home)
        .on_collapse(move |value| callback_values_for_handler.borrow_mut().push(value))
        .show_version(false);

    let mut tree =
        ViewAdapter::build_nodes(crate::ui::view::embed(navigation.build(theme.tokens())));
    let toggle = tree
        .find_by_type::<Button>()
        .expect("callback-only navigation should expose a toggle button");
    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(8.0, 8.0),
        modifiers: KeyMod::NONE,
    };

    assert_eq!(
        tree.dispatch_semantic(SemanticEvent::click(toggle, click)),
        EventResult::Handled
    );
    assert_eq!(*callback_values.borrow(), vec![true]);
}

#[test]
fn external_navigation_state_reconciles_existing_nav_items() {
    let page = State::new(TestPage::Home);
    let theme = Theme::default();
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        crate::ui::view::embed(
            Navigation::new("Test")
                .item("Home", TestPage::Home)
                .item("Settings", TestPage::Settings)
                .active_page(&page)
                .show_version(false)
                .build(theme.tokens()),
        )
    }));
    let root = tree.root_id().expect("navigation root");
    let nav_items = tree
        .get(root)
        .expect("navigation container")
        .children()
        .iter()
        .copied()
        .filter(|id| {
            tree.get(*id)
                .is_some_and(|node| node.component().as_any().is::<NavItem>())
        })
        .collect::<Vec<_>>();
    assert_eq!(nav_items.len(), 2);
    let first_ptr = tree
        .get(nav_items[0])
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<NavItem>()
        .unwrap() as *const NavItem;
    tree.reset_invalidation();

    page.set(TestPage::Settings);
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| {
        crate::ui::view::embed(
            Navigation::new("Test")
                .item("Home", TestPage::Home)
                .item("Settings", TestPage::Settings)
                .active_page(&page)
                .show_version(false)
                .build(theme.tokens()),
        )
    });
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let home = tree
        .get(nav_items[0])
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<NavItem>()
        .unwrap();
    let settings = tree
        .get(nav_items[1])
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<NavItem>()
        .unwrap();
    assert_eq!(home as *const NavItem, first_ptr);
    assert!(!home.is_active());
    assert!(settings.is_active());
}
