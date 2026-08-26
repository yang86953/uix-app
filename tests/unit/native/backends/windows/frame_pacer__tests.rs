use super::{enqueue_worker_failure, format_dwm_flush_failure};
use crate::core::{Errc, Error};
use crate::diagnostics::PendingFailureQueue;

#[test]
fn frame_worker_failures_are_deferred_to_owner_boundary() {
    let queue = PendingFailureQueue::new();
    let source = queue.source();
    let callback_source = source.clone();
    std::thread::spawn(move || {
        enqueue_worker_failure(
            &callback_source,
            Error::new(Errc::PlatformError, format_dwm_flush_failure(-1)),
        );
        enqueue_worker_failure(
            &callback_source,
            Error::new(
                Errc::PlatformError,
                "Windows DWM frame pacer: PostMessageW failed",
            ),
        );
    })
    .join()
    .unwrap_or_else(|_| panic!("frame worker callback must finish"));

    let Some(flush_failure) = source.take() else {
        panic!("DwmFlush failure must reach the owner source");
    };
    assert_eq!(flush_failure.code(), Errc::PlatformError);
    assert!(flush_failure.message().contains("DwmFlush"));

    let Some(post_failure) = source.take() else {
        panic!("PostMessageW failure must reach the owner source");
    };
    assert_eq!(post_failure.code(), Errc::PlatformError);
    assert!(post_failure.message().contains("PostMessageW"));
    assert!(source.take().is_none());
}
