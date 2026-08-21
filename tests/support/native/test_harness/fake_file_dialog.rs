//! Fake 文件对话框 — 预设返回值，记录调用参数。

use crate::native::Result;
use crate::platform::system::IFileDialog;

#[derive(Debug, Clone, Default)]
pub struct FakeFileDialogState {
    /// `open` 的预设返回值；`None` 表示用户取消
    pub open_result: Option<Vec<String>>,
    /// `save` 的预设返回值；`None` 表示用户取消
    pub save_result: Option<String>,
    /// `open_folder` 的预设返回值；`None` 表示用户取消
    pub open_folder_result: Option<String>,
    /// `open` 调用记录 (title, filters)
    pub open_calls: Vec<(String, String)>,
    /// `save` 调用记录 (title, filters)
    pub save_calls: Vec<(String, String)>,
    /// `open_folder` 调用记录 (title)
    pub open_folder_calls: Vec<String>,
}

#[derive(Debug)]
pub struct FakeFileDialog {
    pub state: FakeFileDialogState,
}

impl FakeFileDialog {
    pub fn new() -> Self {
        Self {
            state: FakeFileDialogState::default(),
        }
    }

    /// 设置 `open` 返回的文件路径列表；`None` 表示模拟用户取消
    pub fn mock_open_result(&mut self, paths: Option<Vec<String>>) {
        self.state.open_result = paths;
    }

    /// 设置 `save` 返回的文件路径；`None` 表示模拟用户取消
    pub fn mock_save_result(&mut self, path: Option<&str>) {
        self.state.save_result = path.map(|p| p.to_string());
    }

    /// 设置 `open_folder` 返回的目录路径；`None` 表示模拟用户取消
    pub fn mock_open_folder_result(&mut self, path: Option<&str>) {
        self.state.open_folder_result = path.map(|p| p.to_string());
    }

    pub fn clear_history(&mut self) {
        self.state.open_calls.clear();
        self.state.save_calls.clear();
        self.state.open_folder_calls.clear();
    }
}

impl Default for FakeFileDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl IFileDialog for FakeFileDialog {
    fn open(&mut self, title: &str, filters: &str) -> Result<Option<Vec<String>>> {
        self.state
            .open_calls
            .push((title.to_string(), filters.to_string()));
        Ok(self.state.open_result.clone())
    }

    fn save(&mut self, title: &str, filters: &str) -> Result<Option<String>> {
        self.state
            .save_calls
            .push((title.to_string(), filters.to_string()));
        Ok(self.state.save_result.clone())
    }

    fn open_folder(&mut self, title: &str) -> Result<Option<String>> {
        self.state.open_folder_calls.push(title.to_string());
        Ok(self.state.open_folder_result.clone())
    }
}
