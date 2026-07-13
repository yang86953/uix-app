use crate::core::diagnostic::Collector;
use crate::core::{Errc, Error};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[test]
fn collector_callback_can_reenter_without_holding_internal_lock() {
    const MESSAGE: &str = "collector reentrant callback probe";

    let observed = Arc::new(AtomicBool::new(false));
    let callback_observed = Arc::clone(&observed);
    let collector = Collector::instance();
    let token = collector.on_collect(move |error| {
        if error.message() == MESSAGE {
            let _ = Collector::instance().stored_count();
            callback_observed.store(true, Ordering::Release);
        }
    });

    collector.collect(Error::new(Errc::Unknown, MESSAGE));

    assert!(observed.load(Ordering::Acquire));
    assert!(collector.remove_callback(token));
}
