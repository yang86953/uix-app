//! `src/platform/adapters/transport/unsupported.rs` 的仅测试占位探针。
//! 自源文件移入，模块层级不变，经 #[path] 引用，不进发布包；保留待重新接线。
#![allow(dead_code)]

use std::io;
use std::path::Path;

#[cfg(test)]
pub(super) fn discovery_permissions_are_private_for_test(_path: &Path) -> io::Result<Option<bool>> {
    Ok(None)
}

#[cfg(test)]
pub(super) fn endpoint_permissions_are_private_for_test(
    _endpoint: &str,
) -> io::Result<Option<bool>> {
    Ok(None)
}
