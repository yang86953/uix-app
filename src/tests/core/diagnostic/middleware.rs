use crate::core::diagnostic::middleware::retry_exhausted_message_for_test;
use crate::core::diagnostic::{MiddlewareContext, MiddlewarePipeline, RetryMiddleware};

#[test]
fn pipeline_handler_can_borrow_invocation_state() {
    let mut pipeline = MiddlewarePipeline::new();
    pipeline.add(RetryMiddleware::new(0));
    let mut context = MiddlewareContext::default();
    let mut handled = 0;

    pipeline.execute(&mut context, |context| {
        handled += 1;
        context.operation = "borrowed handler".to_owned();
    });

    assert_eq!(handled, 1);
    assert_eq!(context.operation, "borrowed handler");
}

#[test]
fn retry_resets_the_previous_attempt_status_before_reentry() {
    let mut pipeline = MiddlewarePipeline::new();
    pipeline.add(RetryMiddleware::new(1));
    let mut context = MiddlewareContext::default();
    let mut attempts = 0;

    pipeline.execute(&mut context, |context| {
        if attempts == 0 {
            context.succeeded = false;
            context.status_code = 503;
            context.error_message = "unavailable".to_owned();
        } else {
            assert_eq!(context.status_code, 0);
            assert!(context.error_message.is_empty());
        }
        attempts += 1;
    });

    assert_eq!(attempts, 2);
    assert!(context.succeeded);
    assert_eq!(context.status_code, 0);
    assert_eq!(context.retry_count, 1);
}

#[test]
fn retry_exhaustion_reports_executed_attempts() {
    assert_eq!(
        retry_exhausted_message_for_test("service", 1),
        "[RETRY] service exhausted after 1 attempt"
    );
    assert_eq!(
        retry_exhausted_message_for_test("service", 4),
        "[RETRY] service exhausted after 4 attempts"
    );
}
