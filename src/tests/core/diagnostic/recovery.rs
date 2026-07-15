use crate::core::diagnostic::{retry, FixedRetryPolicy, RecoveryHandler};
use crate::core::{Errc, Error};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[test]
fn retry_callback_runs_only_when_another_attempt_will_execute() {
    let mut handler = RecoveryHandler::new(Box::new(FixedRetryPolicy::new(1, 0)));
    let callback_attempts = Arc::new(Mutex::new(Vec::new()));
    let observed_attempts = Arc::clone(&callback_attempts);
    handler.set_on_retry(move |_, attempt| {
        observed_attempts
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(attempt);
    });
    let operation_attempts = AtomicUsize::new(0);

    let result = handler.execute(|| {
        operation_attempts.fetch_add(1, Ordering::Relaxed);
        Err(Error::new(Errc::IoError, "still failing"))
    });

    assert_eq!(
        result.expect_err("operation must fail").code(),
        Errc::IoError
    );
    assert_eq!(operation_attempts.load(Ordering::Relaxed), 2);
    assert_eq!(
        callback_attempts
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_slice(),
        [0]
    );
}

#[test]
fn retry_accepts_an_operation_that_mutates_borrowed_state() {
    let mut attempts = 0;

    let result = retry(
        2,
        || {
            attempts += 1;
            if attempts == 2 {
                Ok(())
            } else {
                Err(Error::new(Errc::IoError, "first attempt fails"))
            }
        },
        0,
    );

    assert!(result.is_ok());
    assert_eq!(attempts, 2);
}
