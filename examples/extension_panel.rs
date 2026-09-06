//! 动态扩展示例：真窗口中的 Lisp 扩展面板（P2 验收应用）。
//!
//! 运行：`cargo run --release --features extensions,agent-control --example extension_panel`
//!
//! 装载一个文本处理扩展：点击「增加」更新扩展状态并提交新声明；输入
//! 回显到标题；「分析」走异步端口，完成后状态文本更新。全部 UI 变更经
//! `(uix ui)` 声明提交 → 宿主校验 → 投影，UI 线程不执行任何 Lisp。
//!
//! Agent 验收：先 `apps` 枚举并绑定本实例，再按 automation_id
//! （`panel-ext/panel/...`）读取与操作。

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use uix::app::extensions::{
    AsyncCompletion, ExtensionAsyncPort, ExtensionHost, ExtensionPackage, ExtensionUiHandle,
    ExtensionValue, UiNode, UiProjector, UiUpdate,
};
use uix::prelude::*;

fn main() {
    let (updates_tx, updates_rx) = mpsc::channel::<UiUpdate>();
    let current: Arc<Mutex<Option<UiNode>>> = Arc::new(Mutex::new(None));
    let revision = State::new(0u64);
    let projector: Arc<Mutex<Option<UiProjector>>> = Arc::new(Mutex::new(None));
    let worker_handle: Arc<Mutex<Option<ExtensionUiHandle>>> = Arc::new(Mutex::new(None));

    // 宿主端口：只读文档查询与受控业务改写（写端口由应用实现真值）。
    let documents: Arc<Mutex<BTreeMap<String, String>>> = Arc::new(Mutex::new(
        [("intro".to_string(), "UIX 动态扩展示例文档".to_string())]
            .into_iter()
            .collect(),
    ));
    let analyze_port: ExtensionAsyncPort = {
        let documents = Arc::clone(&documents);
        Arc::new(move |request: u64, args: &[ExtensionValue], completion: AsyncCompletion| {
            let documents = Arc::clone(&documents);
            let key = match args.first() {
                Some(ExtensionValue::Text(key)) => key.clone(),
                _ => "intro".to_string(),
            };
            thread::spawn(move || {
                thread::sleep(std::time::Duration::from_millis(300));
                let analyzed = documents
                    .lock()
                    .unwrap()
                    .get(&key)
                    .cloned()
                    .map(|text| format!("已分析 {} 字符", text.chars().count()))
                    .unwrap_or_else(|| "文档不存在".to_string());
                completion.complete(Ok(ExtensionValue::Text(analyzed)));
            });
            let _ = request;
            Ok(())
        })
    };

    let host = ExtensionHost::new()
        .with_mount("panel")
        .with_port(
            "documents-query",
            Arc::new({
                let documents = Arc::clone(&documents);
                move |args: &[ExtensionValue]| match args.first() {
                    Some(ExtensionValue::Text(key)) => documents
                        .lock()
                        .unwrap()
                        .get(key)
                        .cloned()
                        .map(ExtensionValue::Text)
                        .ok_or_else(|| "未知文档".to_string()),
                    _ => Err("需要文档键".to_string()),
                }
            }),
        )
        .with_async_port("documents-analyze", analyze_port)
        .with_ui_sink(Arc::new(move |update| {
            let _ = updates_tx.send(update);
        }));
    let handle = host.spawn_worker();

    let build_current = Arc::clone(&current);
    let build_revision = revision.clone();
    let build_projector = Arc::clone(&projector);

    let app = App::new()
        .title("UIX 动态扩展示例")
        .size(460, 420)
        .enable_agent_control()
        .on_start({
            let handle = handle.clone();
            let current = Arc::clone(&current);
            let revision = revision.clone();
            let projector = Arc::clone(&projector);
            let worker_handle = Arc::clone(&worker_handle);
            move |app_handle| {
                // 声明交付桥：sink（扩展线程）→ post_to_ui（窗口 owner thread）。
                let app_handle = app_handle.clone();
                let update_source = {
                    let current = Arc::clone(&current);
                    let revision = revision.clone();
                    move |update: UiUpdate| {
                        let current = Arc::clone(&current);
                        let revision = revision.clone();
                        app_handle.post_to_ui(move || {
                            if let UiUpdate::Applied { node, .. } = update {
                                *current.lock().unwrap() = Some(node);
                                revision.update(|value| { *value += 1; });
                            }
                        });
                    }
                };
                thread::spawn(move || {
                    while let Ok(update) = updates_rx.recv() {
                        update_source(update);
                    }
                });
                // 装载扩展：准备 → 激活（初始声明经桥回投）。
                let package = extension_package();
                match handle.prepare(&package)
                    .and_then(|prepared| handle.activate(prepared))
                {
                    Ok(receipt) => {
                        *projector.lock().unwrap() = Some(UiProjector::new(
                            &receipt.extension_id,
                            receipt.generation,
                            handle.event_sender(),
                        ));
                        *worker_handle.lock().unwrap() = Some(handle.clone());
                    }
                    Err(error) => {
                        *current.lock().unwrap() = Some(load_failed_node(&error.to_string()));
                        revision.update(|value| { *value += 1; });
                    }
                }
            }
        })
        .root(move || {
            build_revision.get();
            let node = build_current.lock().unwrap().clone();
            match (node, build_projector.lock().unwrap().as_mut()) {
                (Some(node), Some(projector)) => projector.project("panel", &node),
                _ => column(Vec::<ViewNode>::new()),
            }
        });

    let exit = app.run();
    if let Some(handle) = worker_handle.lock().unwrap().as_ref() {
        let _ = handle.clone().shutdown();
    }
    std::process::exit(exit);
}

