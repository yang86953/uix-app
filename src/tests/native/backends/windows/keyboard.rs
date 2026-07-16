use crate::native::backends::windows::keyboard::virtual_key_candidates;
use crate::native::traits::input::KeyCode;

#[test]
fn super_key_state_queries_both_windows_keys() {
    assert_eq!(virtual_key_candidates(KeyCode::Super), [0x5B, 0x5C]);
}

#[test]
fn ordinary_and_unknown_keys_keep_single_candidate_semantics() {
    assert_eq!(virtual_key_candidates(KeyCode::A), [0x41, 0]);
    assert_eq!(virtual_key_candidates(KeyCode::F12), [0x7B, 0]);
    assert_eq!(virtual_key_candidates(KeyCode::Unknown), [0, 0]);
}
