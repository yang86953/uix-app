//! 可移植模块 View、提交原子性和公开 worker 生命周期的消费者回归。
#![cfg(feature = "uix-dynamic")]

use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::Duration;
use uix::app::modules::*;

const LIST: &str = include_str!("../examples/modules/dynamic_list.uix");
const WAIT: Duration = Duration::from_secs(5);

fn instances() -> Vec<Instance> {
    [
        uix::uix_module!("examples/modules/dynamic_list.uix"),
        load_module(LIST, "dynamic_list.uix").unwrap(),
    ]
    .into_iter()
    .map(|module| Instance::new(module, HostPorts::new(), Limits::default()).unwrap())
    .collect()
}
fn find<'a>(node: &'a ViewSnapshot, key: &str) -> Option<&'a ViewSnapshot> {
    if node.key == key {
        Some(node)
    } else {
        node.children.iter().find_map(|child| find(child, key))
    }
}
fn entry(id: &str, title: &str, enabled: bool) -> Value {
    Value::Record(BTreeMap::from([
        ("id".into(), Value::String(id.into())),
        ("title".into(), Value::String(title.into())),
        ("enabled".into(), Value::Bool(enabled)),
        ("parts".into(), Value::Array(vec![Value::Int(1)])),
    ]))
}

#[test]
fn aot_and_dynamic_lists_keep_identity_and_route_current_owned_captures() {
    let mut observed = Vec::new();
    for mut instance in instances() {
        let initial = instance.view().unwrap().unwrap();
        let alpha = "choose/s616c706861";
        let beta = "choose/s62657461";
        assert_eq!(
            find(&initial, alpha).unwrap().properties["text"],
            Value::String("甲".into())
        );
        instance
            .event("part/s616c706861/i2", 1, &[], &Cancellation::default())
            .unwrap();
        assert_eq!(
            instance.state()["selected"],
            Value::String("alpha:2".into())
        );
        let error = instance
            .event(beta, 1, &[], &Cancellation::default())
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::CapabilityDenied);
        instance
            .call(
                "setEntries",
                &[Value::Array(vec![
                    entry("beta", "更新乙", true),
                    entry("alpha", "更新甲", true),
                ])],
            )
            .unwrap();
        let reordered = instance.view().unwrap().unwrap();
        assert_eq!(reordered.children[0].key, "entry/s62657461");
        assert_eq!(
            find(&reordered, "position/s616c706861").unwrap().properties["text"],
            Value::String("1".into())
        );
        instance
            .event(alpha, 1, &[], &Cancellation::default())
            .unwrap();
        assert_eq!(
            instance.state()["selected"],
            Value::String("alpha:更新甲".into())
        );
        instance
            .call(
                "setEntries",
                &[Value::Array(vec![entry("beta", "乙", true)])],
            )
            .unwrap();
        assert_eq!(
            instance
                .event(alpha, 1, &[], &Cancellation::default())
                .unwrap_err()
                .kind,
            ErrorKind::UnknownCommand
        );
        instance.call("setPhase", &[Value::Int(1)]).unwrap();
        assert!(find(&instance.view().unwrap().unwrap(), "alternate").is_some());
        assert_eq!(
            instance
                .event(beta, 1, &[], &Cancellation::default())
                .unwrap_err()
                .kind,
            ErrorKind::UnknownCommand
        );
        instance.call("setPhase", &[Value::Int(2)]).unwrap();
        let hidden = instance.view().unwrap().unwrap();
        assert!(find(&hidden, "empty").is_some());
        assert!(find(&hidden, "alternate").is_none());
        observed.push((
            initial,
            reordered,
            hidden,
            instance.state(),
            instance.revision(),
        ));
    }
    assert_eq!(observed[0], observed[1]);
}

