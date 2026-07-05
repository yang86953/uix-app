// ============================================================================
// platform/file_service.rs — 文件 I/O 服务
//
// 提供读写操作的薄封装，统一使用平台 Error/Result 类型。
// 原位于 services crate，迁入 platform 层以消除服务层。
// ============================================================================

use crate::platform::{Error, Result};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// 文件 I/O 服务，提供带统一错误处理的读写操作。
#[derive(Debug, Clone, Copy)]
pub struct FileService;

impl Default for FileService {
    fn default() -> Self {
        Self
    }
}

impl FileService {
    pub fn new() -> Self {
        Self
    }

    /// 读取文件全部内容为字符串。
    pub fn read_to_string(&self, path: &str) -> Result<String> {
        fs::read_to_string(path)
            .map_err(|e| Error::io_error(format!("failed to read '{}': {}", path, e)))
    }

    /// 读取文件全部内容为字节数组。
    pub fn read_bytes(&self, path: &str) -> Result<Vec<u8>> {
        fs::read(path).map_err(|e| Error::io_error(format!("failed to read '{}': {}", path, e)))
    }

    /// 将字符串写入文件（自动创建父目录）。
    pub fn write_string(&self, path: &str, content: &str) -> Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)
            .map_err(|e| Error::io_error(format!("failed to write '{}': {}", path, e)))
    }

    /// 将字节数组写入文件（自动创建父目录）。
    pub fn write_bytes(&self, path: &str, data: &[u8]) -> Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, data)
            .map_err(|e| Error::io_error(format!("failed to write '{}': {}", path, e)))
    }

    /// 追加字符串到文件末尾。
    pub fn append_string(&self, path: &str, content: &str) -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    /// 按行读取文件。
    pub fn read_lines(&self, path: &str) -> Result<Vec<String>> {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = Vec::new();
        for line in reader.lines() {
            lines.push(line?);
        }
        Ok(lines)
    }

    /// 检查文件是否存在。
    pub fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    /// 删除文件。
    pub fn remove(&self, path: &str) -> Result<()> {
        fs::remove_file(path)
            .map_err(|e| Error::io_error(format!("failed to remove '{}': {}", path, e)))
    }

    /// 获取文件大小。
    pub fn file_size(&self, path: &str) -> Result<u64> {
        fs::metadata(path)
            .map(|m| m.len())
            .map_err(|e| Error::io_error(format!("failed to stat '{}': {}", path, e)))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 测试
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
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

    #[test]
    fn file_service_is_clone_and_copy() {
        let fs1 = FileService::new();
        let fs2 = fs1;
        let _fs3 = fs2;
    }
}
