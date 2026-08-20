// 引入稳定文件身份生成器所需的原子计数器。
use std::sync::atomic::{AtomicU64, Ordering};

// 为本进程生成的上传文件身份提供单调序列。
static NEXT_UPLOAD_FILE_ID: AtomicU64 = AtomicU64::new(1);

/// 非空的稳定上传文件身份。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UploadFileId(String);

// 实现稳定上传文件身份的受控构造入口。
impl UploadFileId {
    /// 从调用方提供的非空文本构造稳定身份。
    pub fn new(value: impl Into<String>) -> Result<Self, UploadFileIdError> {
        // 取得调用方提供的拥有型身份文本。
        let value = value.into();
        // 空白身份不能承担队列唯一性。
        if value.trim().is_empty() {
            // 返回类型化的身份格式错误。
            return Err(UploadFileIdError);
        }
        // 保留原始非空文本作为稳定身份。
        Ok(Self(value))
    }

    /// 生成当前进程内单调且非空的稳定身份。
    pub fn generated() -> Self {
        // 原子取得不会与本生成器先前结果重复的序号。
        let sequence = NEXT_UPLOAD_FILE_ID.fetch_add(1, Ordering::Relaxed);
        // 固定前缀使身份便于诊断且始终非空。
        Self(format!("upload-{sequence}"))
    }

    /// 借用稳定身份文本。
    pub fn as_str(&self) -> &str {
        // 不暴露内部字符串的可变所有权。
        &self.0
    }
}

// 提供稳定身份的展示实现。
impl std::fmt::Display for UploadFileId {
    // 把身份按原始文本写入格式化器。
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 委托内部字符串的展示实现。
        formatter.write_str(&self.0)
    }
}

/// 上传文件身份为空时返回的格式错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadFileIdError;

// 提供上传文件身份错误的可读说明。
impl std::fmt::Display for UploadFileIdError {
    // 写出稳定且可测试的错误说明。
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 明确指出非空约束。
        formatter.write_str("UploadFileId 不能为空")
    }
}

// 将上传文件身份错误接入标准错误链。
impl std::error::Error for UploadFileIdError {}

/// 上传文件项。
#[derive(Debug, Clone, PartialEq)]
pub struct UploadFile {
    /// 队列内用于更新与移除的稳定身份。
    pub id: UploadFileId,
    /// 仅用于展示的文件名。
    pub name: String,
    /// 真实本地文件的原始路径；合成队列项保持为空。
    pub source_path: Option<String>,
    /// 文件字节大小。
    pub size: u64,
    /// 应用服务写回的归一化进度。
    pub progress: f32,
    /// 应用服务写回的上传状态。
    pub status: UploadStatus,
}

// 提供不会遗漏稳定身份的上传文件构造入口。
impl UploadFile {
    /// 使用自动生成的稳定身份构造文件项。
    pub fn new(name: impl Into<String>, size: u64) -> Self {
        // 委托显式身份构造器建立完整默认状态。
        Self::with_id(UploadFileId::generated(), name, size)
    }

    /// 使用调用方提供的稳定身份构造文件项。
    pub fn with_id(id: UploadFileId, name: impl Into<String>, size: u64) -> Self {
        // 返回等待应用服务处理的文件项。
        Self {
            // 保存非空稳定身份。
            id,
            // 保存展示名称。
            name: name.into(),
            // 外部构造项默认不声明本地路径。
            source_path: None,
            // 保存已知字节大小。
            size,
            // 新项从零进度开始。
            progress: 0.0,
            // 新项进入待处理状态。
            status: UploadStatus::Pending,
        }
    }

    /// 附加真实本地文件来源路径。
    pub fn source_path(mut self, source_path: impl Into<String>) -> Self {
        // 保存应用可读取的原始路径。
        self.source_path = Some(source_path.into());
        // 返回完成配置的文件项。
        self
    }
}

/// 应用服务拥有的上传处理状态。
///
/// 与 ValidateStatus/InputStatus/StepStatus/BadgeStatus 共享「组件状态」命名模式，
/// 但各自语义与变体独立（本枚举是应用上传传输状态），勿强行合并。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UploadStatus {
    /// 等待应用决定是否启动。
    Pending,
    /// 应用已开始传输。
    Uploading,
    /// 应用已报告成功。
    Done,
    /// 应用已报告失败。
    Error,
}

/// 上传队列发生的不可变类型化变化事实。
#[derive(Debug, Clone, PartialEq)]
pub enum UploadChange {
    /// 一批文件已原子加入队列。
    Added {
        /// 本批新增文件的稳定身份。
        ids: Vec<UploadFileId>,
        /// 更新完成后的完整队列快照。
        files: Vec<UploadFile>,
    },
    /// 一批文件已原子移出队列。
    Removed {
        /// 本批移除文件的稳定身份。
        ids: Vec<UploadFileId>,
        /// 更新完成后的完整队列快照。
        files: Vec<UploadFile>,
    },
    /// 队列已原子清空。
    Cleared {
        /// 更新完成后的空队列快照。
        files: Vec<UploadFile>,
    },
}

// 提供变化事实的稳定快照读取入口。
impl UploadChange {
    /// 借用变化完成后的完整队列。
    pub fn files(&self) -> &[UploadFile] {
        // 所有变化种类都携带同一语义的更新后快照。
        match self {
            // 新增事实返回新增后的快照。
            Self::Added { files, .. }
            // 移除事实返回移除后的快照。
            | Self::Removed { files, .. }
            // 清空事实返回空快照。
            | Self::Cleared { files } => files,
        }
    }
}

/// 动态 accept 配置不符合首版扩展名语法时返回的格式错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadAcceptError {
    // 保存被拒绝的完整模式，供调用方诊断。
    pub(crate) pattern: String,
}

// 提供 accept 格式错误的公开读取入口。
impl UploadAcceptError {
    /// 借用被拒绝的模式文本。
    pub fn pattern(&self) -> &str {
        // 返回只读模式借用。
        &self.pattern
    }
}

// 提供 accept 格式错误的可读说明。
impl std::fmt::Display for UploadAcceptError {
    // 写出首版支持范围与实际模式。
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // MIME 与不带点扩展名均不得静默降级为不匹配。
        write!(
            formatter,
            "Upload accept 仅支持空、*、*/*、.ext、*.ext 及其 ASCII 逗号列表：{}",
            self.pattern
        )
    }
}

// 将 accept 格式错误接入标准错误链。
impl std::error::Error for UploadAcceptError {}

/// 用户候选文件未进入队列的确定原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadRejectReason {
    /// 文件扩展名不符合 accept。
    ExtensionMismatch,
    /// 路径不存在、不是普通文件或元数据不可读。
    UnreadableFile,
    /// 文件超过单文件大小限制。
    TooLarge,
    /// 队列已达到用户新增上限。
    MaxCount,
}

/// 单个用户候选的类型化拒绝结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadRejection {
    /// 被拒绝的原始路径。
    pub path: String,
    /// 确定的拒绝原因。
    pub reason: UploadRejectReason,
}

/// 应用按稳定身份更新文件状态时的类型化错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadUpdateError {
    /// 目标身份不存在于当前队列。
    MissingFile(UploadFileId),
    /// 进度不是有限数值。
    NonFiniteProgress,
}

/// 一批用户候选的入队结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadQueueResult {
    /// 成功加入的稳定身份。
    pub added_ids: Vec<UploadFileId>,
    /// 未修改队列的被拒绝候选。
    pub rejected: Vec<UploadRejection>,
}