#[test]
fn invalid_layout_duplicate_keys_and_incompatible_replacement_keep_committed_view() {
    for mut instance in instances() {
        let original = instance.view().unwrap();
        for values in [vec![
            entry("duplicate", "a", true),
            entry("duplicate", "b", true),
        ]] {
            assert_eq!(
                instance
                    .call("setEntries", &[Value::Array(values)])
                    .unwrap_err()
                    .kind,
                ErrorKind::Conflict
            );
            assert_eq!(instance.view().unwrap(), original);
            assert_eq!(instance.revision(), 0);
        }
        assert_eq!(
            instance
                .call("setSpacing", &[Value::Float(-1.0)])
                .unwrap_err()
                .kind,
            ErrorKind::Type
        );
        assert_eq!(instance.view().unwrap(), original);
        assert_eq!(instance.state()["spacing"], Value::Float(8.0));
        let invalid = load_module(
            &LIST.replace("gap={spacing}", "gap={-1}"),
            "invalid-view.uix",
        )
        .unwrap();
        assert!(instance.replace(invalid, 1).is_err());
        assert_eq!(instance.view().unwrap(), original);
        assert_eq!(instance.generation(), 1);
        instance
            .replace(
                load_module(&LIST.replace("version=\"1\"", "version=\"2\""), "v2.uix").unwrap(),
                1,
            )
            .unwrap();
        assert_eq!(
            instance
                .event("choose/s616c706861", 1, &[], &Cancellation::default())
                .unwrap_err()
                .kind,
            ErrorKind::Conflict
        );
        instance
            .event("choose/s616c706861", 2, &[], &Cancellation::default())
            .unwrap();
    }
}

#[test]
fn expansion_uses_aggregate_value_budget_and_failed_update_is_atomic() {
    let source = r#"<Module name="Bounded" version="1" schema="1">
        <State name="values" type="Array<Int>" value={[]}/>
        <Command name="set" params="value: Array<Int>" returns="Unit" body="do { setState(values: value); }"/>
        <View><Column key="root"><For {item} in {values} key={item}><Text key="value" text="1234567890"/></For></Column></View>
    </Module>"#;
    let module = load_module(source, "bounded.uix").unwrap();
    let mut instance = Instance::new(
        module,
        HostPorts::new(),
        Limits {
            value_bytes: 4096,
            ..Limits::default()
        },
    )
    .unwrap();
    instance
        .call("set", &[Value::Array((0..5).map(Value::Int).collect())])
        .unwrap();
    let before = (
        instance.state(),
        instance.view().unwrap(),
        instance.revision(),
    );
    assert_eq!(
        instance
            .call("set", &[Value::Array((0..40).map(Value::Int).collect())])
            .unwrap_err()
            .kind,
        ErrorKind::Quota
    );
    assert_eq!(
        (
            instance.state(),
            instance.view().unwrap(),
            instance.revision()
        ),
        before
    );
}

#[test]
fn compiler_rejects_unstable_or_impure_structural_bindings_in_both_entries() {
    for body in [
        r#"<For {item} in {[1, 2]}><Text key="text" text={item.toString()}/></For>"#,
        r#"<For {item} in {[1, 2]} key={true}><Text key="text" text="bad"/></For>"#,
        r#"<For {item} in {3} key={item}><Text key="text" text="bad"/></For>"#,
        r#"<If {1}><Text key="text" text="bad"/></If>"#,
        r#"<Else><Text key="text" text="bad"/></Else>"#,
        r#"<Text key="text" text={host()}/>"#,
    ] {
        let source = format!(
            r#"<Module name="BadView" version="1" schema="1"><Port name="host" returns="String" effect="query"/><View><Column key="root">{body}</Column></View></Module>"#
        );
        let dynamic = load_module(&source, "bad-view.uix").unwrap_err();
        let aot = uix_lang_compiler::modules::compile_inline(&source, "bad-view.uix").unwrap_err();
        assert_eq!(dynamic.code, aot.code);
        assert_eq!(dynamic.message, aot.message);
        assert_eq!(dynamic.source_name, "bad-view.uix");
    }
}

