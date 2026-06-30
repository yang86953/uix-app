//! Frame Graph — 资源中心化与版本追踪子系统。
//!
//! 将纹理（Texture）和 Buffer 抽象为逻辑资源，通过单调递增的版本号
//! 追踪每次写入，为 Pass 裁剪提供版本比对依据。

// ════════════════════════════════════════════════════════════════════════════
// 标识符与版本
// ════════════════════════════════════════════════════════════════════════════

/// 帧图内逻辑资源的唯一标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(pub u32);

/// Pass 节点的唯一标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassId(pub u32);

/// 资源版本号 —— 每次写入后单调递增，用于变更检测。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u64);

impl Version {
    pub const fn initial() -> Self {
        Self(0)
    }

    pub fn advance(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 资源类别
// ════════════════════════════════════════════════════════════════════════════

/// 帧图可追踪的资源类别。
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceKind {
    /// 像素纹理（Offscreen 帧缓冲）。
    Texture { width: u32, height: u32 },
    /// 原始数据缓冲。
    Buffer { byte_size: u64 },
}

impl ResourceKind {
    pub fn texture(width: u32, height: u32) -> Self {
        Self::Texture { width, height }
    }

    pub fn buffer(byte_size: u64) -> Self {
        Self::Buffer { byte_size }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 资源条目（模块内部）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub(crate) struct ResourceEntry {
    pub(crate) id: ResourceId,
    pub(crate) name: String,
    #[allow(dead_code)]
    pub(crate) kind: ResourceKind,
    pub(crate) version: Version,
    pub(crate) last_writer: Option<PassId>,
}

// ════════════════════════════════════════════════════════════════════════════
// ResourceRegistry —— 资源注册表
// ════════════════════════════════════════════════════════════════════════════

/// 帧图全局资源注册表。
///
/// 管理所有逻辑资源的生命周期与版本号。Pass 通过 `ResourceId` 引用资源，
/// 每次写入后 `bump_version` 使版本递增。
#[derive(Debug)]
pub struct ResourceRegistry {
    entries: Vec<ResourceEntry>,
    next_res_id: u32,
    next_pass_id: u32,
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_res_id: 0,
            next_pass_id: 0,
        }
    }

    // ── 资源管理 ──

    /// 注册一个新资源。返回唯一 `ResourceId`。
    pub fn register(&mut self, name: &str, kind: ResourceKind) -> ResourceId {
        let id = ResourceId(self.next_res_id);
        self.next_res_id += 1;
        self.entries.push(ResourceEntry {
            id,
            name: name.to_string(),
            kind,
            version: Version::initial(),
            last_writer: None,
        });
        id
    }

    /// 分配一个 PassId（供 PassBuilder 使用）。
    pub fn allocate_pass_id(&mut self) -> PassId {
        let id = PassId(self.next_pass_id);
        self.next_pass_id += 1;
        id
    }

    // ── 版本查询 ──

    pub fn version_of(&self, id: ResourceId) -> Version {
        self.entry(id)
            .map(|e| e.version)
            .unwrap_or(Version::initial())
    }

    pub fn contains(&self, id: ResourceId) -> bool {
        self.entry(id).is_some()
    }

    pub fn resource_name(&self, id: ResourceId) -> &str {
        self.entry(id)
            .map(|e| e.name.as_str())
            .unwrap_or("<unknown>")
    }

    pub fn last_writer_of(&self, id: ResourceId) -> Option<PassId> {
        self.entry(id).and_then(|e| e.last_writer)
    }

    // ── 版本变更 ──

    /// 标记资源被写入：版本递增 + 记录写入者。
    pub fn bump_version(&mut self, id: ResourceId, pass: PassId) -> Version {
        if let Some(entry) = self.entry_mut(id) {
            entry.version = entry.version.advance();
            entry.last_writer = Some(pass);
            entry.version
        } else {
            Version::initial()
        }
    }

    // ── 内部辅助 ──

    fn entry(&self, id: ResourceId) -> Option<&ResourceEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn entry_mut(&mut self, id: ResourceId) -> Option<&mut ResourceEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }
}
