//! `src/platform/adapters/transport/unix.rs` 的仅测试探针：owner-only 权限验收。
//! 自源文件移入，模块层级不变，经 #[path] 引用，不进发布包；历史消费者为
//! owner-only transport ACL 验收（0342f92da），测试移除后保留待重新接线。
#![allow(dead_code)]

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

#[cfg(test)]
pub(super) fn discovery_permissions_are_private_for_test(path: &Path) -> io::Result<Option<bool>> {
    Ok(Some(fs::metadata(path)?.permissions().mode() & 0o077 == 0))
}

#[cfg(test)]
pub(super) fn endpoint_permissions_are_private_for_test(
    endpoint: &str,
) -> io::Result<Option<bool>> {
    discovery_permissions_are_private_for_test(Path::new(endpoint))
}

