use super::*;

#[test]
fn event_ring_is_bounded_and_keeps_latest_facts() {
    let module = ReproModule::new();
    for correlation_id in 1..=140 {
        module.record_input(WindowId::new(7), correlation_id, "pointer_move");
    }

    let snapshot = module.capture(CaptureReason::Manual, 9, true);
    assert_eq!(snapshot.events.len(), MAX_EVENTS);
    assert_eq!(snapshot.dropped_events, 12);
    assert_eq!(
        snapshot.events.first().map(|event| event.sequence),
        Some(13)
    );
    assert_eq!(
        snapshot.events.last().map(|event| event.sequence),
        Some(140)
    );
    let rendered = render(&snapshot);
    assert!(rendered.len() <= MAX_MANIFEST_BYTES);
    assert!(rendered.contains("event_count_retained=128\n"));
}

#[test]
fn manifest_keeps_codes_but_never_error_text_or_paths() {
    let module = ReproModule::new();
    let error = Error::new(Errc::PlatformError, "token=super-secret")
        .with_source(Error::new(Errc::IoError, "/home/private/profile.txt"));
    module.record_error(ReportId(3), &error);

    let rendered = render(&module.capture(CaptureReason::Manual, 11, false));
    assert!(rendered.contains("code:platform_error"));
    assert!(rendered.contains("cause_codes:io_error"));
    assert!(!rendered.contains("super-secret"));
    assert!(!rendered.contains("/home/private"));
}

#[test]
fn panic_capture_never_waits_for_repro_lock() {
    let module = ReproModule::new();
    let _held = module
        .state
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let snapshot = module.try_capture_for_panic(17, true);
    assert!(snapshot.capture_busy);
    assert!(snapshot.events.is_empty());
    assert!(render(&snapshot).contains("capture_busy=true"));
}
