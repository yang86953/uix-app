//! 仅消费 AOT 模块与宿主端口，不要求应用链接运行期源码前端。
#![cfg(feature = "uix-modules")]

use std::sync::Arc;
use std::time::Duration;
use uix_app::app::modules::*;

#[test]
fn aot_module_executes_typed_async_business_through_the_public_worker() {
    let module = uix_app::uix_module!("examples/modules/async_bench.uix");
    let ports = module
        .ports
        .iter()
        .map(|signature| {
            let name = signature.name.clone();
            (
                name.clone(),
                AsyncHostPort {
                    signature: signature.clone(),
                    start: Arc::new(move |call| {
                        let value = if name == "read" {
                            Value::String(call.arguments[0].as_str()?.to_uppercase())
                        } else {
                            Value::Result(Ok(Box::new(Value::Bool(true))))
                        };
                        call.completion.complete(Ok(value))
                    }),
                },
            )
        })
        .collect();
    let instance =
        Instance::new_with_async_ports(module, HostPorts::new(), ports, Limits::default()).unwrap();
    let owner = ModuleWorker::spawn(instance, 2, || {}).unwrap();
    let handle = owner.handle();
    assert_eq!(
        handle
            .call("process", vec![Value::String("aot source".into())])
            .unwrap()
            .wait(Duration::from_secs(5))
            .unwrap(),
        Value::String("AOT SOURCE".into())
    );
    assert_eq!(
        handle.snapshot().state["phase"],
        Value::String("done".into())
    );
    owner.close(Duration::from_secs(5)).unwrap();
}

#[test]
fn aot_only_build_runs_multi_file_composition_without_a_runtime_source_frontend() {
    let module = uix_app::uix_module!("examples/modules/composed/main.uix");
    assert_eq!(module.dependencies.len(), 4);
    let mut ports = HostPorts::new();
    let mut asynchronous = AsyncHostPorts::new();
    for signature in &module.ports {
        if signature.asynchronous {
            asynchronous.insert(
                signature.name.clone(),
                AsyncHostPort {
                    signature: signature.clone(),
                    start: Arc::new(|call| {
                        call.completion
                            .complete(Ok(Value::String("compiled".into())))
                    }),
                },
            );
        } else {
            ports.insert(
                signature.name.clone(),
                HostPort {
                    signature: signature.clone(),
                    callback: Arc::new(|_| Ok(Value::Unit)),
                },
            );
        }
    }
    let mut instance =
        Instance::new_with_async_ports(module, ports, asynchronous, Limits::default()).unwrap();
    instance
        .call("run", &[Value::String(" AOT composition ".into())])
        .unwrap();
    assert_eq!(instance.state()["left.count"], Value::Int(1));
    assert_eq!(instance.state()["right.count"], Value::Int(0));
    let owner = ModuleWorker::spawn(instance, 2, || {}).unwrap();
    let handle = owner.handle();
    handle
        .event("fetch", handle.snapshot().generation, vec![])
        .unwrap()
        .wait(Duration::from_secs(3))
        .unwrap();
    assert_eq!(
        handle.snapshot().state["left.saved"],
        Value::String("compiled".into())
    );
    owner.close(Duration::from_secs(3)).unwrap();
}
