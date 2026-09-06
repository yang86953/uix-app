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

// ---------- P2：动态 UI、事件与受控业务能力 ----------

use uix::app::extensions::{AsyncCompletion, UiUpdate};

fn ui_package() -> ExtensionPackage {
    let manifest = "(uix-extension (schema-version 1) (id \"panel-ext\") (version \"0.2.0\") \
                    (language r7rs-small) (entry \"main.scm\") \
                    (capabilities documents-query mount-panel))"
        .to_string();
    let sources: BTreeMap<String, String> = [(
        "main.scm".to_string(),
        r#"
(define clicks 0)
(define (panel-declaration)
  (list 'column (list 'key "root") (list 'pad 8.0) (list 'gap 6.0)
        (list 'text (list 'key "title") (list 'size 16.0)
              (string-append "点击次数: " (number->string clicks)))
        (list 'button (list 'key "btn") (list 'on-click 'on-increment)
              "增加")
        (list 'input (list 'key "field") (list 'placeholder "输入文本")
              (list 'on-change 'on-field-change) "")
        (list 'list (list 'key "items") (list "规则甲" "规则乙"))))
(register-command! "refresh" (lambda () (submit-ui! 'panel (panel-declaration)) 0))
(register-command! "clicks" (lambda () clicks))
;; 面板事件：更新扩展状态并提交新声明。
;; on-increment 由投影闭包按 (on-click 'on-increment) 绑定。
(define (on-increment)
  (set! clicks (+ clicks 1))
  (submit-ui! 'panel (panel-declaration)))
(register-handler! "on-increment" on-increment)
(define (on-field-change text)
  (submit-ui! 'panel
    (list 'column (list 'key "root")
          (list 'text (list 'key "title") (string-append "输入: " text))
          (list 'button (list 'key "btn") (list 'on-click 'on-increment) "增加"))))
(register-handler! "on-field-change" on-field-change)
;; 受控业务改写：读宿主文档后转换（P2 写端口由应用实现）。
(register-command! "analyze"
  (lambda ()
    (string-upcase (documents-query "intro"))))
(submit-ui! 'panel (panel-declaration))
"#
        .to_string(),
    )]
    .into_iter()
    .collect();
    ExtensionPackage::from_parts(manifest, sources).expect("包构造")
}

/// worker 模式：准备 / 激活 / 声明交付 / 事件回投 / 受控读取。
#[test]
fn worker_ui_roundtrip_with_test_app() {
    #[cfg(feature = "test-harness")]
    {
        use std::sync::{Arc, Mutex};
        use uix::app::extensions::{ExtensionHost, ExtensionHostConfig, UiProjector};
        use uix::prelude::{State, column};
        use uix::ui::test_harness::TestApp;

        let (updates_tx, updates_rx) = std::sync::mpsc::channel::<UiUpdate>();
        let host = ExtensionHost::new()
            .with_config(ExtensionHostConfig::default())
            .with_mount("panel")
            .with_port(
                "documents-query",
                Arc::new(|arguments: &[ExtensionValue]| match arguments.first() {
                    Some(ExtensionValue::Text(key)) if key == "intro" => {
                        Ok(text("worker panel data"))
                    }
                    _ => Err("未知文档".to_string()),
                }),
            )
            .with_ui_sink(Arc::new(move |update| {
                let _ = updates_tx.send(update);
            }));
        let handle = host.spawn_worker();

        let prepared = handle.prepare(&ui_package()).expect("准备");
        let receipt = handle.activate(prepared).expect("激活");
        assert_eq!(receipt.extension_id, "panel-ext");
        // 入口顶层 submit 的初始声明在激活后交付。
        let first = recv_update(&updates_rx);
        let UiUpdate::Applied { mount, revision, node, .. } = first else {
            panic!("初始声明应 Applied：{first:?}")
        };
        assert_eq!(mount, "panel");
        assert_eq!(revision, 1);

        // 挂载区域：当前声明 + 修订 State 驱动重建。
        let current: Arc<Mutex<Option<uix::app::extensions::UiNode>>> = Arc::new(Mutex::new(None));
        *current.lock().unwrap() = Some(node);
        let revision_state = State::new(0u64);
        let projector: Arc<Mutex<UiProjector>> = Arc::new(Mutex::new(UiProjector::new(
            "panel-ext",
            receipt.generation,
            handle.event_sender(),
        )));
        let build_current = Arc::clone(&current);
        let build_projector = Arc::clone(&projector);
        let build_revision = revision_state.clone();
        let mut app = TestApp::new(
            (420.0, 320.0),
            move || {
                build_revision.get();
                let node = build_current.lock().unwrap().clone();
                match node {
                    Some(node) => build_projector.lock().unwrap().project("panel", &node),
                    None => column(Vec::<uix::prelude::ViewNode>::new()),
                }
            },
        );
        app.settle().expect("初始 settle");
        // automation 前缀：扩展 / 挂载位 / key。
        let title_text = app.text("panel-ext/panel/root/title").expect("标题");
        assert!(title_text.contains("点击次数: 0"), "{title_text}");

        // 点击按钮 → 事件 → worker 执行 → 新声明 → 应用后重投影。
        app.click("panel-ext/panel/root/btn").expect("点击");
        let second = recv_update(&updates_rx);
        let UiUpdate::Applied { revision, node, .. } = second else {
            panic!("事件后声明应 Applied：{second:?}")
        };
        assert_eq!(revision, 2);
        *current.lock().unwrap() = Some(node);
        revision_state.set(1);
        app.settle().expect("更新 settle");
        let updated = app.text("panel-ext/panel/root/title").expect("新标题");
        assert!(updated.contains("点击次数: 1"), "{updated}");
        // 扩展状态与命令共存：无窗口逻辑部分仍可调用。
        let clicks = handle
            .call_command("panel-ext", "clicks", &[])
            .expect("命令调用");
        assert_eq!(clicks, ExtensionValue::Int(1));

        // 无效声明被拒绝且原树保留。
        handle
            .call_command("panel-ext", "refresh", &[])
            .expect("refresh");
        app.click("panel-ext/panel/root/btn").expect("再次点击");
        let third = recv_update(&updates_rx);
        assert!(
            matches!(third, UiUpdate::Applied { revision: 3, .. }),
            "第三次应为 Applied：{third:?}"
        );

        handle.shutdown().expect("关停");
    }
}

fn recv_update(receiver: &std::sync::mpsc::Receiver<UiUpdate>) -> UiUpdate {
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("UI 更新超时")
}

/// 异步端口：请求-终态-回调-新声明全链路。
#[test]
fn async_port_completes_and_drives_declaration() {
    let (updates_tx, updates_rx) = std::sync::mpsc::channel::<UiUpdate>();
    let host = ExtensionHost::new()
        .with_mount("panel")
        .with_ui_sink(Arc::new(move |update| {
            let _ = updates_tx.send(update);
        }))
        .with_async_port(
            "documents-analyze",
            Arc::new(|_request: u64, _args: &[ExtensionValue], completion: AsyncCompletion| {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    completion.complete(Ok(ExtensionValue::Text("analyzed".to_string())));
                });
                Ok(())
            }),
        );
    let handle = host.spawn_worker();
    let manifest = "(uix-extension (schema-version 1) (id \"async-ext\") (version \"0.2.0\") \
                    (language r7rs-small) (entry \"main.scm\") \
                    (capabilities documents-analyze mount-panel))"
        .to_string();
    let sources: BTreeMap<String, String> = [(
        "main.scm".to_string(),
        r#"
(define status "idle")
(register-command! "start"
  (lambda ()
    (call-async "documents-analyze" '()
      (lambda (request result)
        (set! status (string-append "done:" result))
        (submit-ui! 'panel
          (list 'column (list 'key "root")
                (list 'text (list 'key "status") status)))))
    'started))
(register-command! "status" (lambda () status))
(submit-ui! 'panel
  (list 'column (list 'key "root") (list 'text (list 'key "status") status)))
