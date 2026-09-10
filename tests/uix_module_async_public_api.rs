//! 真正 AOT/动态 AsyncCommand 与显式宿主完成合同的消费者回归。
#![cfg(feature = "uix-dynamic")]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;
use uix_app::app::modules::*;

const SOURCE: &str = include_str!("../examples/modules/async_bench.uix");
const WAIT: Duration = Duration::from_secs(5);
fn modules() -> Vec<Module> {
    vec![
        uix_app::uix_module!("examples/modules/async_bench.uix"),
        load_module(SOURCE, "async_bench.uix").unwrap(),
    ]
}
fn worker(
    module: Module,
    capacity: usize,
    limits: Limits,
) -> (ModuleWorker, mpsc::Receiver<(String, AsyncCall)>) {
    let (send, receive) = mpsc::channel();
    let ports = module
        .ports
        .iter()
        .map(|signature| {
            let name = signature.name.clone();
            let sender = send.clone();
            let signature = signature.clone();
            (
                name.clone(),
                AsyncHostPort {
                    signature,
                    start: Arc::new(move |call| {
                        sender.send((name.clone(), call)).map_err(|_| {
                            RuntimeError::new(ErrorKind::HostFailure, "合成宿主已退出")
                        })
                    }),
                },
            )
        })
        .collect();
    let instance = Instance::new_with_async_ports(module, HostPorts::new(), ports, limits).unwrap();
    (
        ModuleWorker::spawn(instance, capacity, || {}).unwrap(),
        receive,
    )
}
fn text(value: &str) -> Value {
    Value::String(value.into())
}

#[test]
fn same_source_aot_and_dynamic_suspend_commit_branches_and_resume_without_replay() {
    let mut observed = Vec::new();
    for module in modules() {
        let (owner, endpoint) = worker(module, 4, Limits::default());
        let handle = owner.handle();
        let request = handle.call("process", vec![text("input")]).unwrap();
        let sequence = request.sequence;
        let (port, read) = endpoint.recv_timeout(WAIT).unwrap();
        assert_eq!(port, "read");
        assert_eq!(read.arguments, vec![text("input")]);
        let first = handle.snapshot();
        assert_eq!(first.state["phase"], text("reading"));
        assert_eq!(first.state["draft"], text("initial"));
        assert!(first.pending_sequences.contains(&sequence));
        // 在端口仍挂起时，合法公开查询必须真正完成。
        assert_eq!(
            handle.call("current", vec![]).unwrap().wait(WAIT).unwrap(),
            text("initial")
        );
        read.completion.complete(Ok(text("NORMALIZED"))).unwrap();
        let (port, commit) = endpoint.recv_timeout(WAIT).unwrap();
        assert_eq!(port, "commit");
        assert_eq!(commit.arguments, vec![text("NORMALIZED")]);
        assert_eq!(commit.operation.instance, read.operation.instance);
        assert_eq!(commit.operation.request, sequence);
        assert_eq!(
            (read.operation.await_ordinal, commit.operation.await_ordinal),
            (1, 2)
        );
        assert_eq!(handle.snapshot().state["draft"], text("NORMALIZED"));
        commit
            .completion
            .complete(Ok(Value::Result(Ok(Box::new(Value::Bool(true))))))
            .unwrap();
        assert_eq!(request.wait(WAIT).unwrap(), text("NORMALIZED"));
        let finished = handle.snapshot();
        assert!(finished.pending_sequences.is_empty());
        assert_eq!(finished.state["phase"], text("done"));
        assert_eq!(
            read.completion
                .complete(Ok(text("duplicate")))
                .unwrap_err()
                .kind,
            ErrorKind::Conflict
        );
        let empty = handle.call("process", vec![text("")]).unwrap();
        let (_, call) = endpoint.recv_timeout(WAIT).unwrap();
        call.completion.complete(Ok(text(""))).unwrap();
        assert_eq!(empty.wait(WAIT).unwrap(), text(""));
        assert_eq!(handle.snapshot().state["phase"], text("empty"));
        assert_eq!(
            handle
                .call("early", vec![Value::Bool(true)])
                .unwrap()
                .wait(WAIT)
                .unwrap(),
            text("skipped")
        );
        let unit = handle.call("waitOnly", vec![]).unwrap();
        let (_, call) = endpoint.recv_timeout(WAIT).unwrap();
        assert_eq!(call.arguments, vec![text("unit")]);
        call.completion.complete(Ok(text("ignored"))).unwrap();
        assert_eq!(unit.wait(WAIT).unwrap(), Value::Unit);
        assert!(endpoint.try_recv().is_err());
        observed.push((
            first.state,
            finished.state,
            finished.revision,
            handle.snapshot().state,
        ));
        owner.close(WAIT).unwrap();
    }
    assert_eq!(observed[0], observed[1]);
}

