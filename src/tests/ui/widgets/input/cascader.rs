use crate::tests::common::*;
use crate::ui::widgets::{Cascader, CascaderOption};

fn cascader() -> Cascader {
    Cascader::new(
        vec![
            CascaderOption::new("Unavailable", "unavailable").disabled(true),
            CascaderOption::new("China", "china").children(vec![
                CascaderOption::new("Unavailable city", "unavailable-city").disabled(true),
                CascaderOption::new("Beijing", "beijing"),
            ]),
            CascaderOption::new("Singapore", "singapore"),
        ],
        "Region",
    )
}

#[test]
fn cascader_keyboard_enters_levels_and_skips_disabled_options() {
    let mut cascader = cascader();
    assert_eq!(
        cascader.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Down,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(cascader.is_open());

    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["china"]);
    assert!(cascader.is_open());

    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["china", "beijing"]);
    assert!(!cascader.is_open());
    let accessibility = cascader.snapshot_fields().accessibility();
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("China / Beijing")
    );
    assert_eq!(accessibility.state.expanded, Some(false));
}

#[test]
fn cascader_arrow_navigation_wraps_across_enabled_root_options() {
    let mut cascader = cascader();
    cascader.open();
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["singapore"]);

    cascader.open();
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Up,
        mods: KeyMod::NONE,
    });
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["singapore"]);
}

#[test]
fn cascader_reconcile_rebuilds_open_navigation_from_new_options() {
    let mut cascader = cascader();
    cascader.open();
    cascader.sync_from(Cascader::new(
        vec![CascaderOption::new("Japan", "japan")],
        "New region",
    ));

    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["japan"]);
    assert!(matches!(
        cascader.snapshot_fields(),
        SnapshotFields::Cascader {
            selected_values,
            open: false,
            ..
        } if selected_values == ["japan"]
    ));
}

#[test]
fn cascader_left_returns_to_parent_level() {
    let mut cascader = cascader();
    cascader.open();
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    let _ = cascader.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(cascader.selected().values, ["singapore"]);
}