"#
        .to_string(),
    )]
    .into_iter()
    .collect();
    let package = ExtensionPackage::from_parts(manifest, sources).expect("包构造");
    let prepared = handle.prepare(&package).expect("准备");
    handle.activate(prepared).expect("激活");
    let _first = recv_update(&updates_rx);

    let started = handle
        .call_command("async-ext", "start", &[])
        .expect("发起异步");
    assert_eq!(started, ExtensionValue::Symbol("started".to_string()));
    // 终态回调提交的新声明经 sink 交付。
    let done = recv_update(&updates_rx);
    let UiUpdate::Applied { node, .. } = done else {
        panic!("异步完成应驱动新声明：{done:?}")
    };
    let status = handle.call_command("async-ext", "status", &[]).expect("状态");
    assert_eq!(status, ExtensionValue::Text("done:analyzed".to_string()));
    // UiNode 校验已通过；节点文本可核对。
    let uix::app::extensions::UiNode::Column { children, .. } = &node else {
        panic!("声明应为 column")
    };
    assert_eq!(children.len(), 1);
    handle.shutdown().expect("关停");
}

/// 服务扩展点：register-service! 登记的实现在同步与 worker 模式都可调用。
#[test]
fn service_extension_point_is_callable() {
    let package = package(
        "services",
        r#"
(register-service! "normalize"
  (lambda (s) (string-downcase s)))
(register-command! "via-command"
  (lambda (s) (string-append "cmd:" s)))
"#,
    );
    let mut host = ExtensionHost::new();
    let prepared = host.prepare(&package).expect("准备");
    host.activate(prepared).expect("激活");
    let outcome = host
        .call_service("services", "normalize", &[text("OK")])
        .expect("服务调用");
    assert_eq!(outcome, text("ok"));
    match host.call_service("services", "missing", &[]) {
        Err(ExtensionError::UnknownCommand { .. }) => {}
        other => panic!("未知服务应拒绝：{other:?}"),
    }
}

