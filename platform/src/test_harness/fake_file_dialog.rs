//! Fake 文件对话框 — 预设返回值，记录调用参数。

use crate::api::traits::IFileDialog;

#[derive(Debug, Clone)]
pub struct FakeFileDialogState {
    /// `open` 的预设返回值
    pub open_result: Vec<String>,
    /// `save` 的预设返回值
    pub save_result: String,
    /// `open_folder` 的预设返回值
    pub open_folder_result: String,
    /// `open` 调用记录 (title, filters)
    pub open_calls: Vec<(String, String)>,
    /// `save` 调用记录 (title, filters)
    pub save_calls: Vec<(String, String)>,
    /// `open_folder` 调用记录 (title)
    pub open_folder_calls: Vec<String>,
}

impl Default for FakeFileDialogState {
    fn default() -> Self {
        Self {
            open_result: Vec::new(),
            save_result: String::new(),
            open_folder_result: String::new(),
            open_calls: Vec::new(),
            save_calls: Vec::new(),
            open_folder_calls: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct FakeFileDialog {
    pub state: FakeFileDialogState,
}

impl FakeFileDialog {
    pub fn new() -> Self {
        Self { state: FakeFileDialogState::default() }
    }

    /// 设置 `open` 返回的文件路径列表
    pub fn mock_open_result(&mut self, paths: Vec<String>) {
        self.state.open_result = paths;
    }

    /// 设置 `save` 返回的文件路径
    pub fn mock_save_result(&mut self, path: &str) {
        self.state.save_result = path.to_string();
    }

    /// 设置 `open_folder` 返回的目录路径
    pub fn mock_open_folder_result(&mut self, path: &str) {
        self.state.open_folder_result = path.to_string();
    }

    pub fn clear_history(&mut self) {
        self.state.open_calls.clear();
        self.state.save_calls.clear();
        self.state.open_folder_calls.clear();
    }
}

impl IFileDialog for FakeFileDialog {
    fn open(&mut self, title: &str, filters: &str) -> Vec<String> {
        self.state.open_calls.push((title.to_string(), filters.to_string()));
        self.state.open_result.clone()
    }

    fn save(&mut self, title: &str, filters: &str) -> String {
        self.state.save_calls.push((title.to_string(), filters.to_string()));
        self.state.save_result.clone()
    }

    fn open_folder(&mut self, title: &str) -> String {
        self.state.open_folder_calls.push(title.to_string());
        self.state.open_folder_result.clone()
    }
}
