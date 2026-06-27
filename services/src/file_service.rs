use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use uix_platform::{Error, Result};

/// File I/O service providing read/write operations with proper error handling.
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

    pub fn read_to_string(&self, path: &str) -> Result<String> {
        fs::read_to_string(path)
            .map_err(|e| Error::io_error(format!("failed to read '{}': {}", path, e)))
    }

    pub fn read_bytes(&self, path: &str) -> Result<Vec<u8>> {
        fs::read(path).map_err(|e| Error::io_error(format!("failed to read '{}': {}", path, e)))
    }

    pub fn write_string(&self, path: &str, content: &str) -> Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)
            .map_err(|e| Error::io_error(format!("failed to write '{}': {}", path, e)))
    }

    pub fn write_bytes(&self, path: &str, data: &[u8]) -> Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, data)
            .map_err(|e| Error::io_error(format!("failed to write '{}': {}", path, e)))
    }

    pub fn append_string(&self, path: &str, content: &str) -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    pub fn read_lines(&self, path: &str) -> Result<Vec<String>> {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = Vec::new();
        for line in reader.lines() {
            lines.push(line?);
        }
        Ok(lines)
    }

    pub fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    pub fn remove(&self, path: &str) -> Result<()> {
        fs::remove_file(path)
            .map_err(|e| Error::io_error(format!("failed to remove '{}': {}", path, e)))
    }

    pub fn file_size(&self, path: &str) -> Result<u64> {
        fs::metadata(path)
            .map(|m| m.len())
            .map_err(|e| Error::io_error(format!("failed to stat '{}': {}", path, e)))
    }
}