#[test]
fn business_result_and_technical_failure_have_distinct_partial_commit_semantics() {
    for module in modules() {
        let (owner, endpoint) = worker(module, 2, Limits::default());
        let handle = owner.handle();
        let business = handle.call("process", vec![text("input")]).unwrap();
        endpoint
            .recv_timeout(WAIT)
            .unwrap()
            .1
            .completion
            .complete(Ok(text("kept")))
            .unwrap();
        endpoint
            .recv_timeout(WAIT)
            .unwrap()
            .1
            .completion
            .complete(Ok(Value::Result(Err(Box::new(text("拒绝"))))))
            .unwrap();
        assert_eq!(business.wait(WAIT).unwrap(), text("declined"));
        assert_eq!(handle.snapshot().state["draft"], text("kept"));
        assert!(handle.snapshot().error.is_none());
        let technical = handle.call("process", vec![text("bad")]).unwrap();
        let (_, call) = endpoint.recv_timeout(WAIT).unwrap();
        call.completion
            .complete(Err(RuntimeError::new(
                ErrorKind::HostFailure,
                "真实的宿主失败出口",
            )))
            .unwrap();
        let error = technical.wait(WAIT).unwrap_err();
        assert_eq!(error.kind, ErrorKind::HostFailure);
        assert!(error.location.source.ends_with("async_bench.uix"));
        assert!(error.location.line > 0);
        assert_eq!(handle.snapshot().state["phase"], text("reading"));
        assert_eq!(handle.snapshot().state["draft"], text("kept"));
        owner.close(WAIT).unwrap();
    }
}

#[test]
fn resumed_state_revision_conflict_preserves_new_authority_and_never_replays_io() {
    for module in modules() {
        let (owner, endpoint) = worker(module, 2, Limits::default());
        let handle = owner.handle();
        let request = handle.call("process", vec![text("old")]).unwrap();
        let (_, call) = endpoint.recv_timeout(WAIT).unwrap();
        handle
            .call("edit", vec![text("new authority")])
            .unwrap()
            .wait(WAIT)
            .unwrap();
        call.completion.complete(Ok(text("stale result"))).unwrap();
        assert_eq!(request.wait(WAIT).unwrap_err().kind, ErrorKind::Conflict);
        assert_eq!(handle.snapshot().state["draft"], text("new authority"));
        assert!(endpoint.try_recv().is_err());
        owner.close(WAIT).unwrap();
    }
}

#[test]
fn cancellation_timeout_revocation_close_and_replacement_reject_late_completion() {
    for (case, kind) in [
        ("cancel", ErrorKind::Cancelled),
        ("timeout", ErrorKind::Timeout),
        ("revoke", ErrorKind::CapabilityDenied),
        ("close", ErrorKind::Closed),
        ("replace", ErrorKind::Conflict),
    ] {
        let module = load_module(SOURCE, "async_bench.uix").unwrap();
        let limits = Limits {
            timeout: if case == "timeout" {
                Duration::from_millis(60)
            } else {
                Duration::from_secs(3)
            },
            ..Limits::default()
        };
        let (owner, endpoint) = worker(module, 2, limits);
        let handle = owner.handle();
        let request = handle.call("process", vec![text(case)]).unwrap();
        let (_, call) = endpoint.recv_timeout(WAIT).unwrap();
        match case {
            "cancel" => request.cancellation.cancel(),
            "revoke" => handle.revoke(),
            "close" => handle.close(),
            "replace" => {
                handle
                    .replace(
                        load_module(&SOURCE.replace("version=\"1\"", "version=\"2\""), "v2.uix")
                            .unwrap(),
                        1,
                    )
                    .unwrap()
                    .wait(WAIT)
                    .unwrap();
            }
            _ => {}
        }
        assert_eq!(request.wait(WAIT).unwrap_err().kind, kind, "{case}");
        assert!(call.cancellation.is_cancelled());
        assert_eq!(
            call.completion.complete(Ok(text("late"))).unwrap_err().kind,
            ErrorKind::Cancelled
        );
        assert!(endpoint.try_recv().is_err());
        if !matches!(case, "revoke" | "close") {
            assert_eq!(handle.snapshot().state["draft"], text("initial"));
        }
        owner.close(WAIT).unwrap();
        assert!(!handle.snapshot().active);
        assert!(handle.snapshot().state.is_empty());
    }
}

