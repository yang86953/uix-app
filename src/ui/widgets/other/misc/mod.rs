//! 历史 `misc` 兼容路由：保留二维码、穿梭框、上传与水印的既有导入面。

// 仅在使用方启用二维码 capability 时编译实现模块。
#[cfg(feature = "qrcode")]
// 二维码实现物理归入 display，此处只保留兼容模块路径。
#[path = "../../display/qrcode/mod.rs"]
mod qrcode;
#[path = "../../input/transfer/mod.rs"]
mod transfer;
#[path = "../../input/upload/mod.rs"]
mod upload;
#[path = "../../display/watermark/mod.rs"]
mod watermark;

// 关闭二维码 capability 时不保留模块级公开入口。
#[cfg(feature = "qrcode")]
// 启用后沿既有组件路径重导出二维码类型。
pub use qrcode::*;
pub use transfer::*;
pub use upload::*;
pub use watermark::*;
