//! uix-services crate 集成测试。

use std::sync::atomic::{AtomicU32, Ordering};
use uix::services::file_service::FileService;
use uix::services::middleware::{LogMiddleware, MiddlewareContext, MiddlewarePipeline, RetryMiddleware, Middleware};
use uix::services::notification_service::{NotificationLevel, NotificationService};
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use std::fmt;

// ════════════════════════════════════════════════════════════════════════════
// file_service 测试
// ════════════════════════════════════════════════════════════════════════════

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_path(name: &str) -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("uix_fs_test_{}_{}", id, name));
    p.to_string_lossy().to_string()
}

#[test]
fn write_and_read_string() {
    let fs = FileService::new();
    let path = unique_path("hello.txt");
    fs.write_string(&path, "Hello UIX!").unwrap();
    assert!(fs.exists(&path));
    let content = fs.read_to_string(&path).unwrap();
    assert_eq!(content, "Hello UIX!");
    let _ = fs.remove(&path);
}

#[test]
fn write_and_read_bytes() {
    let fs = FileService::new();
    let path = unique_path("data.bin");
    let data = vec![0u8, 1, 2, 3, 255];
    fs.write_bytes(&path, &data).unwrap();
    let read = fs.read_bytes(&path).unwrap();
    assert_eq!(read, data);
    let _ = fs.remove(&path);
}

#[test]
fn append_string() {
    let fs = FileService::new();
    let path = unique_path("append.txt");
    fs.write_string(&path, "Line 1\n").unwrap();
    fs.append_string(&path, "Line 2\n").unwrap();
    let content = fs.read_to_string(&path).unwrap();
    assert_eq!(content, "Line 1\nLine 2\n");
    let _ = fs.remove(&path);
}

#[test]
fn read_lines() {
    let fs = FileService::new();
    let path = unique_path("lines.txt");
    fs.write_string(&path, "a\nb\nc").unwrap();
    let lines = fs.read_lines(&path).unwrap();
    assert_eq!(lines, vec!["a", "b", "c"]);
    let _ = fs.remove(&path);
}

#[test]
fn remove_file() {
    let fs = FileService::new();
    let path = unique_path("todelete.txt");
    fs.write_string(&path, "delete me").unwrap();
    assert!(fs.exists(&path));
    fs.remove(&path).unwrap();
    assert!(!fs.exists(&path));
}

#[test]
fn file_size_returns_correct_value() {
    let fs = FileService::new();
    let path = unique_path("size.txt");
    fs.write_string(&path, "12345").unwrap();
    let size = fs.file_size(&path).unwrap();
    assert_eq!(size, 5);
    let _ = fs.remove(&path);
}

#[test]
fn read_nonexistent_returns_error() {
    let fs = FileService::new();
    let result = fs.read_to_string(&format!(
        "/nonexistent/uix_test_{}",
        TEST_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    assert!(result.is_err());
}

#[test]
fn auto_creates_parent_directories() {
    let fs = FileService::new();
    let path = unique_path("sub/nested/test.txt");
    fs.write_string(&path, "nested").unwrap();
    assert!(fs.exists(&path));
    let _ = fs.remove(&path);
    let _ = std::fs::remove_dir_all(std::path::Path::new(&path).parent().unwrap());
}

#[test]
fn exists_returns_false_for_nonexistent() {
    let fs = FileService::new();
    assert!(!fs.exists("/nonexistent/uix_fs_nonexistent_file"));
}

#[test]
fn file_service_is_clone_and_copy() {
    let fs1 = FileService::new();
    let fs2 = fs1;
    let _fs3 = fs2;
}

// ════════════════════════════════════════════════════════════════════════════
// middleware 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn empty_pipeline_executes_handler() {
    let mut ctx = MiddlewareContext::default();
    let pipeline = MiddlewarePipeline::new();
    let handler_called = Arc::new(AtomicBool::new(false));
    let flag = handler_called.clone();
    pipeline.execute(&mut ctx, move |_| {
        flag.store(true, Ordering::SeqCst);
    });
    assert!(handler_called.load(Ordering::SeqCst));
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
    let attempt = Arc::new(AtomicU32::new(0));
    let mut ctx = MiddlewareContext::default();
    let mut p = MiddlewarePipeline::new();
    p.add(RetryMiddleware::new(3));
    let att = attempt.clone();
    p.execute(&mut ctx, move |_| {
        att.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(attempt.load(Ordering::SeqCst), 1);
}

#[test]
fn middleware_order_is_preserved() {
    let order = Arc::new(Mutex::new(Vec::new()));
    struct Tracker(u32, Arc<Mutex<Vec<u32>>>);
    impl fmt::Debug for Tracker {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

// ════════════════════════════════════════════════════════════════════════════
// notification_service 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_notify_adds_toast() {
    let mut svc = NotificationService::new();
    let id = svc.notify("Test", "Message", NotificationLevel::Info, 4000);
    assert!(svc.has_active());
    assert_eq!(svc.visible_toasts().len(), 1);
    assert_eq!(svc.visible_toasts()[0].id, id);
}

#[test]
fn test_dismiss_removes_toast() {
    let mut svc = NotificationService::new();
    let id = svc.notify("Test", "Message", NotificationLevel::Info, 4000);
    svc.dismiss(id);
    assert!(!svc.has_active());
}

#[test]
fn test_max_visible_trims() {
    let mut svc = NotificationService::new();
    svc.set_max_visible(3);
    for i in 0..5 {
        svc.notify(&format!("Title {}", i), "Msg", NotificationLevel::Info, 4000);
    }
    assert_eq!(svc.visible_toasts().len(), 3);
}