#[test]
fn completion_drop_type_failure_and_inflight_capacity_are_bounded() {
    let (owner, endpoint) = worker(
        load_module(SOURCE, "async_bench.uix").unwrap(),
        1,
        Limits::default(),
    );
    let handle = owner.handle();
    let first = handle.call("process", vec![text("held")]).unwrap();
    let (_, call) = endpoint.recv_timeout(WAIT).unwrap();
    assert_eq!(
        handle
            .call("early", vec![Value::Bool(false)])
            .unwrap()
            .wait(WAIT)
            .unwrap_err()
            .kind,
        ErrorKind::Quota
    );
    assert_eq!(
        handle.call("current", vec![]).unwrap().wait(WAIT).unwrap(),
        text("initial")
    );
    drop(call);
    assert_eq!(first.wait(WAIT).unwrap_err().kind, ErrorKind::HostFailure);
    let wrong = handle.call("process", vec![text("wrong")]).unwrap();
    let (_, call) = endpoint.recv_timeout(WAIT).unwrap();
    // 完成回执确认交回，业务端收到类型化失败而不是错误类型的成功值。
    call.completion.complete(Ok(Value::Int(17))).unwrap();
    assert_eq!(wrong.wait(WAIT).unwrap_err().kind, ErrorKind::Type);
    owner.close(WAIT).unwrap();
}

#[test]
fn step_budget_is_not_reset_at_each_await() {
    let source = r#"<Module name="TotalBudget" version="1" schema="1">
        <Port name="one" returns="Int" effect="query" async="true"/>
        <AsyncCommand name="run" returns="Int" body="do { let a = await one(); let b = await one(); let c = await one(); let d = await one(); return a + b + c + d; }"/>
    </Module>"#;
    let module = load_module(source, "total-budget.uix").unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let ports = AsyncHostPorts::from([(
        "one".into(),
        AsyncHostPort {
            signature: module.ports[0].clone(),
            start: Arc::new(move |call| {
                calls.fetch_add(1, Ordering::Relaxed);
                call.completion.complete(Ok(Value::Int(1)))
            }),
        },
    )]);
    let instance = Instance::new_with_async_ports(
        module,
        HostPorts::new(),
        ports,
        Limits {
            steps: 6,
            ..Limits::default()
        },
    )
    .unwrap();
    let owner = ModuleWorker::spawn(instance, 1, || {}).unwrap();
    let error = owner
        .handle()
        .call("run", vec![])
        .unwrap()
        .wait(WAIT)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Quota);
    assert!(
        (1..4).contains(&count.load(Ordering::Relaxed)),
        "已启动端口数：{}",
        count.load(Ordering::Relaxed)
    );
    owner.close(WAIT).unwrap();
}

#[test]
fn failing_resumed_fragment_rolls_back_only_its_uncommitted_state() {
    for module in modules() {
        let (owner, endpoint) = worker(module, 2, Limits::default());
        let handle = owner.handle();
        let request = handle.call("failAfterRead", vec![]).unwrap();
        let (_, call) = endpoint.recv_timeout(WAIT).unwrap();
        call.completion
            .complete(Ok(text("must not commit")))
            .unwrap();
        assert_eq!(request.wait(WAIT).unwrap_err().kind, ErrorKind::Arithmetic);
        assert_eq!(handle.snapshot().state["draft"], text("initial"));
        assert_eq!(handle.snapshot().state["phase"], text("reading"));
        assert_eq!(handle.snapshot().revision, 1);
        owner.close(WAIT).unwrap();
    }
}

