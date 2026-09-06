//! 软件动态扩展（P1 无界面最小闭环）公开 API 消费者测试。
//!
//! 验证面（docs/使用/能力与边界/软件动态扩展.md）：
//! - 扩展装载、命令注册、类型化调用与实例状态保留；
//! - 宿主端口授权数据访问与跨边界 owned 值；
//! - 包校验、冲突 ID、未知命令、能力拒绝、超时、取消与停止终态；
//! - R7RS-small 标准语义样例经公开装载 / 调用入口提交（章节出处随断言）。

#![cfg(feature = "extensions")]

use std::collections::BTreeMap;
use std::sync::Arc;

use uix::app::extensions::{
    ExtensionError, ExtensionHost, ExtensionHostConfig, ExtensionPackage, ExtensionValue,
};

fn package(id: &str, source: &str) -> ExtensionPackage {
    let manifest = format!(
        "(uix-extension (schema-version 1) (id \"{id}\") (version \"0.1.0\") \
         (language r7rs-small) (entry \"main.scm\"))"
    );
    package_with(manifest, &[("main.scm", source)])
}

fn package_with(manifest: String, sources: &[(&str, &str)]) -> ExtensionPackage {
    let sources: BTreeMap<String, String> = sources
        .iter()
        .map(|(path, source)| (path.to_string(), source.to_string()))
        .collect();
    ExtensionPackage::from_parts(manifest, sources).expect("包构造")
}

fn text(value: &str) -> ExtensionValue {
    ExtensionValue::Text(value.to_string())
}

/// 文本处理工作台基线：无窗口装载、执行新算法、状态跨调用保留。
#[test]
fn text_extension_minimal_loop() {
    let package = package(
        "text-tools",
        r#"
(define uses 0)
(register-command! "reverse"
  (lambda (s)
    (set! uses (+ uses 1))
    (list->string (reverse (string->list s)))))
(register-command! "uses" (lambda () uses))
"#,
    );
    let mut host = ExtensionHost::new();
    let prepared = host.prepare(&package).expect("准备");
    assert_eq!(prepared.commands(), ["reverse", "uses"]);
    let receipt = host.activate(prepared).expect("激活");
    assert_eq!(receipt.extension_id, "text-tools");
    assert_eq!(receipt.generation, 1);

    let outcome = host
        .call_command("text-tools", "reverse", &[text("Scheme")])
        .expect("调用");
    assert_eq!(outcome, text("emehcS"));
    let again = host
        .call_command("text-tools", "reverse", &[text("abc")])
        .expect("再次调用");
    assert_eq!(again, text("cba"));
    // 扩展私有状态跨调用保留（实例复用）。
    let uses = host.call_command("text-tools", "uses", &[]).expect("计数");
    assert_eq!(uses, ExtensionValue::Int(2));

    let status = host.list();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].commands, ["reverse", "uses"]);

    let teardown = host.deactivate("text-tools").expect("停止");
    assert_eq!(teardown.extension_id, "text-tools");
    match host.call_command("text-tools", "reverse", &[text("x")]) {
        Err(ExtensionError::UnknownExtension(id)) => assert_eq!(id, "text-tools"),
        other => panic!("停止后调用应拒绝：{other:?}"),
    }
}

