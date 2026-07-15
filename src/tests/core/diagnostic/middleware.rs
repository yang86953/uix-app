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
