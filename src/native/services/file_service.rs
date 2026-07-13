// ============================================================================
// platform/file_service.rs — 文件 I/O 服务
//
// 提供读写操作的薄封装，统一使用平台 Error/Result 类型。
// 原位于 services crate，迁入 platform 层以消除服务层。
// ============================================================================

use crate::native::{Error, Result};
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

