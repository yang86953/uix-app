//! Fake 文件系统 — 内存文件树。预设文件内容，追踪读取记录。
//!
//! 所有读取方法通过 `&self` 访问，使用 `RefCell` 追踪调用历史。

use crate::api::traits::IFileSystem;
use crate::types::SpecialDir;
use crate::{Errc, Error, Result};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct FakeFileSystemState {
    /// 文件树：路径 → 内容
    pub files: HashMap<String, Vec<u8>>,
    /// 存在的目录路径
    pub dirs: HashSet<String>,
    /// 特殊目录映射（用 Vec 因为 SpecialDir 未实现 Hash）
    pub special_dirs: Vec<(SpecialDir, String)>,
    /// 可执行文件路径
    pub executable_path: String,
    /// 可执行文件所在目录
    pub executable_dir: String,
}

impl Default for FakeFileSystemState {
    fn default() -> Self {
        Self {
            files: HashMap::new(),
            dirs: HashSet::new(),
            special_dirs: vec![
                (SpecialDir::Home, "/home/user".to_string()),
                (SpecialDir::Temp, "/tmp".to_string()),
                (SpecialDir::AppData, "/home/user/.config".to_string()),
                (
                    SpecialDir::LocalAppData,
                    "/home/user/.local/share".to_string(),
                ),
            ],
            executable_path: "/usr/bin/uix-app".to_string(),
            executable_dir: "/usr/bin".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct FakeFileSystem {
    /// 文件系统状态（公开读写）
    pub state: FakeFileSystemState,
    /// `read_file` 调用记录（通过 RefCell 支持 &self 追踪）
    pub read_calls: RefCell<Vec<String>>,
}

impl FakeFileSystem {
    pub fn new() -> Self {
        Self {
            state: FakeFileSystemState::default(),
            read_calls: RefCell::new(Vec::new()),
        }
    }

    /// 向内存文件系统添加一个文件
    pub fn add_file(&mut self, path: &str, content: Vec<u8>) {
        self.state.files.insert(path.to_string(), content);
        if let Some(parent) = std::path::Path::new(path).parent() {
            self.state.dirs.insert(parent.to_string_lossy().to_string());
        }
    }

    /// 添加一个空目录
    pub fn add_dir(&mut self, path: &str) {
        self.state.dirs.insert(path.to_string());
    }

    /// 设置特殊目录返回值
    pub fn set_special_dir(&mut self, dir: SpecialDir, path: &str) {
        self.state.special_dirs.retain(|(d, _)| *d != dir);
        self.state.special_dirs.push((dir, path.to_string()));
    }

    fn get_special_dir_impl(&self, dir: SpecialDir) -> String {
        self.state
            .special_dirs
            .iter()
            .find(|(d, _)| *d == dir)
            .map(|(_, p)| p.clone())
            .unwrap_or_default()
    }

    pub fn clear_history(&mut self) {
        self.read_calls.borrow_mut().clear();
    }
}

impl IFileSystem for FakeFileSystem {
    fn get_special_dir(&self, dir: SpecialDir) -> String {
        self.get_special_dir_impl(dir)
    }

    fn executable_path(&self) -> String {
        self.state.executable_path.clone()
    }

    fn executable_dir(&self) -> String {
        self.state.executable_dir.clone()
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        self.read_calls.borrow_mut().push(path.to_string());
        self.state
            .files
            .get(path)
            .cloned()
            .ok_or_else(|| Error::new(Errc::NotFound, format!("fake file not found: {}", path)))
    }
}