#[test]
fn start_failure_and_panic_are_final_and_host_captures_release_at_shutdown() {
    for panic in [false, true] {
        let module = load_module(SOURCE, "async_bench.uix").unwrap();
        let resource = Arc::new(AtomicUsize::new(0));
        let weak = Arc::downgrade(&resource);
        let ports = module
            .ports
            .iter()
            .map(|signature| {
                let resource = resource.clone();
                (
                    signature.name.clone(),
                    AsyncHostPort {
                        signature: signature.clone(),
                        start: Arc::new(move |_| {
                            resource.fetch_add(1, Ordering::Relaxed);
                            if panic {
                                panic!("合成异步启动故障");
                            }
                            Err(RuntimeError::new(ErrorKind::HostFailure, "合成启动拒绝"))
                        }),
                    },
                )
            })
            .collect();
        let instance =
            Instance::new_with_async_ports(module, HostPorts::new(), ports, Limits::default())
                .unwrap();
        let owner = ModuleWorker::spawn(instance, 1, || {}).unwrap();
        let handle = owner.handle();
        assert_eq!(
            handle
                .call("process", vec![text("once")])
                .unwrap()
                .wait(WAIT)
                .unwrap_err()
                .kind,
            ErrorKind::HostFailure
        );
        assert_eq!(resource.load(Ordering::Relaxed), 1);
        drop(resource);
        owner.close(WAIT).unwrap();
        assert!(weak.upgrade().is_none());
        assert!(handle.snapshot().state.is_empty());
    }
}

