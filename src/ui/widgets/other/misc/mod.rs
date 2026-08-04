//! 杂项组件：二维码、穿梭框、上传与水印。

// 仅在使用方启用二维码 capability 时编译实现模块。
#[cfg(feature = "qrcode")]
// 二维码实现保持在杂项组件的所有权边界内。
mod qrcode;
mod transfer;
mod upload;
mod watermark;

// 关闭二维码 capability 时不保留模块级公开入口。
#[cfg(feature = "qrcode")]
// 启用后沿既有组件路径重导出二维码类型。
pub use qrcode::*;
pub use transfer::*;
pub use upload::*;
pub use watermark::*;
