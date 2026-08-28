//! Platform System 内部使用的基础系统服务协议。

// 系统信息值与端口由独立中立叶唯一持有。
pub(crate) mod info;
// 文件系统目录值由独立中立叶唯一持有。
pub(crate) mod filesystem;