/// 陈旧代事件被稳定拒绝：不更新状态、不产生新声明。
#[test]
fn stale_generation_events_are_dropped() {
    let (updates_tx, updates_rx) = std::sync::mpsc::channel::<UiUpdate>();
    let host = ExtensionHost::new()
        .with_mount("panel")
        .with_port(
            "documents-query",
            Arc::new(|_: &[ExtensionValue]| Ok(text("stale"))),
        )
        .with_ui_sink(Arc::new(move |update| {
            let _ = updates_tx.send(update);
        }));
    let handle = host.spawn_worker();
    let package = ui_package();
    let prepared = handle.prepare(&package).expect("准备");
    let receipt = handle.activate(prepared).expect("激活");
    let _initial = recv_update(&updates_rx);

    // 直接以陈旧代投递事件（模拟旧代回调迟到）。
    let sender = handle.event_sender();
    sender
        .send(uix::app::extensions::UiEvent {
            extension: "panel-ext".to_string(),
            generation: receipt.generation + 100,
            handler: "on-increment".to_string(),
            payload: uix::app::extensions::UiEventPayload::Click,
        })
        .expect("投递陈旧事件");
    let clicks = handle.call_command("panel-ext", "clicks", &[]).expect("状态");
    assert_eq!(clicks, ExtensionValue::Int(0), "陈旧事件不得改状态");
    handle.shutdown().expect("关停");
}

// ---------- P3：热替换、状态迁移、撤权与失败恢复 ----------

use uix::app::extensions::ReplacementReceipt;

