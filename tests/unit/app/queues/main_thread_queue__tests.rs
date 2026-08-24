    // 复用当前模块的队列、上下文与预算常量。
    use super::*;

    // 验证回调继续向同一队列投递时，单次 drain 仍会在固定预算处返回。
    #[test]
    // 执行主线程队列自投递公平性场景。
    fn self_posted_work_is_left_for_the_next_drain() {
        // 创建目标窗口独占的主线程队列。
        let queue = MainThreadQueue::new();
        // 克隆同一队列，供首个回调继续投递工作。
        let producer = queue.clone();
        // 首个回调在执行时再投递一个完整预算的任务。
        queue.enqueue(move || {
            // 生成足以跨越当前轮次预算的后续任务。
            for _ in 0..MAX_JOBS_PER_DRAIN {
                // 每个后续任务都为空操作，只观测队列调度语义。
                producer.enqueue(|| {});
            }
        });
        // 测试上下文不提交新根。
        let mut pending_root = None;
        // 测试上下文初始没有协调请求。
        let mut reconcile_pending = false;
        // 把窗口拥有的根与协调状态借给本轮任务。
        let mut context = MainThreadContext::new(&mut pending_root, &mut reconcile_pending);

        // 第一轮必须执行工作并在预算耗尽后返回。
        assert!(queue.drain(&mut context));
        // 首个回调加后续任务共超出预算一项，该项必须仍在队列中。
        assert_eq!(queue.len(), 1);
        // 下一轮继续执行保留的最后一项。
        assert!(queue.drain(&mut context));
        // 两轮结束后队列应完整清空且没有任务丢失。
        assert_eq!(queue.len(), 0);
    }

    // 批量取出后的回调 panic 不得丢弃同批尚未执行的任务。
    #[test]
    fn panicking_job_requeues_unexecuted_batch_in_fifo_order() {
        let queue = MainThreadQueue::new();
        let order = Arc::new(Mutex::new(Vec::new()));
        queue.enqueue(|| panic!("intentional main-thread job panic"));
        for value in [2, 3] {
            let order = Arc::clone(&order);
            queue.enqueue(move || {
                order
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .push(value);
            });
        }
        let mut pending_root = None;
        let mut reconcile_pending = false;
        let mut context = MainThreadContext::new(&mut pending_root, &mut reconcile_pending);

        let panic =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| queue.drain(&mut context)));
        assert!(panic.is_err());
        assert_eq!(queue.len(), 2);
        assert!(queue.drain(&mut context));
        assert_eq!(
            order
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .as_slice(),
            &[2, 3]
        );
    }

    // 回调内 clear 必须同时取消已批量取出但尚未执行的旧任务。
    #[test]
    fn clear_during_drain_discards_unexecuted_batch() {
        let queue = MainThreadQueue::new();
        let clearer = queue.clone();
        let calls = Arc::new(AtomicU64::new(0));
        queue.enqueue(move || clearer.clear());
        for _ in 0..2 {
            let calls = Arc::clone(&calls);
            queue.enqueue(move || {
                calls.fetch_add(1, Ordering::Relaxed);
            });
        }
        let mut pending_root = None;
        let mut reconcile_pending = false;
        let mut context = MainThreadContext::new(&mut pending_root, &mut reconcile_pending);

        assert!(queue.drain(&mut context));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(queue.len(), 0);
    }
