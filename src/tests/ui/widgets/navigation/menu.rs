use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::navigation::menu::*;
use crate::ui::State;

fn render_menu(menu: &Menu, frame: Rect) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(480, 160));
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
            480,
            160,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(menu, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn menu_item(key: &str, label: &str) -> MenuItem {
    MenuItem {
        key: key.to_string(),
        label: label.to_string(),
        icon: String::new(),
        children: Vec::new(),
        disabled: false,
    }
}

fn nested_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem {
            children: vec![MenuItem {
                children: vec![menu_item("grandchild", "Grandchild")],
                ..menu_item("child", "Child")
            }],
            ..menu_item("root", "Root")
        },
        menu_item("other", "Other"),
    ]
}

fn pointer_down(y: f32) -> SystemEvent {
    SystemEvent::PointerDown {
        pos: Point::new(16.0, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

fn key_down(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    }
}

#[test]
fn menu_selection_emits_change_semantic_event() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(
        Menu::new()
            .add_item(MenuItem {
                key: "home".into(),
                label: "首页".into(),
                icon: String::new(),
                children: Vec::new(),
                disabled: false,
            })
            .add_item(MenuItem {
                key: "docs".into(),
                label: "文档".into(),
                icon: String::new(),
                children: Vec::new(),
                disabled: false,
            })
            .active_key("home"),
    ));
    tree.get_mut(id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 32.0));

    let selected = Rc::new(RefCell::new(String::new()));
    let selected_for_handler = selected.clone();
    tree.handler_table()
        .on(id, SemanticKind::Change, move |event| {
            if let Some(value) = event.text_payload() {
                *selected_for_handler.borrow_mut() = value.to_string();
            }
        });

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(90.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(&*selected.borrow(), "docs");
}

