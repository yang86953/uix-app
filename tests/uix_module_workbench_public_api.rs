//! 使用工作台公开源码及框架门面验证业务合同；不调用示例私有管理器或 GPU。
#![cfg(feature = "uix-dynamic")]
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use uix::app::modules::*;

const WAIT: Duration = Duration::from_secs(3);
fn text(value: &str) -> Value {
    Value::String(value.into())
}
fn document(content: &str, version: i64) -> Value {
    Value::Record(BTreeMap::from([
        ("content".into(), text(content)),
        ("version".into(), Value::Int(version)),
    ]))
}

#[test]
fn external_workbench_and_aot_share_async_versioned_commit_replacement_and_revocation_contracts() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/modules/workbench");
    for (v1, v2) in [
        (
            load_module_file(&path.join("v1/main.uix")).unwrap(),
            load_module_file(&path.join("v2/main.uix")).unwrap(),
        ),
        (
            uix::uix_module!("examples/modules/workbench/v1/main.uix"),
            uix::uix_module!("examples/modules/workbench/v2/main.uix"),
        ),
    ] {
        let store = Arc::new(Mutex::new((" initial ".to_owned(), 1i64)));
        let grant = Arc::new(Mutex::new(true));
        let (read_sender, read_receiver) = mpsc::sync_channel::<AsyncCall>(2);
        let mut sync = HostPorts::new();
        let mut asynchronous = AsyncHostPorts::new();
        for signature in &v1.ports {
            if signature.asynchronous {
                let sender = read_sender.clone();
                asynchronous.insert(
                    signature.name.clone(),
                    AsyncHostPort {
                        signature: signature.clone(),
                        start: Arc::new(move |call| {
                            sender
                                .try_send(call)
                                .map_err(|_| RuntimeError::new(ErrorKind::Quota, "合成端口忙"))
                        }),
                    },
                );
            } else {
                let store = store.clone();
                let grant = grant.clone();
                sync.insert(
                    signature.name.clone(),
                    HostPort {
                        signature: signature.clone(),
                        callback: Arc::new(move |args| {
                            let allowed = *grant.lock().unwrap();
                            let mut doc = store.lock().unwrap();
                            let accepted = allowed && args[0].as_int()? == doc.1;
                            if accepted {
                                doc.0 = args[1].as_str()?.into();
                                doc.1 += 1;
                            }
                            Ok(Value::Record(BTreeMap::from([
                                ("accepted".into(), Value::Bool(accepted)),
                                ("version".into(), Value::Int(doc.1)),
                                (
                                    "status".into(),
                                    text(if accepted {
                                        "saved"
                                    } else if allowed {
                                        "conflict"
                                    } else {
                                        "denied"
                                    }),
                                ),
                            ])))
                        }),
                    },
                );
            }
        }
        let new_instance = |module| {
            Instance::new_with_async_ports(
                module,
                sync.clone(),
                asynchronous.clone(),
                Limits::default(),
            )
            .unwrap()
        };
        let owner = ModuleWorker::spawn(new_instance(v1.clone()), 4, || {}).unwrap();
        let handle = owner.handle();
        let read = handle.call("read", vec![]).unwrap();
        let pending = read_receiver.recv_timeout(WAIT).unwrap();
        assert!(handle.snapshot().pending_sequences.contains(&read.sequence));
        assert_eq!(handle.snapshot().state["status"], text("异步读取中"));
        assert_eq!(
            handle
                .call("transform", vec![text(" a  b ")])
                .unwrap()
                .wait(WAIT)
                .unwrap(),
            text("a b")
        );
        pending
            .completion
            .complete(Ok(document(" initial ", 1)))
            .unwrap();
        read.wait(WAIT).unwrap();
        handle
            .call("edit", vec![text(" alpha  beta ")])
            .unwrap()
            .wait(WAIT)
            .unwrap();
        assert_eq!(
            handle.call("process", vec![]).unwrap().wait(WAIT).unwrap(),
            text("alpha beta")
        );
        assert_eq!(
            handle.call("commit", vec![]).unwrap().wait(WAIT).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(*store.lock().unwrap(), ("alpha beta".into(), 2));
        let before = handle.snapshot();
        handle
            .replace(v2, before.generation)
            .unwrap()
            .wait(WAIT)
            .unwrap();
        assert_eq!(handle.snapshot().state, before.state);
        assert_eq!(handle.snapshot().generation, before.generation + 1);
        assert_eq!(
            handle.call("process", vec![]).unwrap().wait(WAIT).unwrap(),
            text("ALPHA | BETA")
        );
        let unchanged = store.lock().unwrap().clone();
        assert_eq!(
            handle
                .event("commit", before.generation, vec![])
                .unwrap()
                .wait(WAIT)
                .unwrap_err()
                .kind,
            ErrorKind::Conflict
        );
        assert_eq!(*store.lock().unwrap(), unchanged);
        *store.lock().unwrap() = ("independent newer content".into(), 3);
        assert_eq!(
            handle.call("commit", vec![]).unwrap().wait(WAIT).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(handle.snapshot().state["base"], Value::Int(2));
        assert_eq!(handle.snapshot().state["status"], text("conflict"));
        assert_eq!(
            *store.lock().unwrap(),
            ("independent newer content".into(), 3)
        );
        *grant.lock().unwrap() = false;
        handle.call("commit", vec![]).unwrap().wait(WAIT).unwrap();
        assert_eq!(handle.snapshot().state["status"], text("denied"));
        handle.revoke();
        assert_eq!(
            handle.call("process", vec![]).err().unwrap().kind,
            ErrorKind::CapabilityDenied
        );
        owner.close(WAIT).unwrap();
        assert!(!handle.snapshot().active);
        assert_eq!(
            handle.call("read", vec![]).err().unwrap().kind,
            ErrorKind::Closed
        );
        // 重新装载创建新实例，不复活旧句柄；显式授权仍由宿主决定。
        *grant.lock().unwrap() = true;
        let reloaded = ModuleWorker::spawn(new_instance(v1), 4, || {}).unwrap();
        assert_eq!(reloaded.handle().snapshot().state["draft"], text(""));
        reloaded.close(WAIT).unwrap();
    }
}
