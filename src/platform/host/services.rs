//! 可由独立 [`crate::platform::Platform`] 同步完成的平台服务值。

// 引入集合以按首次声明顺序去除重复扩展名。
use std::collections::HashSet;

// 引入公开的类型化错误，用于在进入平台 Adapter 前验证应用身份。
use crate::core::{Errc, Error, ErrorSeverity, Result};

use super::capabilities::StatusLevel;

// 保持公开 services 路径，同时复用 Platform System 的唯一文件系统目录类型。
pub use crate::platform::system::filesystem::SpecialDir;

/// Drawing 字体发现所需的最小平台协议。
///
/// OS 实现可以同时提供更多系统信息，但 Drawing 只能依赖这四项字体能力。
pub trait FontSystemInfo {
    /// 返回按优先级排列的默认字体路径。
    fn default_font_paths(&self) -> Result<Vec<String>>;

    /// 返回按优先级排列的 CJK 回退字体路径。
    fn probe_cjk_font_paths(&self) -> Vec<String>;

    /// 按字体族名查找可加载路径。
    fn probe_family_font_path(&self, family: &str) -> Option<String>;

    /// 执行受限的最后回退扫描。
    fn scan_fallback_font_path(&self) -> Option<String>;
}

/// UI 消费的中立 Toast 值，不持有系统通知 Provider。
#[derive(Debug, Clone)]
pub struct ToastEntry {
    pub id: u64,
    pub title: String,
    pub message: String,
    pub level: StatusLevel,
    /// 持续时间（毫秒）；零表示手动关闭。
    pub duration_ms: u32,
    /// 是否仍应显示。
    pub visible: bool,
    /// 条目创建时刻。
    pub created_at: std::time::Instant,
}

impl ToastEntry {
    /// 将非致命框架错误投影为中立 Toast 值。
    pub fn from_error(id: u64, error: &Error, created_at: std::time::Instant) -> Option<Self> {
        if error.severity().is_fatal() {
            return None;
        }
        let (title, level, duration_ms) = match error.severity() {
            ErrorSeverity::Info => ("Info", StatusLevel::Info, 4_000),
            ErrorSeverity::Warning => ("Warning", StatusLevel::Warning, 5_000),
            ErrorSeverity::Error => ("Error", StatusLevel::Error, 6_000),
            ErrorSeverity::Fatal => return None,
        };
        let mut message = error.message().to_owned();
        if let Some(source) = error.source_error() {
            message.push_str(": ");
            message.push_str(source.message());
        }
        Some(Self {
            id,
            title: title.to_owned(),
            message,
            level,
            duration_ms,
            visible: true,
            created_at,
        })
    }
}

/// 文件对话框中的一个命名过滤器。
///
/// 扩展名会去除可选的 `.` 或 `*.` 前缀、统一为 ASCII 小写并稳定去重。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileDialogFilter {
    // 保存面向用户的过滤器名称。
    name: String,
    // 保存不含点号与通配符的规范扩展名。
    extensions: Box<[String]>,
}

