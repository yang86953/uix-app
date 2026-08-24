//! 历史 `misc` 兼容路由：保留二维码、穿梭框、上传与水印的既有导入面。

// 仅在使用方启用二维码 capability 时编译实现模块。
#[cfg(feature = "qrcode")]
// 二维码实现物理归入 display，此处只保留兼容模块路径。
#[path = "../../display/qrcode/mod.rs"]
mod qrcode;
#[path = "../../input/transfer/mod.rs"]
mod transfer;
#[path = "../../input/upload.rs"]
mod upload;
// Upload 的几何、排版与展示格式保持在组件私有 presentation 边界。
#[path = "../../input/upload_presentation.rs"]
mod upload_presentation;
// 拆分 Upload 的公开数据契约，保持组件实现文件低于行数门禁。
#[path = "../../input/upload_types.rs"]
mod upload_types;
// 聚焦验证 Upload 受控队列、稳定身份与类型化事实。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/other/misc/upload_tests.rs"]
mod upload_tests;
#[path = "../../display/watermark/mod.rs"]
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