const INPUT: &str = r#"<Module name="InputFlow" version="1" schema="1">
    <State name="draft" type="String" value={'initial'}/>
    <State name="shown" type="Bool" value={true}/>
    <Port name="write" params="text: String" returns="String" effect="command"/>
    <Command name="edit" params="text: String" returns="Unit" body="do { let value = write(text); setState(draft: value); }"/>
    <Command name="reset" params="text: String" returns="Unit" body="do { setState(draft: text); }"/>
    <Command name="show" params="value: Bool" returns="Unit" body="do { setState(shown: value); }"/>
    <View><Column key="root"><If {shown}><Input key="editor" value={draft} onChange={edit($event.value)}/></If><Text key="current" text={draft}/></Column></View>
</Module>"#;

struct Gate {
    open: Arc<(Mutex<usize>, Condvar)>,
}
impl Gate {
    fn release_through(&self, count: usize) {
        *self.open.0.lock().unwrap() = count;
        self.open.1.notify_all();
    }
    fn release(&self) {
        self.release_through(usize::MAX);
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.release();
    }
}
fn gated_instance() -> (Instance, mpsc::Receiver<String>, Gate) {
    let module = load_module(INPUT, "input-flow.uix").unwrap();
    let (sender, receiver) = mpsc::channel();
    let open = Arc::new((Mutex::new(0), Condvar::new()));
    let gate = Gate { open: open.clone() };
    let ports = HostPorts::from([(
        "write".into(),
        HostPort {
            signature: module.ports[0].clone(),
            callback: Arc::new(move |args| {
                let text = args[0].as_str()?.to_string();
                sender.send(text.clone()).unwrap();
                if text == "hold" || text == "first" || text == "second" {
                    let required = if text == "second" { 2 } else { 1 };
                    let (lock, wake) = &*open;
                    let (guard, timeout) = wake
                        .wait_timeout_while(lock.lock().unwrap(), WAIT, |ready| *ready < required)
                        .unwrap();
                    if timeout.timed_out() && *guard < required {
                        return Err(RuntimeError::new(
                            ErrorKind::HostFailure,
                            "合成端点门闩超时",
                        ));
                    }
                }
                if text == "fail" {
                    return Err(RuntimeError::new(ErrorKind::HostFailure, "合成写入拒绝"));
                }
                Ok(Value::String(text.to_uppercase()))
            }),
        },
    )]);
    (
        Instance::new(
            module,
            ports,
            Limits {
                timeout: Duration::from_secs(8),
                ..Limits::default()
            },
        )
        .unwrap(),
        receiver,
        gate,
    )
}

#[test]
fn accepted_sequence_capacity_cancellation_and_failures_are_observable() {
    let (instance, entered, gate) = gated_instance();
    let worker = ModuleWorker::spawn(instance, 1, || {}).unwrap();
    let handle = worker.handle();
    let first = handle
        .call("edit", vec![Value::String("hold".into())])
        .unwrap();
    assert_eq!(entered.recv_timeout(WAIT).unwrap(), "hold");
    let queued = handle
        .call("reset", vec![Value::String("cancelled".into())])
        .unwrap();
    let rejected = handle.call("reset", vec![Value::String("lost".into())]);
    assert!(matches!(
        rejected,
        Err(RuntimeError {
            kind: ErrorKind::Quota,
            ..
        })
    ));
    queued.cancellation.cancel();
    let sequence = queued.sequence;
    gate.release();
    first.wait(WAIT).unwrap();
    assert_eq!(queued.wait(WAIT).unwrap_err().kind, ErrorKind::Cancelled);
    let failed = handle
        .call("edit", vec![Value::String("fail".into())])
        .unwrap();
    assert_eq!(failed.sequence, sequence + 1);
    assert_eq!(failed.wait(WAIT).unwrap_err().kind, ErrorKind::HostFailure);
    let snapshot = handle.snapshot();
    assert_eq!(snapshot.sequence, sequence + 1);
    assert_eq!(snapshot.state["draft"], Value::String("HOLD".into()));
    assert_eq!(
        find(snapshot.view.as_ref().unwrap(), "editor")
            .unwrap()
            .properties["value"],
        Value::String("HOLD".into())
    );
    assert_eq!(snapshot.error.unwrap().kind, ErrorKind::HostFailure);
    worker.close(WAIT).unwrap();
    assert!(!handle.snapshot().active);
    assert!(handle.snapshot().state.is_empty());
    assert!(handle.snapshot().view.is_none());
}

