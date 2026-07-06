use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_path(name: &str) -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let p = std::env::temp_dir().join(format!("uix_fs_test_{}_{}", id, name));
    p.to_string_lossy().to_string()
}

#[test]
fn write_and_read_string() {
    let fs = FileService::new();
    let path = unique_path("hello.txt");
    fs.write_string(&path, "Hello UIX!").unwrap();
    assert!(fs.exists(&path));
    assert_eq!(fs.read_to_string(&path).unwrap(), "Hello UIX!");
    let _ = fs.remove(&path);
}

#[test]
fn write_and_read_bytes() {
    let fs = FileService::new();
    let path = unique_path("data.bin");
    let data = vec![0u8, 1, 2, 3, 255];
    fs.write_bytes(&path, &data).unwrap();
    assert_eq!(fs.read_bytes(&path).unwrap(), data);
    let _ = fs.remove(&path);
}

#[test]
fn append_string() {
    let fs = FileService::new();
    let path = unique_path("append.txt");
    fs.write_string(&path, "Line 1\n").unwrap();
    fs.append_string(&path, "Line 2\n").unwrap();
    assert_eq!(fs.read_to_string(&path).unwrap(), "Line 1\nLine 2\n");
    let _ = fs.remove(&path);
}

#[test]
fn read_lines() {
    let fs = FileService::new();
    let path = unique_path("lines.txt");
    fs.write_string(&path, "a\nb\nc").unwrap();
    assert_eq!(fs.read_lines(&path).unwrap(), vec!["a", "b", "c"]);
    let _ = fs.remove(&path);
}

#[test]
fn remove_file() {
    let fs = FileService::new();
    let path = unique_path("todelete.txt");
    fs.write_string(&path, "delete me").unwrap();
    assert!(fs.exists(&path));
    fs.remove(&path).unwrap();
    assert!(!fs.exists(&path));
}

#[test]
fn file_size_returns_correct_value() {
    let fs = FileService::new();
    let path = unique_path("size.txt");
    fs.write_string(&path, "12345").unwrap();
    assert_eq!(fs.file_size(&path).unwrap(), 5);
    let _ = fs.remove(&path);
}

#[test]
fn read_nonexistent_returns_error() {
    let fs = FileService::new();
    let result = fs.read_to_string(&format!(
        "/nonexistent/uix_test_{}",
        TEST_COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    assert!(result.is_err());
}

#[test]
fn auto_creates_parent_directories() {
    let fs = FileService::new();
    let path = unique_path("sub/nested/test.txt");
    fs.write_string(&path, "nested").unwrap();
    assert!(fs.exists(&path));
    let _ = fs.remove(&path);
    let _ = std::fs::remove_dir_all(std::path::Path::new(&path).parent().unwrap());
}

#[test]
fn exists_returns_false_for_nonexistent() {
    let fs = FileService::new();
    assert!(!fs.exists("/nonexistent/uix_fs_nonexistent_file"));
}