impl FileDialogFilter {
    /// 创建一个拥有名称与扩展名列表的文件过滤器。
    pub fn new<I, S>(name: impl Into<String>, extensions: I) -> Result<Self>
    where
        // 调用方可以传入数组、切片或其他扩展名迭代器。
        I: IntoIterator<Item = S>,
        // 每个扩展名在构造时转换为 owned 文本。
        S: Into<String>,
    {
        // 取得并清理面向用户的过滤器名称。
        let name = name.into().trim().to_owned();
        // 名称不能破坏各平台过滤器描述格式。
        if name.is_empty()
            // NUL 会截断 Win32 过滤器字符串。
            || name.contains('\0')
            // 分号是 UIX 可移植过滤器之间的分隔符。
            || name.contains(';')
            // 竖线在部分原生对话框中承担描述分隔职责。
            || name.contains('|')
            // 控制字符不能进入原生对话框标题。
            || name.chars().any(char::is_control)
        {
            // 在进入 OS Adapter 前返回稳定参数错误。
            return Err(Error::new(
                // 非法过滤器名称属于调用方输入错误。
                Errc::InvalidArgument,
                // 诊断列出跨平台可移植约束。
                "FileDialogFilter::new: name must be non-empty and contain no control, NUL, ';', or '|' characters",
            ));
        }
        // 记录大小写折叠后的扩展名以稳定去重。
        let mut seen = HashSet::new();
        // 保存调用方首次声明的扩展名顺序。
        let mut normalized = Vec::new();
        // 逐项验证并规范化扩展名。
        for extension in extensions {
            // 取得 owned 输入并去除外围空白。
            let extension = extension.into();
            // 先兼容常见的星号点号前缀。
            let extension = extension
                .trim()
                .strip_prefix("*.")
                .unwrap_or(extension.trim());
            // 再兼容单点号前缀。
            let extension = extension.strip_prefix('.').unwrap_or(extension);
            // 扩展名必须是可移植的裸文件类型，而不是路径或全通配符。
            if extension.is_empty()
                // 单星号和 `*.*` 规范化后的星号都不能表达确定类型。
                || extension == "*"
                // 路径分隔符会把过滤器变成意外的路径模式。
                || extension.contains('/')
                // Windows 路径分隔符同样不允许。
                || extension.contains('\\')
                // NUL 会截断 Win32 过滤器。
                || extension.contains('\0')
                // 只接受各 Provider 都能稳定解释的 ASCII 类型字符。
                || !extension.chars().all(|character| {
                    // 支持普通扩展名、复合扩展名、横线与下划线。
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
            {
                // 拒绝模糊或平台相关的过滤器输入。
                return Err(Error::new(
                    // 非法扩展名属于调用方参数错误。
                    Errc::InvalidArgument,
                    // 诊断明确要求传入裸扩展名。
                    "FileDialogFilter::new: extensions must be concrete ASCII file types without paths, wildcards, or NUL",
                ));
            }
            // 统一大小写以保证三平台语义一致。
            let extension = extension.to_ascii_lowercase();
            // 只保存首次出现的规范扩展名。
            if seen.insert(extension.clone()) {
                // 稳定保留调用方声明顺序。
                normalized.push(extension);
            }
        }
        // 空过滤器会在不同 Provider 中产生不一致语义。
        if normalized.is_empty() {
            // 要求调用方显式提供至少一个确定扩展名。
            return Err(Error::new(
                // 缺失扩展名属于调用方参数错误。
                Errc::InvalidArgument,
                // 诊断提示无过滤需求应直接传空过滤器切片。
                "FileDialogFilter::new: at least one extension is required; pass no filters for an unrestricted dialog",
            ));
        }
        // 保存完整验证后的 owned 值。
        Ok(Self {
            // 名称生命周期不依赖调用方。
            name,
            // 扩展名集合收窄为不可增删的 owned slice。
            extensions: normalized.into_boxed_slice(),
        })
    }

    /// 返回面向用户的过滤器名称。
    pub fn name(&self) -> &str {
        // 只暴露不可变借用。
        &self.name
    }

    /// 返回不含点号与通配符的规范扩展名。
    pub fn extensions(&self) -> &[String] {
        // 只暴露不可变切片，维持构造时不变量。
        &self.extensions
    }
}

/// Windows 应用用户模型标识（AUMID）。
///
/// 该值由应用或部署层提供；UIX 不会创建快捷方式、写注册表或推断身份。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppUserModelId(String);

impl AppUserModelId {
    /// 构造一个满足 Windows 显式 AUMID 基础约束的值。
    pub fn new(value: impl Into<String>) -> Result<Self> {
        // 先取得 owned 文本，确保配置生命周期不依赖调用方。
        let value = value.into();
        // 按 Windows UTF-16 字符单元限制计算长度。
        let utf16_len = value.encode_utf16().count();
        // 拒绝空值、空白、NUL 和超过 Windows 上限的标识。
        if value.is_empty()
            || value.chars().any(char::is_whitespace)
            || value.contains('\0')
            || utf16_len > 128
        {
            // 返回稳定的参数错误，避免把非法身份交给系统 API。
            return Err(Error::new(
                // 非法 AUMID 属于调用方参数错误。
                Errc::InvalidArgument,
                // 诊断同时给出 Windows 的核心格式约束。
                "AppUserModelId::new: value must be 1..=128 UTF-16 code units without whitespace or NUL",
            ));
        }
        // 保存已经验证的应用身份。
        Ok(Self(value))
    }

    /// 返回原始 AUMID 文本。
    pub fn as_str(&self) -> &str {
        // 只暴露不可变借用，身份仍由值对象拥有。
        &self.0
    }
}

/// 当前环境的系统通知可用状态。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemNotificationCapability {
    /// Provider 已就绪，可以尝试发送通知。
    Available,
    /// 当前平台要求调用方先配置应用身份。
    IdentityRequired,
    /// 已配置身份，但部署层尚未完成系统登记。
    IdentityUnregistered,
    /// 当前平台没有系统通知 Provider。
    Unsupported,
}

/// 一条系统通知。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemNotification {
    title: String,
    message: String,
}

impl SystemNotification {
    /// 使用标题和正文创建一条 owned 系统通知。
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }

    /// 返回通知标题。
    pub fn title(&self) -> &str {
        &self.title
    }

    /// 返回通知正文。
    pub fn message(&self) -> &str {
        &self.message
    }
}