/// 宿主端口：扩展读取授权数据；未注入能力在准备期拒绝。
#[test]
fn host_port_reads_authorized_data() {
    let manifest = "(uix-extension (schema-version 1) (id \"reader\") (version \"1.0.0\") \
                    (language r7rs-small) (entry \"main.scm\") (capabilities documents-query))"
        .to_string();
    let package = package_with(
        manifest,
        &[(
            "main.scm",
            r#"
(register-command! "intro-length"
  (lambda () (string-length (documents-query "intro"))))
"#,
        )],
    );
    let host = ExtensionHost::new().with_port(
        "documents-query",
        Arc::new(|arguments: &[ExtensionValue]| match arguments.first() {
            Some(ExtensionValue::Text(key)) if key == "intro" => Ok(text("hello uix")),
            _ => Err("未知文档".to_string()),
        }),
    );
    let prepared = host.prepare(&package).expect("准备");
    let mut host = host;
    host.activate(prepared).expect("激活");
    let outcome = host
        .call_command("reader", "intro-length", &[])
        .expect("读取宿主数据");
    assert_eq!(outcome, ExtensionValue::Int(9));

    // 端口自身失败映射为 HostFailure。
    let failing = package_with(
        "(uix-extension (schema-version 1) (id \"reader2\") (version \"1.0.0\") \
         (language r7rs-small) (entry \"main.scm\") (capabilities documents-query))"
            .to_string(),
        &[(
            "main.scm",
            r#"(register-command! "bad" (lambda () (documents-query "missing")))"#,
        )],
    );
    let prepared = host.prepare(&failing).expect("准备");
    host.activate(prepared).expect("激活");
    match host.call_command("reader2", "bad", &[]) {
        Err(ExtensionError::HostFailure(_)) => {}
        other => panic!("端口失败应映射 HostFailure：{other:?}"),
    }
}

/// 声明宿主未提供的能力在准备期拒绝（声明 ∩ 注入）。
#[test]
fn undeclared_capability_rejected() {
    let manifest = "(uix-extension (schema-version 1) (id \"wants\") (version \"0.1.0\") \
                    (language r7rs-small) (entry \"main.scm\") (capabilities no-such-port))"
        .to_string();
    let package = package_with(manifest, &[("main.scm", "")]);
    let host = ExtensionHost::new();
    match host.prepare(&package) {
        Err(ExtensionError::Incompatible(message)) => {
            assert!(message.contains("no-such-port"), "{message}")
        }
        other => panic!("应拒绝未提供能力：{other:?}"),
    }
}

/// 两个扩展环境互不污染：同名绑定与状态彼此独立。
#[test]
fn extension_environments_are_isolated() {
    let mut host = ExtensionHost::new();
    for id in ["alpha", "beta"] {
        let package = package(
            id,
            r#"
(define secret 0)
(register-command! "bump"
  (lambda ()
    (set! secret (+ secret 1))
    secret))
(register-command! "peek" (lambda () secret))
"#,
        );
        let prepared = host.prepare(&package).expect("准备");
        host.activate(prepared).expect("激活");
    }
    host.call_command("alpha", "bump", &[]).unwrap();
    host.call_command("alpha", "bump", &[]).unwrap();
    host.call_command("beta", "bump", &[]).unwrap();
    let alpha = host.call_command("alpha", "peek", &[]).unwrap();
    let beta = host.call_command("beta", "peek", &[]).unwrap();
    assert_eq!(alpha, ExtensionValue::Int(2));
    assert_eq!(beta, ExtensionValue::Int(1));
}

