use super::*;
use crate::native::shared::OsEventSource;

#[test]
fn waker_records_wake_calls() {
    let source = FakeEventSource::new();
    let waker = OsEventSource::waker(&source);

    waker.wake();
    waker.wake();

    assert_eq!(source.wake_count(), 2);
}
