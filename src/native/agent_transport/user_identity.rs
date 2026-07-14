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
