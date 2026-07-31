//! 平台业务服务。

pub mod file_service;
pub mod filesystem;
pub mod notification;

pub use filesystem::{FileSystemCore, SpecialDirProvider};
