use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::input::select::*;

fn large_select() -> Select {
    let opts: Vec<String> = (0..100).map(|i| format!("Option {i}")).collect();
    Select::new().options(opts)
}

#[test]
fn select_dropdown_scroll_range_limits_visible_rows() {
    let select = large_select();
    let row_count = select.dropdown_row_count();
    let viewport_h = select.dropdown_viewport_height(row_count);
    let (start, end) = select
        .dropdown_scroll
        .scroll_range(row_count, 28.0, viewport_h);
    assert_eq!(start, 0);
    assert!(
        end - start < 100,
        "virtual scroll should expose a small window"
    );
}

#[test]
fn select_dropdown_wheel_records_composite_delta() {
    let mut select = large_select();
    select.open();

    assert_eq!(
        EventHandler::on_event(
            &mut select,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 50.0),
                delta: Point::new(0.0, -1.0),
            },
        ),
        EventResult::Handled
    );
    assert!(select.dropdown_scroll.scroll_offset() > 0.0);
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&select),
        Some((0.0, 40.0))
    );
}

#[test]
fn select_dropdown_row_at_y_accounts_for_scroll_offset() {
    let mut select = large_select();
    select.open();
    select.dropdown_scroll.set_scroll_offset(28.0 * 5.0);

    assert_eq!(select.dropdown_row_at_y(33.0), Some(5));
    assert_eq!(select.dropdown_row_at_y(61.0), Some(6));
}

#[test]
fn single_value_binding_reads_and_writes_option_text() {
    let options = ["Alpha", "Beta", "Gamma"];
    let selected = State::new("Beta".to_owned());
    let mut select = Select::new().options(&options).value(&selected);
    assert_eq!(select.current_value().as_deref(), Some("Beta"));

    select.open();
    assert_eq!(
        select.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 40.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(selected.get(), "Alpha");
    assert_eq!(select.current_value().as_deref(), Some("Alpha"));

    selected.set("Gamma".to_owned());
    select.sync_from(Select::new().value(&selected).options(&options));
    assert_eq!(select.current_value().as_deref(), Some("Gamma"));

    selected.set(String::new());
    select.sync_from(Select::new().options(&options).value(&selected));
    assert_eq!(select.current_value(), None);
    select.open();
    let _ = select.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), "Alpha");
}

#[test]
fn multiple_value_binding_toggles_a_hash_set() {
    let options = ["Alpha", "Beta", "Gamma"];
    let selected = State::new(HashSet::from(["Beta".to_owned()]));
    let mut select = Select::multiple().options(&options).value(&selected);
    assert_eq!(select.current_values(), HashSet::from(["Beta".to_owned()]));

    select.open();
    let _ = select.on_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 40.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(
        selected.get(),
        HashSet::from(["Alpha".to_owned(), "Beta".to_owned()])
    );

    let _ = select.on_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 68.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), HashSet::from(["Alpha".to_owned()]));
}

#[test]
fn searchable_factory_and_external_state_reconcile_are_structural() {
    let options = ["Alpha", "Beta"];
    let selected = State::new("Alpha".to_owned());
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Select::searchable().options(&options).value(&selected))
    }));
    let root = tree.root_id().expect("select root");
    tree.reset_invalidation();

    selected.set("Beta".to_owned());
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| {
        ViewNode::leaf(Select::searchable().options(&options).value(&selected))
    });
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let select = tree
        .get(root)
        .expect("select node")
        .component()
        .as_any()
        .downcast_ref::<Select>()
        .expect("Select component");
    assert_eq!(select.current_value().as_deref(), Some("Beta"));
    assert!(matches!(
        select.snapshot_fields(),
        SnapshotFields::Select { search: true, .. }
    ));
}
