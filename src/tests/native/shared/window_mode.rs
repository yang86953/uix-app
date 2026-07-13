use crate::native::shared::window_mode::{NativeMaximizeTransition, NativeWindowModeState};

#[test]
fn interleaved_native_configures_keep_window_modes_isolated() {
    let mut primary = NativeWindowModeState::default();
    let mut secondary = NativeWindowModeState::default();

    assert_eq!(
        primary.apply_configure(true, false),
        NativeMaximizeTransition::Maximized
    );
    assert_eq!(primary.snapshot(), (true, false));
    assert_eq!(secondary.snapshot(), (false, false));

    assert_eq!(
        secondary.apply_configure(false, true),
        NativeMaximizeTransition::Unchanged
    );
    assert_eq!(secondary.snapshot(), (false, true));
    assert_eq!(primary.snapshot(), (true, false));

    assert_eq!(
        primary.apply_configure(false, false),
        NativeMaximizeTransition::Restored
    );
    assert_eq!(
        secondary.apply_configure(true, false),
        NativeMaximizeTransition::Maximized
    );
    assert_eq!(primary.snapshot(), (false, false));
    assert_eq!(secondary.snapshot(), (true, false));

    assert_eq!(
        secondary.apply_configure(false, false),
        NativeMaximizeTransition::Restored
    );
    assert_eq!(primary.snapshot(), (false, false));
    assert_eq!(secondary.snapshot(), (false, false));
}
