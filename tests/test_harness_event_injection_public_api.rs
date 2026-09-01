// 声明本文件只在启用 test-harness 时编译系统事件注入的公开 API 测试。
#![cfg(feature = "test-harness")]

// 引入文档承诺的无窗口驱动与事件契约。
use uix::prelude::*;
use uix::ui::test_harness::TestApp;

// 验证系统事件注入按逻辑坐标走真实树级分发路径。
#[test]
fn dispatch_system_event_delivers_file_drop_to_upload_queue() {
    // 拖放入队会校验文件可读性，先落一个真实临时文件。
    let path = std::env::temp_dir().join(format!("uix-dispatch-drop-{}.png", std::process::id()));
    std::fs::write(&path, b"png-bytes").expect("临时文件应可写入");

    // 由业务侧持有队列真值，组件只接收句柄。
    let queue = State::new(Vec::<UploadFile>::new());
    let upload_queue = queue.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        ViewNode::leaf(
            Upload::new()
                .files(&upload_queue)
                .drag(true)
                .multiple(true)
                .max_count(4),
        )
        .automation_id("upload.drop")
    });
    // 语义快照的可见中心与事件 position 同属逻辑坐标空间。
    let center = app
        .snapshot()
        .find("upload.drop")
        .ok()
        .and_then(|node| node.center())
        .expect("上传区域应存在且可见");

    let result = app
        .dispatch_system_event(&SystemEvent::FileDrop {
            files: vec![path.to_string_lossy().into_owned()],
            position: center,
        })
        .expect("系统事件注入应完成收敛");

    // 真实树级路径应报告已处理并更新受控队列。
    assert_eq!(result, EventResult::Handled);
    assert_eq!(queue.get().len(), 1);
    let _ = std::fs::remove_file(&path);
}
