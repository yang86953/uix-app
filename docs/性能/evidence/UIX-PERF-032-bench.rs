#![cfg(feature = "extensions")]
//! 扩展引擎参考负载测量（P4 交付证据；不入项目测试）。
use std::collections::BTreeMap;
use std::time::Instant;
use uix::app::extensions::*;

fn pkg(id: &str, src: &str) -> ExtensionPackage {
    let manifest = format!("(uix-extension (schema-version 1) (id \"{id}\") (version \"0.1.0\") (language r7rs-small) (entry \"main.scm\"))");
    let sources: BTreeMap<String, String> = [("main.scm".to_string(), src.to_string())].into_iter().collect();
    ExtensionPackage::from_parts(manifest, sources).unwrap()
}

#[test]
fn extension_reference_loads() {
    // 场景 A：无窗口纯逻辑扩展（文本转换 + 状态计数）。
    let logic = pkg("bench-logic", r#"
(define uses 0)
(define (rev s) (list->string (reverse (string->list s))))
(register-command! "transform"
  (lambda (s) (set! uses (+ uses 1)) (rev s)))
(register-command! "tail-loop"
  (lambda (n) (let loop ((i 0) (acc 0))
    (if (= i n) acc (loop (+ i 1) (+ acc 1))))))
(register-command! "churn"
  (lambda ()
    (let ((ring (cons 1 (cons 2 '()))))
      (set-cdr! (cdr ring) ring)
      'churned)))
"#);
    let mut host = ExtensionHost::new().with_config(ExtensionHostConfig {
        command_wall_time_ms: 30_000,
        maximum_fuel: 2_000_000_000,
        ..Default::default()
    });
    // 冷装载：prepare（含候选求值）+ activate。
    let start = Instant::now();
    let prepared = host.prepare(&logic).unwrap();
    let prepare_us = start.elapsed().as_micros();
    let start = Instant::now();
    host.activate(prepared).unwrap();
    let activate_us = start.elapsed().as_micros();

    // 命令端到端：短转换 ×1000。
    let start = Instant::now();
    for _ in 0..1000 {
        host.call_command("bench-logic", "transform", &[ExtensionValue::Text("benchmark-string".into())]).unwrap();
    }
    let transform_ms = start.elapsed().as_millis();

    // 尾循环 10 万次（解释器步速）。
    let start = Instant::now();
    host.call_command("bench-logic", "tail-loop", &[ExtensionValue::Int(100_000)]).unwrap();
    let loop_ms = start.elapsed().as_millis();

    // 循环分配 ×2000 后的存活堆（回收参与）。
    for _ in 0..2000 {
        host.call_command("bench-logic", "churn", &[]).unwrap();
    }
    let status = host.list();
    println!(
        "LOADS prepare={prepare_us}us activate={activate_us}us transform_x1000={transform_ms}ms tail_100k={loop_ms}ms instances={}",
        status.len()
    );
    println!("REPORT 冷装载(prep+act) {}us+{}us; 1000 次转换 {}ms; 10 万尾循环 {}ms", prepare_us, activate_us, transform_ms, loop_ms);
    host.shutdown().unwrap();
}