#[test]
fn menu_is_focusable_and_keyboard_navigation_wraps_while_skipping_disabled_items() {
    let mut menu = Menu::new()
        .add_item(MenuItem {
            key: "home".into(),
            label: "Home".into(),
            icon: String::new(),
            children: Vec::new(),
            disabled: false,
        })
        .add_item(MenuItem {
            key: "disabled".into(),
            label: "Disabled".into(),
            icon: String::new(),
            children: Vec::new(),
            disabled: true,
        })
        .add_item(MenuItem {
            key: "docs".into(),
            label: "Docs".into(),
            icon: String::new(),
            children: Vec::new(),
            disabled: false,
        });

    assert_eq!(WidgetComponent::tab_index(&menu), 1);
    assert_eq!(
        menu.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(menu.get_active_key(), "home");

    menu.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(menu.get_active_key(), "docs");

    menu.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(menu.get_active_key(), "home");

    menu.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(menu.get_active_key(), "docs");
}

#[test]
fn vertical_menu_only_handles_vertical_navigation_keys() {
    let mut menu = Menu::new()
        .add_item(MenuItem {
            key: "home".into(),
            label: "Home".into(),
            icon: String::new(),
            children: Vec::new(),
            disabled: false,
        })
        .mode(MenuMode::Vertical);

    assert_eq!(
        menu.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        menu.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(menu.get_active_key(), "home");
}

#[test]
fn inline_menu_recursively_expands_every_level_without_mutating_open_keys() {
    let selected = State::new(Vec::<String>::new());
    let open = State::new(Vec::<String>::new());
    let mut menu = Menu::new()
        .items(nested_menu_items())
        .mode(MenuMode::Inline)
        .selected_keys(&selected)
        .open_keys(&open);

    assert_eq!(
        menu.measure(Constraints::unconstrained()),
        Size::new(200.0, 128.0)
    );

    let open_generation = open.generation();
    assert_eq!(menu.on_event(&pointer_down(16.0)), EventResult::Handled);
    assert_eq!(selected.get(), vec!["root".to_string()]);
    assert!(open.get().is_empty());
    assert_eq!(open.generation(), open_generation);

    menu.on_event(&key_down(KeyCode::Down));
    assert_eq!(menu.get_active_key(), "child");
    menu.on_event(&key_down(KeyCode::Down));
    assert_eq!(menu.get_active_key(), "grandchild");
    menu.on_event(&key_down(KeyCode::Down));
    assert_eq!(menu.get_active_key(), "other");
}

#[test]
fn switching_between_vertical_and_inline_recomputes_nested_visibility() {
    let items = nested_menu_items();
    let empty = Vec::<String>::new();
    let mut menu = Menu::new()
        .items(items.clone())
        .mode(MenuMode::Vertical)
        .open_keys(&empty);

    assert_eq!(menu.measure(Constraints::unconstrained()).h, 64.0);

    menu.sync_from(
        Menu::new()
            .items(items.clone())
            .mode(MenuMode::Inline)
            .open_keys(&empty),
    );
    assert_eq!(menu.measure(Constraints::unconstrained()).h, 128.0);

    let open_path = vec!["root".to_string(), "child".to_string()];
    menu.sync_from(
        Menu::new()
            .items(items)
            .mode(MenuMode::Vertical)
            .open_keys(&open_path),
    );
    assert_eq!(menu.measure(Constraints::unconstrained()).h, 128.0);
}

#[test]
fn plain_key_collections_remain_typed_and_preserve_punctuation() {
    let punctuated_key = "root,\"quoted\"";
    let selected = vec![
        "missing".to_string(),
        punctuated_key.to_string(),
        punctuated_key.to_string(),
    ];
    let open = vec![punctuated_key.to_string(), punctuated_key.to_string()];
    let menu = Menu::new()
        .selected_keys(&selected)
        .open_keys(&open)
        .items(vec![MenuItem {
            children: vec![menu_item("leaf", "Leaf")],
            ..menu_item(punctuated_key, "Root")
        }])
        .mode(MenuMode::Vertical);

    assert_eq!(menu.get_active_key(), punctuated_key);
    assert_eq!(
        menu.get_selected_keys(),
        &["missing".to_string(), punctuated_key.to_string()]
    );
    assert_eq!(menu.get_open_keys(), &[punctuated_key.to_string()]);
    assert_eq!(menu.measure(Constraints::unconstrained()).h, 64.0);
}

#[test]
fn external_selected_and_open_states_request_reconcile_and_update_live_menu() {
    let selected = State::new(vec!["root".to_string()]);
    let open = State::new(Vec::<String>::new());
    let build = || {
        ViewAdapter::capture_root(|| {
            ViewNode::leaf(
                Menu::new()
                    .items(nested_menu_items())
                    .mode(MenuMode::Vertical)
                    .selected_keys(&selected)
                    .open_keys(&open),
            )
        })
    };
    let mut tree = ViewAdapter::build_nodes(build());
    let root = tree.root_id().expect("menu root");
    tree.reset_invalidation();

    selected.set(vec!["grandchild".to_string()]);
    open.set(vec!["root".to_string(), "child".to_string()]);
    assert!(tree.take_reconcile_requested());
    ViewAdapter::reconcile_nodes(&mut tree, build());

    let menu = tree
        .get(root)
        .expect("reused menu")
        .component()
        .as_any()
        .downcast_ref::<Menu>()
        .expect("Menu component");
    assert_eq!(menu.get_active_key(), "grandchild");
    assert_eq!(menu.get_selected_keys(), &["grandchild".to_string()]);
    assert_eq!(
        menu.get_open_keys(),
        &["root".to_string(), "child".to_string()]
    );
    assert_eq!(menu.measure(Constraints::unconstrained()).h, 128.0);
}

#[test]
fn menu_interactions_write_selected_and_open_states_without_redundant_selection_writes() {
    let selected = State::new(vec!["other".to_string()]);
    let open = State::new(Vec::<String>::new());
    let items = vec![
        MenuItem {
            children: vec![menu_item("child", "Child")],
            ..menu_item("root", "Root")
        },
        menu_item("other", "Other"),
    ];
    let mut menu = Menu::new()
        .items(items)
        .mode(MenuMode::Vertical)
        .selected_keys(&selected)
        .open_keys(&open);

    menu.on_event(&pointer_down(16.0));
    assert_eq!(selected.get(), vec!["root".to_string()]);
    assert_eq!(open.get(), vec!["root".to_string()]);

    menu.on_event(&pointer_down(48.0));
    assert_eq!(selected.get(), vec!["child".to_string()]);
    assert_eq!(open.get(), vec!["root".to_string()]);

    menu.on_event(&key_down(KeyCode::Down));
    assert_eq!(selected.get(), vec!["other".to_string()]);

    menu.on_event(&pointer_down(16.0));
    assert_eq!(selected.get(), vec!["root".to_string()]);
    assert!(open.get().is_empty());
    let selected_generation = selected.generation();

    menu.on_event(&pointer_down(16.0));
    assert_eq!(selected.generation(), selected_generation);
    assert_eq!(open.get(), vec!["root".to_string()]);
}

#[test]
fn reconcile_replaces_menu_bindings_and_old_states_no_longer_drive_or_receive_changes() {
    let old_selected = State::new(vec!["root".to_string()]);
    let old_open = State::new(vec!["root".to_string()]);
    let current_selected = State::new(vec!["other".to_string()]);
    let current_open = State::new(Vec::<String>::new());
    let items = vec![
        MenuItem {
            children: vec![menu_item("child", "Child")],
            ..menu_item("root", "Root")
        },
        menu_item("other", "Other"),
    ];
    let mut menu = Menu::new()
        .items(items.clone())
        .mode(MenuMode::Vertical)
        .selected_keys(&old_selected)
        .open_keys(&old_open);

    menu.sync_from(
        Menu::new()
            .items(items)
            .mode(MenuMode::Vertical)
            .selected_keys(&current_selected)
            .open_keys(&current_open),
    );
    assert_eq!(menu.get_active_key(), "other");
    assert!(menu.get_open_keys().is_empty());

    current_selected.set(vec!["child".to_string()]);
    current_open.set(vec!["root".to_string()]);
    menu.on_event(&SystemEvent::PointerLeave);
    assert_eq!(menu.get_active_key(), "child");
    assert_eq!(menu.get_open_keys(), &["root".to_string()]);

    old_selected.set(vec!["other".to_string()]);
    old_open.set(Vec::new());
    menu.on_event(&SystemEvent::PointerMove {
        pos: Point::new(500.0, 500.0),
        mods: KeyMod::NONE,
    });
    assert_eq!(menu.get_active_key(), "child");
    assert_eq!(menu.get_open_keys(), &["root".to_string()]);

    menu.on_event(&pointer_down(16.0));
    assert_eq!(current_selected.get(), vec!["root".to_string()]);
    assert!(current_open.get().is_empty());
    assert_eq!(old_selected.get(), vec!["other".to_string()]);
    assert!(old_open.get().is_empty());
}

#[test]
fn empty_and_unknown_controlled_keys_are_safe_and_keyboard_selection_recovers() {
    let selected = State::new(vec!["unknown".to_string(), "unknown".to_string()]);
    let open = State::new(vec!["missing-parent".to_string()]);
    let mut menu = Menu::new()
        .items(nested_menu_items())
        .mode(MenuMode::Vertical)
        .selected_keys(&selected)
        .open_keys(&open);

    assert_eq!(menu.get_active_key(), "");
    assert_eq!(menu.get_selected_keys(), &["unknown".to_string()]);
    assert_eq!(menu.measure(Constraints::unconstrained()).h, 64.0);

    menu.on_event(&key_down(KeyCode::Down));
    assert_eq!(menu.get_active_key(), "root");
    assert_eq!(selected.get(), vec!["root".to_string()]);

    selected.set(Vec::new());
    menu.on_event(&SystemEvent::PointerLeave);
    assert_eq!(menu.get_active_key(), "");
    assert!(menu.get_selected_keys().is_empty());
}

#[test]
fn menu_item_icon_changes_horizontal_measure_paint_and_keeps_hit_target_active() {
    let label = "ExportingData";
    let plain = Menu::new().add_item(MenuItem::new(label));
    let mut with_icon = Menu::new().add_item(MenuItem::new(label).icon("download"));
    let plain_size = plain.measure(Constraints::unconstrained());
    let icon_size = with_icon.measure(Constraints::unconstrained());

    assert_eq!(icon_size.w - plain_size.w, 20.0);
    let plain_render = render_menu(&plain, Rect::new(0.0, 0.0, plain_size.w, plain_size.h));
    let icon_render = render_menu(&with_icon, Rect::new(0.0, 0.0, icon_size.w, icon_size.h));
    assert!(
        icon_render.matches("DrawText { text:").count()
            > plain_render.matches("DrawText { text:").count(),
        "horizontal menu icon must emit an actual icon-font glyph: {icon_render}"
    );
    assert_eq!(
        with_icon.on_event(&SystemEvent::PointerDown {
            pos: Point::new(icon_size.w - 1.0, icon_size.h * 0.5),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(with_icon.get_active_key(), label);
}