#[test]
fn revoke_and_close_reject_inflight_publication_and_release_host_captures() {
    for revoke in [true, false] {
        let (instance, entered, gate) = gated_instance();
        let resource = Arc::downgrade(&gate.open);
        let worker = ModuleWorker::spawn(instance, 1, || {}).unwrap();
        let handle = worker.handle();
        let first = handle
            .call("edit", vec![Value::String("hold".into())])
            .unwrap();
        entered.recv_timeout(WAIT).unwrap();
        let queued = handle
            .call("reset", vec![Value::String("late".into())])
            .unwrap();
        if revoke {
            handle.revoke();
        } else {
            handle.close();
        }
        let kind = if revoke {
            ErrorKind::CapabilityDenied
        } else {
            ErrorKind::Closed
        };
        assert!(!handle.snapshot().active);
        assert!(handle.snapshot().state.is_empty());
        assert!(handle.snapshot().view.is_none());
        assert!(
            matches!(handle.call("reset", vec![Value::String("late".into())]), Err(error) if error.kind == kind)
        );
        gate.release();
        assert_eq!(first.wait(WAIT).unwrap_err().kind, kind);
        assert_eq!(queued.wait(WAIT).unwrap_err().kind, kind);
        worker.close(WAIT).unwrap();
        drop(gate);
        assert!(resource.upgrade().is_none());
        assert!(handle.snapshot().state.is_empty());
    }
}

#[test]
fn wait_timeout_does_not_claim_business_cancellation() {
    let (instance, entered, gate) = gated_instance();
    let worker = ModuleWorker::spawn(instance, 1, || {}).unwrap();
    let handle = worker.handle();
    let request = handle
        .call("edit", vec![Value::String("hold".into())])
        .unwrap();
    entered.recv_timeout(WAIT).unwrap();
    let cancellation = request.cancellation.clone();
    assert_eq!(
        request.wait(Duration::from_millis(5)).unwrap_err().kind,
        ErrorKind::Timeout
    );
    assert!(!cancellation.is_cancelled());
    gate.release();
    // 串行栅栏确认前项已完成，不通过短时睡眠猜测业务终态。
    handle
        .call("show", vec![Value::Bool(false)])
        .unwrap()
        .wait(WAIT)
        .unwrap();
    assert_eq!(
        handle.snapshot().state["draft"],
        Value::String("HOLD".into())
    );
    worker.close(WAIT).unwrap();
}

#[cfg(feature = "test-harness")]
#[test]
fn projected_inputs_rollback_rejected_drafts_and_apply_acknowledged_authority() {
    use uix::ui::test_harness::TestApp;
    let (instance, entered, gate) = gated_instance();
    let (worker, view) = ModuleView::spawn(instance, 1, || {}).unwrap();
    let handle = view.handle();
    let mut app = TestApp::new((480.0, 240.0), move || view.project().unwrap());
    let hold = handle
        .call("edit", vec![Value::String("hold".into())])
        .unwrap();
    entered.recv_timeout(WAIT).unwrap();
    let queued = handle
        .call("reset", vec![Value::String("authority".into())])
        .unwrap();
    app.set_value("InputFlow/editor", "rejected").unwrap();
    app.settle().unwrap();
    assert_eq!(app.text("InputFlow/editor").unwrap(), "initial");
    assert_eq!(handle.snapshot().error.unwrap().kind, ErrorKind::Quota);
    gate.release();
    hold.wait(WAIT).unwrap();
    queued.wait(WAIT).unwrap();
    app.settle().unwrap();
    assert_eq!(app.text("InputFlow/editor").unwrap(), "authority");
    app.set_value("InputFlow/editor", "fail").unwrap();
    assert_eq!(entered.recv_timeout(WAIT).unwrap(), "fail");
    handle
        .call("show", vec![Value::Bool(false)])
        .unwrap()
        .wait(WAIT)
        .unwrap();
    app.settle().unwrap();
    assert!(app.snapshot().find("InputFlow/editor").is_err());
    handle
        .call("reset", vec![Value::String("restored".into())])
        .unwrap()
        .wait(WAIT)
        .unwrap();
    handle
        .call("show", vec![Value::Bool(true)])
        .unwrap()
        .wait(WAIT)
        .unwrap();
    app.settle().unwrap();
    assert_eq!(app.text("InputFlow/editor").unwrap(), "restored");
    worker.close(WAIT).unwrap();
}