#[test]
fn declared_async_capability_and_call_shapes_are_checked_before_execution() {
    let module = load_module(SOURCE, "async_bench.uix").unwrap();
    assert!(matches!(
        Instance::new(module, HostPorts::new(), Limits::default()),
        Err(RuntimeError {
            kind: ErrorKind::CapabilityDenied,
            ..
        })
    ));
    for declaration in [
        r#"<Port name="read" returns="String" effect="query"/><AsyncCommand name="run" returns="String" body="do { let result = await read(); return result; }"/>"#,
        r#"<Port name="read" returns="String" effect="query" async="true"/><Command name="run" returns="String" body="do { let result = await read(); return result; }"/>"#,
        r#"<Port name="read" returns="String" effect="query" async="true"/><AsyncCommand name="run" returns="String" body="read()"/>"#,
        r#"<Port name="read" returns="String" effect="query" async="true"/><AsyncCommand name="run" returns="String" body="do { if true { let result = await read(); } return result; }"/>"#,
    ] {
        let source =
            format!(r#"<Module name="AsyncErrors" version="1" schema="1">{declaration}</Module>"#);
        let dynamic = load_module(&source, "async-errors.uix").unwrap_err();
        let aot =
            uix_lang_compiler::modules::compile_inline(&source, "async-errors.uix").unwrap_err();
        assert_eq!(dynamic.code, aot.code);
        assert_eq!(dynamic.message, aot.message);
        assert!(dynamic.line > 0);
    }
    let (owner, _endpoint) = worker(
        load_module(SOURCE, "async_bench.uix").unwrap(),
        2,
        Limits::default(),
    );
    assert_eq!(
        owner
            .handle()
            .call("", vec![])
            .unwrap()
            .wait(WAIT)
            .unwrap_err()
            .kind,
        ErrorKind::UnknownCommand
    );
    assert_eq!(
        owner
            .handle()
            .call("process", vec![Value::Int(4)])
            .unwrap()
            .wait(WAIT)
            .unwrap_err()
            .kind,
        ErrorKind::Argument
    );
    owner.close(WAIT).unwrap();
}

#[cfg(feature = "test-harness")]
fn wait_snapshot(
    handle: &ModuleHandle,
    notifications: &mpsc::Receiver<()>,
    predicate: impl Fn(&ModuleSnapshot) -> bool,
) {
    let deadline = std::time::Instant::now() + WAIT;
    loop {
        if predicate(&handle.snapshot()) {
            return;
        }
        notifications
            .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .unwrap();
    }
}

#[cfg(feature = "test-harness")]
#[test]
fn async_button_event_uses_worker_without_blocking_native_view_projection() {
    use uix_app::ui::test_harness::TestApp;
    for module in modules() {
        let (send, endpoint) = mpsc::channel::<(String, AsyncCall)>();
        let ports = module
            .ports
            .iter()
            .map(|signature| {
                let send = send.clone();
                let name = signature.name.clone();
                (
                    name.clone(),
                    AsyncHostPort {
                        signature: signature.clone(),
                        start: Arc::new(move |call| {
                            send.send((name.clone(), call)).unwrap();
                            Ok(())
                        }),
                    },
                )
            })
            .collect();
        let instance =
            Instance::new_with_async_ports(module, HostPorts::new(), ports, Limits::default())
                .unwrap();
        let (send, notifications) = mpsc::channel();
        let (owner, view) = ModuleView::spawn(instance, 2, move || {
            let _ = send.send(());
        })
        .unwrap();
        let handle = view.handle();
        let mut app = TestApp::new((480.0, 300.0), move || view.project().unwrap());
        app.invoke("AsyncBench/process").unwrap();
        let (port, call) = endpoint.recv_timeout(WAIT).unwrap();
        assert_eq!(port, "read");
        assert_eq!(call.arguments, vec![text("initial")]);
        app.settle().unwrap();
        assert_eq!(app.text("AsyncBench/phase").unwrap(), "reading");
        call.completion
            .complete(Ok(text("from async event")))
            .unwrap();
        let (_, call) = endpoint.recv_timeout(WAIT).unwrap();
        call.completion
            .complete(Ok(Value::Result(Ok(Box::new(Value::Bool(true))))))
            .unwrap();
        wait_snapshot(&handle, &notifications, |snapshot| {
            snapshot.pending_sequences.is_empty() && snapshot.state["phase"] == text("done")
        });
        app.settle().unwrap();
        assert_eq!(app.text("AsyncBench/input").unwrap(), "from async event");
        owner.close(WAIT).unwrap();
    }
}

#[cfg(feature = "test-harness")]
#[test]
fn out_of_order_terminal_sequence_does_not_acknowledge_pending_input() {
    use uix_app::ui::test_harness::TestApp;
    let source = r#"<Module name="AsyncInput" version="1" schema="1">
        <State name="draft" type="String" value={'initial'}/>
        <Port name="write" params="value: String" returns="String" effect="command" async="true"/>
        <AsyncCommand name="edit" params="value: String" returns="Unit" body="do { let result = await write(value); setState(draft: result); }"/>
        <Query name="current" returns="String" body="draft"/>
        <View><Input key="input" value={draft} onChange={edit($event.value)}/></View>
    </Module>"#;
    let module = load_module(source, "async-input.uix").unwrap();
    let (send, endpoint) = mpsc::channel::<AsyncCall>();
    let ports = AsyncHostPorts::from([(
        "write".into(),
        AsyncHostPort {
            signature: module.ports[0].clone(),
            start: Arc::new(move |call| {
                send.send(call).unwrap();
                Ok(())
            }),
        },
    )]);
    let instance =
        Instance::new_with_async_ports(module, HostPorts::new(), ports, Limits::default()).unwrap();
    let (send, notifications) = mpsc::channel();
    let (owner, view) = ModuleView::spawn(instance, 2, move || {
        let _ = send.send(());
    })
    .unwrap();
    let handle = view.handle();
    let mut app = TestApp::new((480.0, 100.0), move || view.project().unwrap());
    app.set_value("AsyncInput/input", "pending draft").unwrap();
    let call = endpoint.recv_timeout(WAIT).unwrap();
    let later = handle.call("current", vec![]).unwrap();
    assert!(later.sequence > call.operation.request);
    assert_eq!(later.wait(WAIT).unwrap(), text("initial"));
    assert!(
        handle
            .snapshot()
            .pending_sequences
            .contains(&call.operation.request)
    );
    app.settle().unwrap();
    assert_eq!(app.text("AsyncInput/input").unwrap(), "pending draft");
    call.completion.complete(Ok(text("ACKNOWLEDGED"))).unwrap();
    wait_snapshot(&handle, &notifications, |snapshot| {
        snapshot.pending_sequences.is_empty()
    });
    app.settle().unwrap();
    assert_eq!(app.text("AsyncInput/input").unwrap(), "ACKNOWLEDGED");
    owner.close(WAIT).unwrap();
}

#[test]
fn competing_completers_accept_exactly_one_owned_result() {
    for module in modules() {
        let (owner, endpoint) = worker(module, 2, Limits::default());
        let request = owner
            .handle()
            .call("early", vec![Value::Bool(false)])
            .unwrap();
        let (_, call) = endpoint.recv_timeout(WAIT).unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let threads: Vec<_> = ["first", "second"]
            .into_iter()
            .map(|value| {
                let completion = call.completion.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    (value, completion.complete(Ok(text(value))))
                })
            })
            .collect();
        let results: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        let accepted: Vec<_> = results
            .iter()
            .filter(|(_, result)| result.is_ok())
            .collect();
        assert_eq!(accepted.len(), 1);
        assert_eq!(request.wait(WAIT).unwrap(), text(accepted[0].0));
        assert!(
            results
                .iter()
                .filter_map(|(_, result)| result.as_ref().err())
                .all(|error| error.kind == ErrorKind::Conflict)
        );
        owner.close(WAIT).unwrap();
    }
}

