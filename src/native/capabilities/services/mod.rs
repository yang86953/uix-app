//! 平台业务服务。

pub(crate) mod filesystem;
pub(crate) mod notification;

pub(crate) use filesystem::{FileSystemCore, SpecialDirProvider};
