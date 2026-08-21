//! Agent 本机传输使用的 Unix 用户身份判定。

/// 只保存 OS 提供的有效用户 ID；传输层不解释用户数据库或用户名。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UnixUserId(u32);

impl UnixUserId {
    pub(super) const fn from_raw(value: u32) -> Self {
        Self(value)
    }

    /// 只有端点进程与已连接 peer 的有效 uid 完全相同才准入。
    pub(super) const fn admits(self, peer: Self) -> bool {
        self.0 == peer.0
    }
}

// 编译期锁定端点用户与同 uid peer 必须准入，避免平台分支漂移成全拒绝。
const _: () = assert!(UnixUserId::from_raw(1_000).admits(UnixUserId::from_raw(1_000)));
// 编译期锁定异 uid peer 必须拒绝，避免 Linux 或 macOS 凭据适配回退为全允许。
const _: () = assert!(!UnixUserId::from_raw(1_000).admits(UnixUserId::from_raw(1_001)));
