use crate::tests::common::*;
use crate::core::error::result::*;
use crate::core::error::ResultExt;

#[test]
fn try_invoke_panic_returns_error() {
    let result = try_invoke(|| -> i32 {
        panic!("intentional panic in test");
    });
    assert!(result.has_error());
    assert_eq!(result.err_code(), Some(Errc::Unknown));
}