fn load_failed_node(message: &str) -> UiNode {
    UiNode::Column {
        key: "root".to_string(),
        padding: Some(16.0),
        gap: None,
        background: None,
        children: vec![UiNode::Text {
            key: "error".to_string(),
            content: format!("扩展装载失败：{message}"),
            color: None,
            size: Some(14.0),
        }],
    }
}

fn extension_package() -> ExtensionPackage {
    let manifest = "(uix-extension (schema-version 1) (id \"panel-ext\") (version \"0.3.0\") \
                    (language r7rs-small) (entry \"main.scm\") \
                    (capabilities documents-query documents-analyze mount-panel))"
        .to_string();
    let source = r#"
(define clicks 0)
(define field "")
(define status "就绪")
(define (declaration)
  (list 'column (list 'key "root") (list 'pad 14.0) (list 'gap 8.0)
        (list 'text (list 'key "title") (list 'size 15.0)
              (string-append "点击次数: " (number->string clicks)))
        (list 'text (list 'key "status")
              (string-append "状态: " status))
        (list 'row (list 'key "controls") (list 'gap 8.0)
              (list 'button (list 'key "btn") (list 'on-click 'on-increment) "增加")
              (list 'button (list 'key "analyze") (list 'on-click 'on-analyze) "分析"))
        (list 'input (list 'key "field") (list 'placeholder "输入文本")
              (list 'on-change 'on-field-change) "")
        (list 'list (list 'key "items")
              (list "只读端口 documents-query"
                    "异步端口 documents-analyze"
                    "挂载位 panel"))))
(register-handler! "on-increment"
  (lambda ()
    (set! clicks (+ clicks 1))
    (submit-ui! 'panel (declaration))))
(register-handler! "on-field-change"
  (lambda (text)
    (set! field text)
    (submit-ui! 'panel (declaration))))
(register-handler! "on-analyze"
  (lambda ()
    (set! status "分析中…")
    (submit-ui! 'panel (declaration))
    (call-async "documents-analyze" (list "intro")
      (lambda (request result)
        (set! status result)
        (submit-ui! 'panel (declaration))))))
(register-command! "field-length" (lambda () (string-length field)))
(submit-ui! 'panel (declaration))
"#;
    let sources: BTreeMap<String, String> =
        [("main.scm".to_string(), source.to_string())]
            .into_iter()
            .collect();
    ExtensionPackage::from_parts(manifest, sources).expect("示例包构造")
}
