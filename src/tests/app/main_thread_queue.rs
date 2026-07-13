use crate::tests::common::*;
use crate::ui::view::ViewNode;
use crate::app::main_thread_queue::*;
use crate::ui::view::combinators::label;
use crate::ui::view::ViewAdapter;
use crate::ui::widgets::Label;

fn drain_with_empty_context(
    queue: &MainThreadQueue,
) -> (bool, Option<crate::ui::view::ViewNode>, bool) {
    let mut pending_root = None;
    let mut reconcile_pending = false;
    let mut context = MainThreadContext::new(&mut pending_root, &mut reconcile_pending);
    let ran = queue.drain(&mut context);
    (ran, pending_root, reconcile_pending)
}

#[test]
fn drain_runs_jobs_fifo() {
    let queue = MainThreadQueue::new();
    let out = Arc::new(Mutex::new(Vec::new()));
    queue.enqueue({
        let out = out.clone();
        move || out.lock().unwrap_or_else(|e| e.into_inner()).push(1)
    });
    queue.enqueue({
        let out = out.clone();
        move || out.lock().unwrap_or_else(|e| e.into_inner()).push(2)
    });

    assert!(drain_with_empty_context(&queue).0);
    assert_eq!(*out.lock().unwrap_or_else(|e| e.into_inner()), vec![1, 2]);
    assert!(queue.is_empty());
}

#[test]
fn drain_runs_jobs_enqueued_by_jobs_in_same_drain() {
    let queue = MainThreadQueue::new();
    let out = Arc::new(Mutex::new(Vec::new()));
    queue.enqueue({
        let out = out.clone();
        let queue = queue.clone();
        move || {
            out.lock().unwrap_or_else(|e| e.into_inner()).push(1);
            queue.enqueue({
                let out = out.clone();
                move || out.lock().unwrap_or_else(|e| e.into_inner()).push(2)
            });
        }
    });

    assert!(drain_with_empty_context(&queue).0);
    assert_eq!(*out.lock().unwrap_or_else(|e| e.into_inner()), vec![1, 2]);
    assert!(queue.is_empty());
}

#[test]
fn context_update_root_keeps_last_root_and_marks_reconcile_pending() {
    let queue = MainThreadQueue::new();
    queue.enqueue_with_context(|ctx| ctx.update_root(label("first")));
    queue.enqueue_with_context(|ctx| ctx.update_root(label("second")));

    let (ran, pending_root, reconcile_pending) = drain_with_empty_context(&queue);
    let tree = ViewAdapter::build_nodes(pending_root.unwrap());
    let label = tree
        .root()
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap();

    assert!(ran);
    assert!(reconcile_pending);
    assert_eq!(label.text(), "second");
}
