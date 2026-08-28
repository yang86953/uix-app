//! 平台业务服务。

// Linux Provider 与主机纯逻辑测试共享外部文件对话框退出分类。
#[cfg(any(test, target_os = "linux"))]
pub(crate) mod file_dialog_process;
