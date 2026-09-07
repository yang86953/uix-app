//! 消费应用模块规范：同一源码经真实 AOT 与动态入口执行。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use uix::app::modules::*;

const TEXT: &str = include_str!("../examples/modules/text_bench.uix");

fn ports(module: &Module, log: Arc<Mutex<Vec<String>>>) -> HostPorts {
    module.ports.iter().map(|signature| {
        let name = signature.name.clone();
        let log = log.clone();
        (name.clone(), HostPort { signature: signature.clone(), callback: Arc::new(move |args| {
            log.lock().expect("日志锁").push(format!("{name}:{args:?}"));
            match name.as_str() {
                "documents_read" => Ok(Value::String("原始 文档".into())),
                "documents_commit" => Ok(Value::Bool(true)),
                _ => Err(RuntimeError::new(ErrorKind::HostFailure, "未知端口")),
            }
        }) })
    }).collect()
}

#[test]
fn aot_and_dynamic_text_processing_have_identical_results_state_and_effect_order() {
    let native = uix::uix_module!("examples/modules/text_bench.uix");
    let dynamic = load_module(TEXT, "examples/modules/text_bench.uix").expect("动态装载");
    let mut observed = Vec::new();
    for module in [native, dynamic] {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut instance = Instance::new(module.clone(), ports(&module, log.clone()), Limits::default()).expect("实例");
        let id = Value::String("doc-1".into());
        assert_eq!(instance.call("read", &[id.clone()]).expect("读取"), Value::String("原始 文档".into()));
        let mut results = Vec::new();
        for input in ["", "   ", "hello", " hello  世界 \nnext "] {
            results.push(instance.call("process", &[Value::String(input.into())]).expect("处理"));
        }
        assert_eq!(results[0], Value::Record(BTreeMap::from([
            ("characters".into(), Value::Int(0)), ("words".into(), Value::Int(0)), ("normalized".into(), Value::String(String::new()))])));
        assert_eq!(instance.call("current", &[]).expect("状态"), Value::String("hello 世界 next".into()));
        assert_eq!(instance.call("commit", &[id]).expect("提交"), Value::Bool(true));
        observed.push((results, instance.state(), instance.revision(), log.lock().expect("日志锁").clone()));
    }
    assert_eq!(observed[0], observed[1]);
    assert_eq!(observed[0].3.len(), 2);
}

const FAILURES: &str = r#"<Module name="FailureCases" version="1" schema="1">
  <State name="count" type="Int" value={0} />
  <Command name="divide" params="divisor: Int" returns="Int" body="do { setState(count: count + 1); return 100 / divisor; }" />
  <Function name="overflow" params="value: Int" returns="Int" export="true" body="value + 1" />
  <Function name="short_circuit" returns="Bool" export="true" body="false &amp;&amp; (1 / 0 == 0)" />
  <Function name="mapped" params="values: Array&lt;Int&gt;" returns="Array&lt;Int&gt;" export="true" body="values.filter(|value| value &gt; 1).map(|value| value * 2)" />
</Module>"#;

#[test]
fn compiler_rejects_types_unbound_names_and_effects_before_execution() {
    for source in [
        r#"<Module name="Bad" version="1" schema="1"><Function name="bad" returns="Int" body="unknownRust()" /></Module>"#,
        r#"<Module name="Bad" version="1" schema="1"><State name="count" type="Int" value={0}/><Function name="bad" returns="Int" body="count"/></Module>"#,
        r#"<Module name="Bad" version="1" schema="1"><Function name="bad" returns="Int" body="'text'"/></Module>"#,
        r#"<Module name="Bad" version="1" schema="1"><Port name="write" returns="Int" effect="command"/><Query name="bad" returns="Int" body="write()"/></Module>"#,
    ] {
        let error = load_module(source, "invalid.uix").expect_err("必须拒绝");
        assert_eq!(error.code, "UIX2100");
        assert_eq!(error.source_name, "invalid.uix");
        assert!(error.line > 0 && !error.message.is_empty());
    }
}

