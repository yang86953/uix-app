//! 动态扩展示例：真窗口中的 Lisp 扩展面板（P2 验收应用）。
//!
//! 运行：`cargo run --release --features extensions --example extension_panel`
//!
//! 装载一个文本处理扩展：点击「增加」更新扩展状态并提交新声明；输入
//! 回显到标题；「分析」走异步端口，完成后状态文本更新。全部 UI 变更经
//! `(uix ui)` 声明提交 → 宿主校验 → 投影，UI 线程不执行任何 Lisp。
//!
//! 本模板的投影器和声明回传绑定用户窗口，不再默认暴露前台 Agent 端点。
//! AI 接入须另建后台根、投影器和 UI worker；只显式共享业务端口，见 Agent后台操作面文档。

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use uix_app::app::extensions::{
    AsyncCompletion, ExtensionAsyncPort, ExtensionHost, ExtensionPackage, ExtensionUiHandle,
    ExtensionValue, UiNode, UiProjector, UiUpdate,
};
use uix_app::prelude::*;

fn main() {
    let hot_replace = std::env::args().any(|argument| argument == "--hot-replace");
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
        Arc::new(
            move |request: u64, args: &[ExtensionValue], completion: AsyncCompletion| {
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
            },
        )
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
        // 前台专属投影器不能克隆成“独立”后台根，因此不启用旧的同窗控制入口。
        .on_start({
            let handle = handle.clone();
            let current = Arc::clone(&current);
            let revision = revision.clone();
            let projector = Arc::clone(&projector);
            let worker_handle = Arc::clone(&worker_handle);
            move |app_handle| {
                // 声明交付桥：sink（扩展线程）→ post_to_ui（窗口 owner thread）。
                let app_handle = app_handle.clone();
                let bridge_projector = Arc::clone(&projector);
                let update_source = {
                    let current = Arc::clone(&current);
                    let revision = revision.clone();
                    move |update: UiUpdate| {
                        let current = Arc::clone(&current);
                        let revision = revision.clone();
                        let projector = Arc::clone(&bridge_projector);
                        let projector = Arc::clone(&projector);
                        app_handle.post_to_ui(move || {
                            if let UiUpdate::Applied {
                                mut node,
                                generation,
                                ..
                            } = update
                            {
                                // 热替换后代际切换：事件授权与声明来源一致。
                                if let Some(projector) = projector.lock().unwrap().as_mut() {
                                    projector.update_generation(generation);
                                    // reset 是本次 Applied 的一次性指令：执行后消耗
                                    // 标记，后续重投影不得再次覆盖本地编辑。
                                    projector.consume_declaration_resets(&mut node);
                                }
                                *current.lock().unwrap() = Some(node);
                                revision.update(|value| {
                                    *value += 1;
                                });
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
                let package = extension_package("0.3.0", 1);
                match handle
                    .prepare(&package)
                    .and_then(|prepared| handle.activate(prepared))
                {
                    Ok(receipt) => {
                        *projector.lock().unwrap() = Some(UiProjector::new(
                            &receipt.extension_id,
                            receipt.generation,
                            handle.event_sender(),
                        ));
                        *worker_handle.lock().unwrap() = Some(handle.clone());
                        // 演示热替换：--hot-replace 启动 5 秒后以 v0.4
                        // （+2 步进 + 标题标记）升级，状态与界面共同迁移。
                        if hot_replace {
                            let handle = handle.clone();
                            thread::spawn(move || {
                                thread::sleep(std::time::Duration::from_secs(5));
                                let upgraded = extension_package("0.4.0", 2);
                                match handle.prepare(&upgraded).and_then(|candidate| {
                                    handle.replace(candidate, receipt.generation)
                                }) {
                                    Ok(replacement) => eprintln!(
                                        "[hot-replace] {} -> v{} generation {} migrated={}",
                                        replacement.extension_id,
                                        replacement.version,
                                        replacement.generation,
                                        replacement.migrated
                                    ),
                                    Err(error) => eprintln!("[hot-replace] 失败：{error}"),
                                }
                            });
                        }
                    }
                    Err(error) => {
                        *current.lock().unwrap() = Some(load_failed_node(&error.to_string()));
                        revision.update(|value| {
                            *value += 1;
                        });
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

fn extension_package(version: &str, step: i64) -> ExtensionPackage {
    let manifest = format!(
        "(uix-extension (schema-version 1) (id \"panel-ext\") (version \"{version}\") \
         (language r7rs-small) (entry \"main.scm\") \
         (capabilities documents-query documents-analyze mount-panel) \
         (state-schema-version 1))"
    );
    let title_prefix = if step == 1 {
        "点击次数"
    } else {
        "点击次数(v2)"
    };
    let source = format!(
        r#"
(define clicks 0)
(define field "")
(define status "就绪")
(register-state-export! (lambda () (list 'clicks clicks) ))
(register-state-import!
  (lambda (snapshot)
    (set! clicks (list-ref snapshot 1))
    (submit-ui! 'panel (declaration))))
(define (declaration)
  (list 'column (list 'key "root") (list 'pad 14.0) (list 'gap 8.0)
        (list 'text (list 'key "title") (list 'size 15.0)
              (string-append "{title_prefix}: " (number->string clicks)))
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
    (set! clicks (+ clicks {step}))
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
"#,
    );
    let sources: BTreeMap<String, String> =
        [("main.scm".to_string(), source)].into_iter().collect();
    ExtensionPackage::from_parts(manifest, sources).expect("示例包构造")
}
