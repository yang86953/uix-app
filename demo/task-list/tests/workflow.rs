#![cfg(feature = "test-harness")]
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use uix_app::ui::test_harness::TestApp;
use uix_task_list::app::TaskApp;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct DataDir(PathBuf);
impl DataDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "uix-task-list-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("tasks.json")
    }
}
impl Drop for DataDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn uix_input_callbacks_validation_identity_navigation_and_save() {
    let dir = DataDir::new();
    let app = TaskApp::new(dir.file()).unwrap();
    let root = app.clone();
    let mut ui = TestApp::new((720.0, 680.0), move || root.view());
    ui.invoke("add").unwrap();
    assert!(app.status().contains("校验失败"));
    ui.insert_text("draft", "学习 UIX").unwrap();
    ui.invoke("add").unwrap();
    assert_eq!(app.data().tasks[0].title, "学习 UIX");
    ui.invoke("toggle-1").unwrap();
    assert!(
        app.data().tasks[0].done,
        "first toggle must change domain state exactly once"
    );
    ui.invoke("toggle-1").unwrap(); // 同一拥有 String ID 的回调可重复调用。
    assert!(!app.data().tasks[0].done);
    ui.insert_text("draft", "保存清单").unwrap();
    ui.invoke("add").unwrap();
    ui.invoke("remove-1").unwrap();
    assert_eq!(app.data().tasks[0].id, 2);
    ui.invoke("nav-stats").unwrap();
    assert!(ui.snapshot().find("stats-result").is_ok());
    ui.invoke("nav-tasks").unwrap();
    assert!(ui.snapshot().find("toggle-2").is_ok());
    assert!(app.is_dirty());
    ui.invoke("save").unwrap();
    assert!(!app.is_dirty());
    assert!(app.status().starts_with("已保存"));
    assert!(dir.file().is_file());
    app.shutdown().unwrap();
}

#[test]
fn actual_save_failure_keeps_dirty_and_retry_works() {
    let dir = DataDir::new();
    let app = TaskApp::new(dir.file()).unwrap();
    app.add("未保存".into());
    // 在已成功 load 后用同名目录制造真实文件系统失败，不依赖测试用户权限。
    std::fs::create_dir(dir.file()).unwrap();
    app.save();
    assert!(app.status().starts_with("保存失败"));
    assert!(app.is_dirty());
    std::fs::remove_dir(dir.file()).unwrap();
    app.save();
    assert!(app.status().starts_with("已保存"));
    assert!(!app.is_dirty());
    app.shutdown().unwrap();
}

#[test]
fn malformed_file_stays_visible_and_cannot_be_overwritten() {
    let dir = DataDir::new();
    std::fs::write(dir.file(), b"not-json").unwrap();
    let app = TaskApp::new(dir.file()).unwrap();
    assert!(app.status().starts_with("加载失败"));
    let root = app.clone();
    let mut ui = TestApp::new((720.0, 680.0), move || root.view());
    assert!(ui.invoke("add").is_err());
    assert!(ui.invoke("save").is_err());
    app.add("不能覆盖".into());
    app.save();
    assert_eq!(std::fs::read(dir.file()).unwrap(), b"not-json");
    assert!(app.data().tasks.is_empty());
    app.shutdown().unwrap();
}

#[test]
fn full_bounded_list_scrolls_to_last_task_and_back() {
    let dir = DataDir::new();
    let app = TaskApp::new(dir.file()).unwrap();
    for n in 1..=uix_task_list::domain::MAX_TASKS {
        app.add(format!("任务 {n}"));
    }
    let root = app.clone();
    let mut ui = TestApp::new((720.0, 680.0), move || root.view());
    assert!(
        ui.snapshot()
            .find("toggle-1")
            .unwrap()
            .visible_bounds
            .is_some()
    );
    assert!(
        ui.snapshot()
            .find("toggle-128")
            .unwrap()
            .visible_bounds
            .is_none()
    );
    ui.scroll("task-scroll", uix_app::core::Point::new(0.0, -100_000.0))
        .unwrap();
    assert!(
        ui.snapshot()
            .find("toggle-128")
            .unwrap()
            .visible_bounds
            .is_some()
    );
    ui.scroll("task-scroll", uix_app::core::Point::new(0.0, 100_000.0))
        .unwrap();
    assert!(
        ui.snapshot()
            .find("toggle-1")
            .unwrap()
            .visible_bounds
            .is_some()
    );
    assert_eq!(app.data().tasks.len(), 128);
    app.shutdown().unwrap();
}
