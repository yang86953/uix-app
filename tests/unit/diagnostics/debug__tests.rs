use super::*;
use std::ffi::OsStr;

#[test]
fn debug_switch_requires_an_explicit_true_value() {
    for value in ["1", "true", "TRUE", "yes", "on"] {
        assert_eq!(parse_debug_switch(OsStr::new(value)), Ok(true));
    }
}

#[test]
fn debug_switch_treats_zero_and_false_values_as_disabled() {
    for value in ["", "0", "false", "FALSE", "no", "off"] {
        assert_eq!(parse_debug_switch(OsStr::new(value)), Ok(false));
    }
}

#[test]
fn debug_switch_rejects_ambiguous_values() {
    assert_eq!(parse_debug_switch(OsStr::new("debug")), Err(()));
}
