use crate::native::shared::OsEventSource;
use crate::native::test_harness::fake_event_source::*;

#[test]
fn waker_records_wake_calls() {
    let source = FakeEventSource::new();
    let waker = OsEventSource::waker(&source);

    waker.wake();
    waker.wake();

    assert_eq!(source.wake_count(), 2);
}
