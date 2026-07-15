use crate::core::error::result::*;
use crate::core::error::ResultExt;
use crate::tests::common::*;

#[test]
fn try_invoke_panic_returns_error() {
    let result = try_invoke(|| -> i32 {
        panic!("intentional panic in test");
    });
    assert!(result.has_error());
    assert_eq!(result.err_code(), Some(Errc::Unknown));
    assert_eq!(
        result.err_message(),
        Some("function panicked: intentional panic in test")
    );
}

#[test]
fn try_invoke_preserves_owned_panic_message() {
    let result = try_invoke(|| -> i32 {
        std::panic::panic_any(String::from("owned panic in test"));
    });

    assert_eq!(
        result.err_message(),
        Some("function panicked: owned panic in test")
    );
}

#[test]
fn would_block_has_stable_general_error_mapping() {
    assert_eq!(Errc::WouldBlock.category(), "general");
    assert_eq!(Errc::WouldBlock.to_string(), "would_block");
    assert_eq!(
        Errc::WouldBlock.to_io_kind(),
        Some(std::io::ErrorKind::WouldBlock)
    );
}
