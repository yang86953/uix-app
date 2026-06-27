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