#[test]
fn failures_rollback_state_and_keep_checked_arithmetic_and_short_circuit() {
    // 表达式字符串沿 UIX 原有规则直接使用运算符，不进行 XML 实体转义。
    let source = FAILURES.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">");
    let module = load_module(&source, "failures.uix").expect("失败场景模块");
    let mut instance = Instance::new(module, BTreeMap::new(), Limits::default()).expect("实例");
    let error = instance.call("divide", &[Value::Int(0)]).expect_err("除零");
    assert_eq!(error.kind, ErrorKind::Arithmetic);
    assert_eq!(error.location.source, "failures.uix");
    assert_eq!(instance.state()["count"], Value::Int(0));
    assert_eq!(instance.revision(), 0);
    assert_eq!(instance.call("divide", &[Value::Int(4)]).expect("成功"), Value::Int(25));
    assert_eq!(instance.state()["count"], Value::Int(1));
    assert_eq!(instance.call("overflow", &[Value::Int(i64::MAX)]).expect_err("溢出").kind, ErrorKind::Arithmetic);
    assert_eq!(instance.call("short_circuit", &[]).expect("短路"), Value::Bool(false));
    assert_eq!(instance.call("mapped", &[Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])]).expect("集合算法"),
        Value::Array(vec![Value::Int(4), Value::Int(6)]));
}

#[test]
fn replacement_revocation_and_close_have_observable_terminal_results() {
    let module = load_module(TEXT, "bench.uix").expect("装载");
    let mut instance = Instance::new(module.clone(), ports(&module, Arc::new(Mutex::new(Vec::new()))), Limits::default()).expect("实例");
    instance.call("process", &[Value::String("old draft".into())]).expect("初始业务");
    let generation = instance.generation();
    let next_source = TEXT.replace("version=\"1.0.0\"", "version=\"2.0.0\"").replace("text.words().join(' ')", "text.words().join('-')");
    let next = load_module(&next_source, "bench-v2.uix").expect("新代");
    assert_eq!(instance.replace(next.clone(), generation + 1).expect_err("陈旧代").kind, ErrorKind::Conflict);
    assert_eq!(instance.generation(), generation);
    instance.replace(next, generation).expect("替换");
    assert_eq!(instance.state()["draft"], Value::String("old draft".into()));
    instance.call("process", &[Value::String("new text".into())]).expect("升级算法");
    assert_eq!(instance.state()["draft"], Value::String("new-text".into()));
    let incompatible = load_module(&next_source.replace("schema=\"1\"", "schema=\"2\""), "incompatible.uix").expect("候选");
    assert_eq!(instance.replace(incompatible, instance.generation()).expect_err("schema 不符").kind, ErrorKind::Conflict);
    instance.revoke();
    assert_eq!(instance.call("current", &[]).expect_err("撤权").kind, ErrorKind::CapabilityDenied);
    instance.close();
    assert!(instance.state().is_empty());
    assert_eq!(instance.call("current", &[]).expect_err("关闭").kind, ErrorKind::Closed);
}

#[test]
fn cancellation_quotas_arguments_and_instance_isolation_are_enforced() {
    let module = load_module(TEXT, "bench.uix").expect("装载");
    assert!(matches!(Instance::new(module.clone(), BTreeMap::new(), Limits::default()), Err(RuntimeError { kind: ErrorKind::CapabilityDenied, .. })));
    let ports = ports(&module, Arc::new(Mutex::new(Vec::new())));
    let mut first = Instance::new(module.clone(), ports.clone(), Limits::default()).expect("实例一");
    let second = Instance::new(module.clone(), ports.clone(), Limits::default()).expect("实例二");
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(first.call_with_cancellation("process", &[Value::String("text".into())], &cancellation).expect_err("取消").kind, ErrorKind::Cancelled);
    assert_eq!(first.call("process", &[Value::Int(3)]).expect_err("参数错误").kind, ErrorKind::Argument);
    first.call("process", &[Value::String("changed".into())]).expect("变更");
    assert_eq!(second.state()["draft"], Value::String(String::new()));
    let mut limited_module = module;
    limited_module.view = None;
    let mut limited = Instance::new(limited_module, ports, Limits { steps: 2, ..Limits::default() }).expect("有界实例");
    assert_eq!(limited.call("process", &[Value::String("text".into())]).expect_err("配额").kind, ErrorKind::Quota);
    assert_eq!(limited.revision(), 0);
}