/// 同 ID 重复激活拒绝；未知命令有类型化失败。
#[test]
fn conflicts_and_unknowns_are_typed() {
    let package = package("dup", r#"(register-command! "only" (lambda () 1))"#);
    let mut host = ExtensionHost::new();
    let first = host.prepare(&package).expect("准备");
    host.activate(first).expect("激活");
    let second = host.prepare(&package).expect("再次准备");
    match host.activate(second) {
        Err(ExtensionError::Incompatible(message)) => {
            assert!(message.contains("已有活动实例"), "{message}")
        }
        other => panic!("重复激活应拒绝：{other:?}"),
    }
    match host.call_command("dup", "missing", &[]) {
        Err(ExtensionError::UnknownCommand { extension, command }) => {
            assert_eq!(extension, "dup");
            assert_eq!(command, "missing");
        }
        other => panic!("未知命令应类型化失败：{other:?}"),
    }
    // 重复卸载同样有确定结果。
    host.deactivate("dup").expect("停止");
    match host.deactivate("dup") {
        Err(ExtensionError::UnknownExtension(id)) => assert_eq!(id, "dup"),
        other => panic!("重复停止应拒绝：{other:?}"),
    }
}

/// 无效包：未知字段、坏 semver、入口缺失、路径逃逸。
#[test]
fn invalid_packages_are_rejected() {
    let host = ExtensionHost::new();
    for (label, manifest) in [
        (
            "未知字段",
            "(uix-extension (schema-version 1) (id \"bad\") (version \"0.1.0\") \
             (language r7rs-small) (entry \"main.scm\") (surprise 1))"
                .to_string(),
        ),
        (
            "坏 semver",
            "(uix-extension (schema-version 1) (id \"bad\") (version \"0.1\") \
             (language r7rs-small) (entry \"main.scm\"))"
                .to_string(),
        ),
        (
            "入口缺失",
            "(uix-extension (schema-version 1) (id \"bad\") (version \"0.1.0\") \
             (language r7rs-small) (entry \"other.scm\"))"
                .to_string(),
        ),
        (
            "路径逃逸",
            "(uix-extension (schema-version 1) (id \"bad\") (version \"0.1.0\") \
             (language r7rs-small) (entry \"../main.scm\"))"
                .to_string(),
        ),
        (
            "不受支持 schema",
            "(uix-extension (schema-version 2) (id \"bad\") (version \"0.1.0\") \
             (language r7rs-small) (entry \"main.scm\"))"
                .to_string(),
        ),
    ] {
        let sources: BTreeMap<String, String> =
            [("main.scm".to_string(), String::new())].into_iter().collect();
        let outcome = ExtensionPackage::from_parts(manifest, sources);
        assert!(outcome.is_err(), "{label} 应在冻结时被拒绝");
    }
}

/// 墙钟超时与取消是可观察终态，且不可被脚本捕获。
#[test]
fn timeout_and_cancellation_have_terminal_results() {
    let package = package("spinner", r#"
(register-command! "spin" (lambda () (let loop () (loop))))
(register-command! "spin-safe"
  (lambda ()
    (guard (caught #t)
      (let loop () (loop)))))
"#);
    let mut host = ExtensionHost::new().with_config(ExtensionHostConfig {
        command_wall_time_ms: 150,
        ..ExtensionHostConfig::default()
    });
    let prepared = host.prepare(&package).expect("准备");
    host.activate(prepared).expect("激活");
    match host.call_command("spinner", "spin", &[]) {
        Err(ExtensionError::Timeout { milliseconds }) => assert_eq!(milliseconds, 150),
        other => panic!("死循环应超时：{other:?}"),
    }
    // guard 不能捕获宿主边界。
    match host.call_command("spinner", "spin-safe", &[]) {
        Err(ExtensionError::Timeout { .. }) => {}
        other => panic!("guard 不应捕获墙钟：{other:?}"),
    }
}

#[test]
fn cancellation_from_another_thread() {
    let package = package("cancel-me", r#"
(register-command! "spin" (lambda () (let loop () (loop))))
"#);
    let mut host = ExtensionHost::new().with_config(ExtensionHostConfig {
        command_wall_time_ms: 10_000,
        ..ExtensionHostConfig::default()
    });
    let prepared = host.prepare(&package).expect("准备");
    host.activate(prepared).expect("激活");
    let cancel = host.cancel_handle("cancel-me").expect("取消令柄");
    let guard = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(80));
        cancel.cancel();
    });
    match host.call_command("cancel-me", "spin", &[]) {
        Err(ExtensionError::Cancelled) => {}
        other => panic!("应观察到取消：{other:?}"),
    }
    guard.join().expect("取消线程");
}

/// 深度递归以配额拒绝（Quota 终态）。
#[test]
fn depth_quota_is_enforced() {
    let package = package("deep", r#"
(define (fall n) (if (= n 0) 0 (+ 1 (fall (- n 1)))))
(register-command! "fall" (lambda () (fall 100000)))
"#);
    let mut host = ExtensionHost::new();
    let prepared = host.prepare(&package).expect("准备");
    host.activate(prepared).expect("激活");
    match host.call_command("deep", "fall", &[]) {
        Err(ExtensionError::Quota(_)) => {}
        other => panic!("深递归应配额拒绝：{other:?}"),
    }
}

/// 循环分配在标记-清扫回收后不无界增长，实例保持可用。
#[test]
fn garbage_cycles_are_collected_and_instance_survives() {
    let package = package("cycler", r#"
(define discarded 0)
(register-command! "churn"
  (lambda ()
    ;; 构造循环 pair 后丢弃；配额不允许它无界累积。
    (let ((ring (cons 1 (cons 2 '()))))
      (set-cdr! (cdr ring) ring)
      (set! discarded (+ discarded 1))
      discarded)))
(register-command! "ping" (lambda () 'alive))
"#);
    let mut host = ExtensionHost::new().with_config(ExtensionHostConfig::default());
    let prepared = host.prepare(&package).expect("准备");
    host.activate(prepared).expect("激活");
    let mut last = 0;
    for _ in 0..64 {
        last = match host.call_command("cycler", "churn", &[]).expect("churn") {
            ExtensionValue::Int(count) => count,
            other => panic!("计数应返回整数：{other:?}"),
        };
    }
    assert_eq!(last, 64);
    let alive = host.call_command("cycler", "ping", &[]).expect("回收后实例可用");
    assert_eq!(alive, ExtensionValue::Symbol("alive".to_string()));
    host.deactivate("cycler").expect("停止后清扫不 panic");
}

/// 脚本 error / 未捕获 raise 是脚本域失败。
#[test]
fn script_failures_are_typed() {
    let package = package("failing", r#"
(register-command! "boom" (lambda () (error "业务失败" 42)))
(register-command! "raise-it" (lambda () (raise 'custom)))
(register-command! "guarded"
  (lambda ()
    (guard (caught ((symbol? caught) #t))
      (raise 'inner)
      'unreachable)))
"#);
    let mut host = ExtensionHost::new();
    let prepared = host.prepare(&package).expect("准备");
    host.activate(prepared).expect("激活");
    match host.call_command("failing", "boom", &[]) {
        Err(ExtensionError::ScriptFailure(message)) => {
            assert!(message.contains("业务失败"), "{message}")
        }
        other => panic!("error 应脚本域失败：{other:?}"),
    }
    match host.call_command("failing", "raise-it", &[]) {
        Err(ExtensionError::ScriptFailure(message)) => {
            assert!(message.contains("custom"), "{message}")
        }
        other => panic!("raise 应脚本域失败：{other:?}"),
    }
    let caught = host
        .call_command("failing", "guarded", &[])
        .expect("guard 捕获");
    assert_eq!(caught, ExtensionValue::Bool(true));
}

/// R7RS-small 标准语义样例（章节出处见各断言）。
#[test]
fn r7rs_semantics_samples() {
    let package = package(
        "semantics",
        r#"
;; 词法闭包与 counter（R7RS §3.4）。
(define (make-counter)
  (let ((n 0))
    (lambda () (set! n (+ n 1)) n)))
(define counter-a (make-counter))
(define counter-b (make-counter))

;; proper tail calls（§3.5）：十万次尾递归不触发深度上限（帧深 8192）。
(define (loop-n n) (if (= n 0) 'done (loop-n (- n 1))))

;; syntax-rules 卫生宏（§4.3）：宏内 swap! 不捕获使用处 temp。
(define-syntax swap!
  (syntax-rules ()
    ((_ a b) (let ((tmp a)) (set! a b) (set! b tmp)))))

;; 数值塔（§6.2）：exact 有理数与 bignum。
(define big (* 99999999999999999999 99999999999999999999))
(define ratio (+ 1/3 1/6))

;; call/cc 与 dynamic-wind（§6.10）。
(define wind-trace '())
(define (rewind-demo)
  (call/cc
    (lambda (escape)
      (dynamic-wind
        (lambda () (set! wind-trace (cons 'before wind-trace)))
        (lambda () (escape 'out))
        (lambda () (set! wind-trace (cons 'after wind-trace)))))))

;; record（§5.5）。
(define-record-type point (make-point x y) point? (x point-x) (y point-y set-point-y!))

;; 库系统（§5.6）。
(define-library (demo math)
  (export triple)
  (begin (define (triple n) (* 3 n))))
(import (demo math))

;; quasiquote（§4.2.8）。
(define qq `(1 ,(+ 1 1) ,@(list 3 4)))

;; 多值（§4.2 / §6.10）。
(define two-values (call-with-values (lambda () (values 3 4)) *))

;; parameterize（§4.2）。
(define p (make-parameter 10))
(define param-result (parameterize ((p 42)) (p)))

;; lazy（(scheme lazy)）。
(define delayed (delay (begin 1 2 3)))

;; guard 与 raise-continuable（§6.11）。
(define continuable
  (with-exception-handler
    (lambda (exc) (+ exc 10))
    (lambda () (raise-continuable 5))))

;; bytevector（§6.9）。
(define bytes (bytevector-append #u8(1 2) #u8(3 4)))

;; 字符串端口（§6.13）。
(define port-text
  (let ((out (open-output-string)))
    (display "line1" out)
    (newline out)
    (get-output-string out)))

;; 字符串端口读取（(scheme read) 内存端口）。
(define parsed
  (let ((in (open-input-string "(a b) 7")))
    (let ((datum (read in)))
      (length datum))))

(define-syntax my-or
  (syntax-rules ()
    ((_) #f)
    ((_ e) e)
    ((_ e1 e2 ...) (let ((t e1)) (if t t (my-or e2 ...))))))

(register-command! "checks"
  (lambda ()
    (list
      ;; 闭包隔离。
      (counter-a) (counter-a) (counter-b)
      ;; 尾调用。
      (loop-n 100000)
      ;; 卫生宏。
      (let ((a 1) (b 2) (tmp 99)) (swap! a b) (list a b tmp))
      ;; 数值塔。
      (> big 0) (= ratio 1/2)
      ;; call/cc + dynamic-wind。
      (rewind-demo) wind-trace
      ;; record。
      (let ((pt (make-point 3 4)))
        (set-point-y! pt 5)
        (list (point? pt) (point-x pt) (point-y pt)))
      ;; 库。
      (triple 14)
      ;; quasiquote。
      qq
      ;; 多值。
      two-values
      ;; 参数。
      param-result (p)
      ;; lazy。
      (force delayed) (force delayed)
      ;; 异常。
      continuable
      ;; bytevector。
      (utf8->string bytes)
      ;; 端口。
      port-text parsed
      ;; 递归宏。
      (my-or #f 7))))
"#,
    );
    let mut host = ExtensionHost::new().with_config(ExtensionHostConfig {
        command_wall_time_ms: 10_000,
        ..ExtensionHostConfig::default()
    });
    let prepared = host.prepare(&package).expect("准备");
    host.activate(prepared).expect("激活");
    let outcome = host
        .call_command("semantics", "checks", &[])
        .expect("标准语义样例");
    let ExtensionValue::List(items) = outcome else {
        panic!("checks 应返回列表");
    };
    let expected: Vec<ExtensionValue> = vec![
        ExtensionValue::Int(1),                                // counter-a 第一次
        ExtensionValue::Int(2),                                // counter-a 第二次
        ExtensionValue::Int(1),                                // counter-b 独立
        ExtensionValue::Symbol("done".into()),                 // 尾调用
        ExtensionValue::List(vec![
            ExtensionValue::Int(2),
            ExtensionValue::Int(1),
            ExtensionValue::Int(99),
        ]),                                                     // swap! 卫生
        ExtensionValue::Bool(true),                             // bignum 正数
        ExtensionValue::Bool(true),                             // 1/3+1/6 = 1/2
        ExtensionValue::Symbol("out".into()),                   // call/cc 逃逸
        ExtensionValue::List(vec![
            ExtensionValue::Symbol("after".into()),
            ExtensionValue::Symbol("before".into()),
        ]),                                                     // wind 追踪（倒序 cons）
        ExtensionValue::List(vec![
            ExtensionValue::Bool(true),
            ExtensionValue::Int(3),
            ExtensionValue::Int(5),
        ]),                                                     // record
        ExtensionValue::Int(42),                                // 库 triple
        ExtensionValue::List(vec![
            ExtensionValue::Int(1),
            ExtensionValue::Int(2),
            ExtensionValue::Int(3),
            ExtensionValue::Int(4),
        ]),                                                     // quasiquote
        ExtensionValue::Int(12),                                // (values 3 4) *
        ExtensionValue::Int(42),                                // parameterize 内
        ExtensionValue::Int(10),                                // parameterize 外恢复
        ExtensionValue::Int(3),                                 // force delay 取尾值
        ExtensionValue::Int(3),                                 // force 幂等
        ExtensionValue::Int(15),                                // raise-continuable 5+10
        ExtensionValue::Text("\u{1}\u{2}\u{3}\u{4}".into()),    // utf8->string
        ExtensionValue::Text("line1\n".into()),                 // 输出端口
        ExtensionValue::Int(2),                                 // read 解析 (a b)
        ExtensionValue::Int(7),                                 // my-or
    ];
    assert_eq!(items.len(), expected.len(), "样例数量");
    for (index, (actual, want)) in items.iter().zip(expected.iter()).enumerate() {
        assert_eq!(actual, want, "样例 #{index} 不符");
    }
}

/// 跨边界值限制：NaN、非严格列表与超大精确整数被拒绝。
#[test]
fn boundary_value_limits() {
    let package = package(
        "values",
        r#"
(register-command! "identity" (lambda (x) x))
(register-command! "dotted" (lambda () (cons 1 2)))
(register-command! "big" (lambda () (expt 10 40)))
"#,
    );
    let mut host = ExtensionHost::new();
    let prepared = host.prepare(&package).expect("准备");
    host.activate(prepared).expect("激活");
    match host.call_command(
        "values",
        "identity",
        &[ExtensionValue::Float(f64::NAN)],
    ) {
        Err(ExtensionError::Argument(_)) => {}
        other => panic!("NaN 应拒绝：{other:?}"),
    }
    match host.call_command("values", "dotted", &[]) {
        Err(ExtensionError::Argument(_)) => {}
        other => panic!("非严格列表应拒绝：{other:?}"),
    }
    match host.call_command("values", "big", &[]) {
        Err(ExtensionError::Argument(message)) => {
            assert!(message.contains("i64"), "{message}")
        }
        other => panic!("超大精确整数应拒绝：{other:?}"),
    }
    // 嵌套列表往返。
    let nested = host
        .call_command(
            "values",
            "identity",
            &[ExtensionValue::List(vec![
                ExtensionValue::Int(1),
                ExtensionValue::List(vec![ExtensionValue::Bool(true)]),
            ])],
        )
        .expect("嵌套列表");
    assert_eq!(
        nested,
        ExtensionValue::List(vec![
            ExtensionValue::Int(1),
            ExtensionValue::List(vec![ExtensionValue::Bool(true)]),
        ])
    );
}

/// shutdown 逐一释放全部实例。
#[test]
fn shutdown_releases_all_instances() {
    let mut host = ExtensionHost::new();
    for id in ["one", "two"] {
        let package = package(id, r#"(register-command! "noop" (lambda () 0))"#);
        let prepared = host.prepare(&package).expect("准备");
        host.activate(prepared).expect("激活");
    }
    host.shutdown().expect("全部停止");
    assert!(host.list().is_empty());
}
