//! 杂项组件：二维码、穿梭框、上传与水印。

// 仅在使用方启用二维码 capability 时编译实现模块。
#[cfg(feature = "qrcode")]
// 二维码实现保持在杂项组件的所有权边界内。
mod qrcode;
mod transfer;
mod upload;
// 拆分 Upload 的公开数据契约，保持组件实现文件低于行数门禁。
mod upload_types;
// 聚焦验证 Upload 受控队列、稳定身份与类型化事实。
#[cfg(test)]
mod upload_tests;
mod watermark;

// 关闭二维码 capability 时不保留模块级公开入口。
#[cfg(feature = "qrcode")]
// 启用后沿既有组件路径重导出二维码类型。
pub use qrcode::*;
pub use transfer::*;
pub use upload::*;
// 统一从 misc 模块导出 Upload 的稳定身份、状态、事实与错误类型。
pub use upload_types::*;
pub use watermark::*;
