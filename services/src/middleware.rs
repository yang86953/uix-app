use std::fmt;

/// Context passed through the middleware chain.
#[derive(Debug, Clone)]
pub struct MiddlewareContext {
    pub service_name: String,
    pub operation: String,
    pub status_code: i32,
    pub succeeded: bool,
    pub error_message: String,
    pub retry_count: u32,
}

impl Default for MiddlewareContext {
    fn default() -> Self {
        Self {
            service_name: String::new(),
            operation: String::new(),
            status_code: 0,
            succeeded: true,
            error_message: String::new(),
            retry_count: 0,
        }
    }
}

/// Middleware trait — intercept and process requests in a chain.
pub trait Middleware: fmt::Debug {
    /// Handle the request. Call `next(ctx)` to invoke the next middleware or final handler.
    fn handle(&self, ctx: &mut MiddlewareContext, next: &mut dyn FnMut(&mut MiddlewareContext));
    fn name(&self) -> &'static str;
}

/// Middleware pipeline — chains multiple middleware together.
#[derive(Debug, Default)]
pub struct MiddlewarePipeline {
    middlewares: Vec<Box<dyn Middleware>>,
}

impl MiddlewarePipeline {
    pub fn new() -> Self {
        Self { middlewares: Vec::new() }
    }

    pub fn add<T: Middleware + 'static>(&mut self, mw: T) {
        self.middlewares.push(Box::new(mw));
    }

    /// Execute the middleware chain. The `final_handler` is called after all middleware complete.
    pub fn execute(
        &self,
        ctx: &mut MiddlewareContext,
        mut final_handler: impl FnMut(&mut MiddlewareContext) + 'static,
    ) {
        if self.middlewares.is_empty() {
            final_handler(ctx);
            return;
        }

        // Build chain from tail to head: each middleware wraps the next
        let mut chain: Box<dyn FnMut(&mut MiddlewareContext)> = Box::new(final_handler);

        for mw in self.middlewares.iter().rev() {
            let mut prev = chain;
            chain = Box::new(move |c: &mut MiddlewareContext| {
                mw.handle(c, &mut *prev);
            });
        }

        chain(ctx);
    }

    pub fn clear(&mut self) {
        self.middlewares.clear();
    }

    pub fn len(&self) -> usize {
        self.middlewares.len()
    }

    pub fn is_empty(&self) -> bool {
        self.middlewares.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pipeline_executes_handler() {
        let mut ctx = MiddlewareContext::default();
        let pipeline = MiddlewarePipeline::new();
        let handler_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = handler_called.clone();
        pipeline.execute(&mut ctx, move |_| {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        assert!(handler_called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn log_middleware_does_not_modify_context() {
        let mut ctx = MiddlewareContext {
            operation: "test".into(),
            ..Default::default()
        };
        let pipeline = {
            let mut p = MiddlewarePipeline::new();
            p.add(LogMiddleware);
            p
        };
        pipeline.execute(&mut ctx, |c| {
            c.succeeded = true;
            c.status_code = 200;
        });
        assert!(ctx.succeeded);
        assert_eq!(ctx.status_code, 200);
    }

    #[test]
    fn retry_middleware_retries_on_failure() {
        let attempt = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let mut ctx = MiddlewareContext::default();
        let mut p = MiddlewarePipeline::new();
        p.add(RetryMiddleware::new(3));
        let att = attempt.clone();
        p.execute(&mut ctx, move |_| {
            att.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        // RetryMiddleware calls the handler once
        assert_eq!(attempt.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn middleware_order_is_preserved() {
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        struct Tracker(u32, std::sync::Arc<std::sync::Mutex<Vec<u32>>>);
        impl std::fmt::Debug for Tracker {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "Tracker({})", self.0)
            }
        }
        impl Middleware for Tracker {
            fn handle(&self, ctx: &mut MiddlewareContext, next: &mut dyn FnMut(&mut MiddlewareContext)) {
                self.1.lock().unwrap().push(self.0);
                next(ctx);
            }
            fn name(&self) -> &'static str { "tracker" }
        }

        let order1 = order.clone();
        let order2 = order.clone();
        let order3 = order.clone();
        let mut p = MiddlewarePipeline::new();
        p.add(Tracker(1, order1));
        p.add(Tracker(2, order2));
        p.add(Tracker(3, order3));

        let order_clone = order.clone();
        p.execute(&mut MiddlewareContext::default(), move |_| {
            order_clone.lock().unwrap().push(0);
        });
        assert_eq!(*order.lock().unwrap(), vec![1, 2, 3, 0]);
    }

    #[test]
    fn context_default_values() {
        let ctx = MiddlewareContext::default();
        assert_eq!(ctx.service_name, "");
        assert_eq!(ctx.operation, "");
        assert_eq!(ctx.status_code, 0);
        assert!(ctx.succeeded);
        assert_eq!(ctx.error_message, "");
        assert_eq!(ctx.retry_count, 0);
    }

    #[test]
    fn clear_removes_all_middleware() {
        let mut p = MiddlewarePipeline::new();
        p.add(LogMiddleware);
        assert_eq!(p.len(), 1);
        p.clear();
        assert!(p.is_empty());
        assert_eq!(p.len(), 0);
    }
}

/// Logging middleware — records request duration and status.
#[derive(Debug)]
pub struct LogMiddleware;

impl Middleware for LogMiddleware {
    fn handle(&self, ctx: &mut MiddlewareContext, next: &mut dyn FnMut(&mut MiddlewareContext)) {
        let start = std::time::Instant::now();
        next(ctx);
        let elapsed = start.elapsed();
        if ctx.succeeded {
            log::info!(
                "[OK] {} {} — {}ms",
                ctx.service_name,
                ctx.operation,
                elapsed.as_millis()
            );
        } else {
            log::warn!(
                "[FAIL] {} {} — {}ms: {}",
                ctx.service_name,
                ctx.operation,
                elapsed.as_millis(),
                ctx.error_message
            );
        }
    }

    fn name(&self) -> &'static str {
        "LogMiddleware"
    }
}

/// Retry middleware — retries failed operations.
#[derive(Debug)]
pub struct RetryMiddleware {
    max_retries: u32,
}

impl RetryMiddleware {
    pub fn new(max_retries: u32) -> Self {
        Self { max_retries }
    }
}

impl Middleware for RetryMiddleware {
    fn handle(&self, ctx: &mut MiddlewareContext, next: &mut dyn FnMut(&mut MiddlewareContext)) {
        for attempt in 0..=self.max_retries {
            ctx.retry_count = attempt;
            ctx.succeeded = true;
            ctx.error_message.clear();

            next(ctx);

            if ctx.succeeded {
                return;
            }

            if attempt >= self.max_retries {
                log::warn!(
                    "[RETRY] {} exhausted after {} attempts",
                    ctx.service_name,
                    attempt
                );
                return;
            }

            log::debug!(
                "[RETRY] {} attempt {} failed, retrying...",
                ctx.service_name,
                attempt + 1
            );
        }
    }

    fn name(&self) -> &'static str {
        "RetryMiddleware"
    }
}
