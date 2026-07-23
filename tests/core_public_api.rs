use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use uix::core::{
    Collector, Errc, Error, FixedRetryPolicy, MiddlewareContext, MiddlewarePipeline, Point,
    RecoveryHandler, Rect, RetryMiddleware,
};

#[test]
fn core_geometry_and_error_chain_are_public_contracts() {
    let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
    assert!(rect.contains(Point::new(25.0, 35.0)));

    let root = Error::new(Errc::IoError, "disk unavailable");
    let error = Error::new(Errc::InvalidState, "settings load failed").with_source(root);
    assert_eq!(error.depth(), 1);
    assert_eq!(error.root_cause().code(), Errc::IoError);
    assert!(error.what().contains("disk unavailable"));
}

#[test]
fn core_diagnostics_observe_errors_and_recovery_through_public_api() {
    const MESSAGE: &str = "core public diagnostic probe";

    let observed = Arc::new(AtomicBool::new(false));
    let callback_observed = Arc::clone(&observed);
    let collector = Collector::instance();
    let token = collector.on_collect(move |error| {
        if error.message() == MESSAGE {
            callback_observed.store(true, Ordering::Release);
        }
    });
    collector.collect(Error::warn(Errc::Unknown, MESSAGE));
    assert!(observed.load(Ordering::Acquire));
    assert!(collector.remove_callback(token));

    const LOG_MESSAGE: &str = "core error log collector bridge probe";
    uix::core::error_fn(LOG_MESSAGE);
    assert!(collector
        .errors()
        .iter()
        .any(|error| error.message() == LOG_MESSAGE));

    let mut attempts = 0;
    let handler = RecoveryHandler::new(Box::new(FixedRetryPolicy::new(1, 0)));
    let error = handler
        .execute(|| {
            attempts += 1;
            Err(Error::new(Errc::IoError, "retry probe"))
        })
        .expect_err("the public recovery policy must return its terminal error");
    assert_eq!(attempts, 2);
    assert_eq!(error.code(), Errc::IoError);
}

#[test]
fn core_middleware_retries_through_its_public_pipeline() {
    let mut pipeline = MiddlewarePipeline::new();
    pipeline.add(RetryMiddleware::new(1));
    let mut context = MiddlewareContext::default();
    let mut attempts = 0;

    pipeline.execute(&mut context, |context| {
        attempts += 1;
        context.succeeded = attempts == 2;
        if !context.succeeded {
            context.status_code = 503;
            context.error_message = "unavailable".to_owned();
        }
    });

    assert_eq!(attempts, 2);
    assert!(context.succeeded);
    assert_eq!(context.retry_count, 1);
}