fn counter_package(id: &str, version: &str, schema: i64, step: i64) -> ExtensionPackage {
    let manifest = format!(
        "(uix-extension (schema-version 1) (id \"{id}\") (version \"{version}\") \
         (language r7rs-small) (entry \"main.scm\") (state-schema-version {schema}))"
    );
    let source = format!(
        r#"
(define count 0)
(register-state-export! (lambda () (list 'count count)))
(register-state-import!
  (lambda (snapshot)
    (set! count (list-ref snapshot 1))))
(register-command! "bump"
  (lambda () (set! count (+ count {step})) count))
(register-command! "peek" (lambda () count))
(register-command! "version" (lambda () "{version}"))
"#
    );
    package_with(manifest, &[("main.scm", &source)])
}

/// 热替换：算法与状态共同升级；冲突代拒绝；失败候选保留旧代。
#[test]
fn hot_replace_upgrades_algorithm_and_migrates_state() {
    let mut host = ExtensionHost::new();
    let prepared = host
        .prepare(&counter_package("hot", "0.1.0", 1, 1))
        .expect("准备 v1");
    let receipt = host.activate(prepared).expect("激活");
    host.call_command("hot", "bump", &[]).unwrap();
    host.call_command("hot", "bump", &[]).unwrap();
    let before = host.call_command("hot", "peek", &[]).unwrap();
    assert_eq!(before, ExtensionValue::Int(2));

    // 热替换到 +10 算法，状态迁移。
    let candidate = host
        .prepare(&counter_package("hot", "0.2.0", 1, 10))
        .expect("准备 v2");
    let replacement = host
        .replace(candidate, receipt.generation)
        .expect("替换");
    assert_eq!(replacement.generation, receipt.generation + 1);
    assert!(replacement.migrated);
    assert_eq!(replacement.version, "0.2.0");
    // 状态保留且算法升级。
    let after = host.call_command("hot", "peek", &[]).unwrap();
    assert_eq!(after, ExtensionValue::Int(2));
    host.call_command("hot", "bump", &[]).unwrap();
    let stepped = host.call_command("hot", "peek", &[]).unwrap();
    assert_eq!(stepped, ExtensionValue::Int(12));

    // 冲突代拒绝：以旧代号重复替换。
    let stale_candidate = host
        .prepare(&counter_package("hot", "0.3.0", 1, 1))
        .expect("准备 v3");
    match host.replace(stale_candidate, receipt.generation) {
        Err(ExtensionError::StaleGeneration { expected, actual }) => {
            assert_eq!(expected, receipt.generation);
            assert_eq!(actual, receipt.generation + 1);
        }
        other => panic!("旧代号应冲突：{other:?}"),
    }
    // 旧代持续可用。
    let version = host.call_command("hot", "version", &[]).unwrap();
    assert_eq!(version, ExtensionValue::Text("0.2.0".to_string()));
}

/// 候选导入失败：替换被拒绝，旧代保留并继续接收。
#[test]
fn failed_migration_preserves_old_generation() {
    let mut host = ExtensionHost::new();
    let prepared = host
        .prepare(&counter_package("migrate", "0.1.0", 1, 1))
        .expect("准备");
    let receipt = host.activate(prepared).expect("激活");
    host.call_command("migrate", "bump", &[]).unwrap();

    // 坏候选：导入过程抛错。
    let manifest = "(uix-extension (schema-version 1) (id \"migrate\") (version \"0.2.0\") \
                    (language r7rs-small) (entry \"main.scm\") (state-schema-version 1))"
        .to_string();
    let source = r#"
(register-state-export! (lambda () '()))
(register-state-import! (lambda (snapshot) (error "迁移不支持")))
(register-command! "peek" (lambda () 'broken))
"#
    .to_string();
    let bad = package_with(manifest, &[("main.scm", &source)]);
    let candidate = host.prepare(&bad).expect("准备坏候选");
    match host.replace(candidate, receipt.generation) {
        Err(ExtensionError::ScriptFailure(message)) => {
            assert!(message.contains("迁移不支持"), "{message}")
        }
        other => panic!("迁移失败应拒绝：{other:?}"),
    }
    // 旧代不受影响。
    let preserved = host.call_command("migrate", "peek", &[]).unwrap();
    assert_eq!(preserved, ExtensionValue::Int(1));
    host.call_command("migrate", "bump", &[]).unwrap();
    let still = host.call_command("migrate", "peek", &[]).unwrap();
    assert_eq!(still, ExtensionValue::Int(2));
}

/// schema 不一致且无迁移：替换拒绝。
#[test]
fn schema_mismatch_replaces_are_rejected() {
    let mut host = ExtensionHost::new();
    let prepared = host
        .prepare(&counter_package("schema", "0.1.0", 1, 1))
        .expect("准备");
    let receipt = host.activate(prepared).expect("激活");
    let candidate = host
        .prepare(&counter_package("schema", "0.2.0", 2, 1))
        .expect("准备 v2");
    match host.replace(candidate, receipt.generation) {
        Err(ExtensionError::Incompatible(message)) => {
            assert!(message.contains("schema"), "{message}")
        }
        other => panic!("schema 不一致应拒绝：{other:?}"),
    }
}

/// 异步外部效果不因替换重复执行或写入新代。
#[test]
fn async_effects_are_not_duplicated_across_replace() {
    let effect_count = Arc::new(std::sync::Mutex::new(0u64));
    let host = ExtensionHost::new().with_async_port(
        "slow-port",
        {
            let effect_count = Arc::clone(&effect_count);
            Arc::new(
                move |_request: u64,
                      _args: &[ExtensionValue],
                      completion: AsyncCompletion| {
                    let effect_count = Arc::clone(&effect_count);
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        // 外部效果只发生一次。
                        *effect_count.lock().unwrap() += 1;
                        completion.complete(Ok(ExtensionValue::Int(1)));
                    });
                    Ok(())
                },
            )
        },
    );
    let handle = host.spawn_worker();
    let manifest = "(uix-extension (schema-version 1) (id \"async-replace\") (version \"0.1.0\") \
                    (language r7rs-small) (entry \"main.scm\") (capabilities slow-port))"
        .to_string();
    let source = r#"
(define results '())
(register-command! "kick"
  (lambda ()
    (call-async "slow-port" '()
      (lambda (request result) (set! results (cons result results))))
    'kicked))
(register-command! "results" (lambda () results))
(register-state-export! (lambda () (list 'results results)))
(register-state-import!
  (lambda (snapshot) (set! results (list-ref snapshot 1))))
"#
    .to_string();
    let package = package_with(manifest, &[("main.scm", &source)]);
    let prepared = handle.prepare(&package).expect("准备");
    let receipt = handle.activate(prepared).expect("激活");
    handle.call_command("async-replace", "kick", &[]).expect("发起");

    // 在途时热替换（外部效果尚未完成）。
    let v2_manifest = "(uix-extension (schema-version 1) (id \"async-replace\") (version \"0.2.0\") \
                       (language r7rs-small) (entry \"main.scm\") (capabilities slow-port) \
                       (state-schema-version 0))"
        .to_string();
    let v2_source = r#"
(define results '())
(register-command! "results" (lambda () results))
"#
    .to_string();
    let v2 = package_with(v2_manifest, &[("main.scm", &v2_source)]);
    // v2 的 schema 为 0 而活动代无 state-schema-version（默认 0）：无状态路径。
    let candidate = handle.prepare(&v2).expect("准备 v2");
    let replacement = handle.replace(candidate, receipt.generation).expect("替换");
    assert!(!replacement.migrated);

    // 等待旧请求终态到达：被代际校验丢弃，不写入新代、不重放。
    std::thread::sleep(std::time::Duration::from_millis(600));
    let results = handle
        .call_command("async-replace", "results", &[])
        .expect("新代结果");
    assert_eq!(results, ExtensionValue::Null, "旧代终态不得写入新代");
    assert_eq!(*effect_count.lock().unwrap(), 1, "外部效果恰好一次");
    handle.shutdown().expect("关停");
}

/// 撤权：新调用拒绝，实例仍可停用。
#[test]
fn revocation_blocks_calls_until_teardown() {
    let mut host = ExtensionHost::new();
    let prepared = host
        .prepare(&package("revoked", r#"(register-command! "ping" (lambda () 'ok))"#))
        .expect("准备");
    host.activate(prepared).expect("激活");
    host.revoke("revoked").expect("撤权");
    match host.call_command("revoked", "ping", &[]) {
        Err(ExtensionError::CapabilityDenied(_)) => {}
        other => panic!("撤权后应拒绝：{other:?}"),
    }
    // 实例仍在列表且可停用。
    assert_eq!(host.list().len(), 1);
    host.deactivate("revoked").expect("停用");
    assert!(host.list().is_empty());
}

/// 反复替换后引擎堆不无界增长，实例保持可用。
#[test]
fn repeated_replace_keeps_heap_bounded() {
    let mut host = ExtensionHost::new();
    let prepared = host
        .prepare(&counter_package("churn", "0.0.1", 0, 1))
        .expect("准备");
    let receipt = host.activate(prepared).expect("激活");
    let mut generation = receipt.generation;
    // 无状态包（schema 0）反复替换。
    for round in 1..=12 {
        let manifest = format!(
            "(uix-extension (schema-version 1) (id \"churn\") (version \"0.0.{round}\") \
             (language r7rs-small) (entry \"main.scm\"))"
        );
        let source = "(register-command! \"peek\" (lambda () 0))".to_string();
        let package = package_with(manifest, &[("main.scm", &source)]);
        let candidate = host.prepare(&package).expect("准备");
        let replacement: ReplacementReceipt = host.replace(candidate, generation).expect("替换");
        generation = replacement.generation;
    }
    // 替换后命令可用；活动实例只有一个。
    let peek = host.call_command("churn", "peek", &[]).expect("调用");
    assert_eq!(peek, ExtensionValue::Int(0));
    assert_eq!(host.list().len(), 1);
}

/// worker 模式 UI 草稿跨代保留（替换后投影器换代，草稿仍在）。
#[test]
fn ui_draft_survives_hot_replace_in_worker() {
    let (updates_tx, updates_rx) = std::sync::mpsc::channel::<UiUpdate>();
    let host = ExtensionHost::new()
        .with_mount("panel")
        .with_ui_sink(Arc::new(move |update| {
            let _ = updates_tx.send(update);
        }));
    let handle = host.spawn_worker();
    let make_package = |version: &str, label: &str| {
        let manifest = format!(
            "(uix-extension (schema-version 1) (id \"draft\") (version \"{version}\") \
             (language r7rs-small) (entry \"main.scm\") (capabilities mount-panel) \
             (state-schema-version 1))"
        );
        let source = format!(
            r#"
(define typed "")
(register-state-export! (lambda () (list 'typed typed)))
(define (panel)
  (list 'column (list 'key "root")
        (list 'input (list 'key "field") (list 'on-change 'on-field-change) typed)
        (list 'text (list 'key "hint") "{label}")))
(register-state-import!
  (lambda (snapshot)
    (set! typed (list-ref snapshot 1))
    (submit-ui! 'panel (panel))))
(register-handler! "on-field-change"
  (lambda (text) (set! typed text) (submit-ui! 'panel (panel))))
(submit-ui! 'panel (panel))
"#
        );
        package_with(manifest, &[("main.scm", &source)])
    };
    let prepared = handle.prepare(&make_package("0.1.0", "v1")).expect("准备");
    let receipt = handle.activate(prepared).expect("激活");
    let _first = recv_update(&updates_rx);

    // 输入草稿（经声明更新回投）。
    let sender = handle.event_sender();
    sender
        .send(uix::app::extensions::UiEvent {
            extension: "draft".to_string(),
            generation: receipt.generation,
            handler: "on-field-change".to_string(),
            payload: uix::app::extensions::UiEventPayload::Change {
                text: "保留我".to_string(),
            },
        })
        .expect("输入事件");
    let after_typing = recv_update(&updates_rx);
    assert!(matches!(after_typing, UiUpdate::Applied { .. }));

    // 热替换：typed 状态迁移，草稿由声明值带回。
    let candidate = handle.prepare(&make_package("0.2.0", "v2")).expect("准备 v2");
    let replacement = handle.replace(candidate, receipt.generation).expect("替换");
    assert!(replacement.migrated);
    let replaced = recv_update(&updates_rx);
    let UiUpdate::Applied { node, .. } = replaced else {
        panic!("替换后应交付新声明：{replaced:?}")
    };
    // 新声明 input 值 = 迁移的 typed。
    let uix::app::extensions::UiNode::Column { children, .. } = &node else {
        panic!("column")
    };
    let uix::app::extensions::UiNode::Input { value, .. } = &children[0] else {
        panic!("input")
    };
    assert_eq!(value, "保留我");
    handle.shutdown().expect("关停");
}