#[cfg(feature = "test-harness")]
#[test]
fn newer_input_draft_survives_older_completion_until_its_own_acknowledgement() {
    use uix::ui::test_harness::TestApp;
    let (instance, entered, gate) = gated_instance();
    let (worker, view) = ModuleView::spawn(instance, 1, || {}).unwrap();
    let handle = view.handle();
    let mut app = TestApp::new((480.0, 240.0), move || view.project().unwrap());
    app.set_value("InputFlow/editor", "first").unwrap();
    assert_eq!(entered.recv_timeout(WAIT).unwrap(), "first");
    app.set_value("InputFlow/editor", "second").unwrap();
    assert_eq!(app.text("InputFlow/editor").unwrap(), "second");
    gate.release_through(1);
    // 第二个端口已开始而未完成，因此序号 1 的完成事实确定可见。
    assert_eq!(entered.recv_timeout(WAIT).unwrap(), "second");
    assert_eq!(handle.snapshot().sequence, 1);
    assert_eq!(
        handle.snapshot().state["draft"],
        Value::String("FIRST".into())
    );
    app.settle().unwrap();
    assert_eq!(app.text("InputFlow/editor").unwrap(), "second");
    gate.release();
    handle
        .call("show", vec![Value::Bool(true)])
        .unwrap()
        .wait(WAIT)
        .unwrap();
    app.settle().unwrap();
    assert_eq!(app.text("InputFlow/editor").unwrap(), "SECOND");
    worker.close(WAIT).unwrap();
}

#[test]
fn worker_replacement_failure_retains_view_and_success_rejects_old_generation() {
    let instance = instances().remove(1);
    let worker = ModuleWorker::spawn(instance, 2, || {}).unwrap();
    let handle = worker.handle();
    let before = handle.snapshot();
    let invalid = load_module(
        &LIST.replace("gap={spacing}", "gap={-1}"),
        "bad-candidate.uix",
    )
    .unwrap();
    assert_eq!(
        handle
            .replace(invalid, 1)
            .unwrap()
            .wait(WAIT)
            .unwrap_err()
            .kind,
        ErrorKind::Type
    );
    let failed = handle.snapshot();
    assert_eq!(failed.view, before.view);
    assert_eq!(failed.state, before.state);
    assert_eq!(failed.generation, before.generation);
    assert_eq!(failed.revision, before.revision);
    assert_eq!(failed.sequence, 1);
    handle
        .replace(
            load_module(
                &LIST.replace("version=\"1\"", "version=\"2\""),
                "new-candidate.uix",
            )
            .unwrap(),
            1,
        )
        .unwrap()
        .wait(WAIT)
        .unwrap();
    assert_eq!(
        handle
            .event("choose/s616c706861", 1, vec![])
            .unwrap()
            .wait(WAIT)
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    handle
        .event("choose/s616c706861", 2, vec![])
        .unwrap()
        .wait(WAIT)
        .unwrap();
    assert_eq!(
        handle.snapshot().state["selected"],
        Value::String("alpha:甲".into())
    );
    worker.close(WAIT).unwrap();
}