#[test]
fn retained_late_completion_does_not_retain_instance_host_resources() {
    let module = load_module(SOURCE, "async_bench.uix").unwrap();
    let resource = Arc::new(());
    let weak = Arc::downgrade(&resource);
    let (send, endpoint) = mpsc::channel();
    let ports = module
        .ports
        .iter()
        .map(|signature| {
            let resource = resource.clone();
            let send = send.clone();
            (
                signature.name.clone(),
                AsyncHostPort {
                    signature: signature.clone(),
                    start: Arc::new(move |call| {
                        let _lease = &resource;
                        send.send(call).unwrap();
                        Ok(())
                    }),
                },
            )
        })
        .collect();
    let instance =
        Instance::new_with_async_ports(module, HostPorts::new(), ports, Limits::default()).unwrap();
    drop(resource);
    let owner = ModuleWorker::spawn(instance, 1, || {}).unwrap();
    let handle = owner.handle();
    let request = handle.call("process", vec![text("retained")]).unwrap();
    let call = endpoint.recv_timeout(WAIT).unwrap();
    owner.close(WAIT).unwrap();
    assert_eq!(request.wait(WAIT).unwrap_err().kind, ErrorKind::Closed);
    assert!(weak.upgrade().is_none());
    assert_eq!(
        call.completion.complete(Ok(text("late"))).unwrap_err().kind,
        ErrorKind::Cancelled
    );
    assert!(handle.snapshot().pending_sequences.is_empty());
}

#[test]
fn oversized_completion_payload_becomes_quota_failure_without_retaining_business_state() {
    let (owner, endpoint) = worker(
        load_module(SOURCE, "async_bench.uix").unwrap(),
        1,
        Limits::default(),
    );
    let handle = owner.handle();
    let request = handle.call("early", vec![Value::Bool(false)]).unwrap();
    let (_, call) = endpoint.recv_timeout(WAIT).unwrap();
    call.completion
        .complete(Ok(Value::String("x".repeat(1_048_577))))
        .unwrap();
    assert_eq!(request.wait(WAIT).unwrap_err().kind, ErrorKind::Quota);
    assert_eq!(handle.snapshot().state["draft"], text("initial"));
    owner.close(WAIT).unwrap();
}

#[test]
fn invalid_candidate_view_prevents_async_external_start() {
    let source = r#"<Module name="BeforeStart" version="1" schema="1">
        <State name="spacing" type="Float" value={8}/>
        <Port name="read" returns="String" effect="query" async="true"/>
        <AsyncCommand name="bad" returns="Unit" body="do { setState(spacing: -1); let ignored = await read(); }"/>
        <View><Column key="root" gap={spacing}/></View>
    </Module>"#;
    let (owner, endpoint) = worker(
        load_module(source, "before-start.uix").unwrap(),
        1,
        Limits::default(),
    );
    let handle = owner.handle();
    let before = handle.snapshot();
    assert_eq!(
        handle
            .call("bad", vec![])
            .unwrap()
            .wait(WAIT)
            .unwrap_err()
            .kind,
        ErrorKind::Type
    );
    let after = handle.snapshot();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.state, before.state);
    assert_eq!(after.view, before.view);
    assert!(endpoint.try_recv().is_err());
    owner.close(WAIT).unwrap();
}

#[test]
fn replacement_receipt_revokes_all_previous_generation_completion_slots() {
    let (owner, endpoint) = worker(
        load_module(SOURCE, "async_bench.uix").unwrap(),
        16,
        Limits::default(),
    );
    let handle = owner.handle();
    let mut requests = Vec::new();
    let mut calls = Vec::new();
    for _ in 0..16 {
        requests.push(handle.call("early", vec![Value::Bool(false)]).unwrap());
        calls.push(endpoint.recv_timeout(WAIT).unwrap().1);
    }
    let candidate =
        load_module(&SOURCE.replace("version=\"1\"", "version=\"2\""), "v2.uix").unwrap();
    handle.replace(candidate, 1).unwrap().wait(WAIT).unwrap();
    assert!(handle.snapshot().pending_sequences.is_empty());
    for call in calls {
        assert!(call.cancellation.is_cancelled());
        assert_eq!(
            call.completion.complete(Ok(text("late"))).unwrap_err().kind,
            ErrorKind::Cancelled
        );
    }
    for request in requests {
        assert_eq!(request.wait(WAIT).unwrap_err().kind, ErrorKind::Conflict);
    }
    assert_eq!(handle.snapshot().generation, 2);
    owner.close(WAIT).unwrap();
}
