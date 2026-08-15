//! 可由独立 [`super::Platform`] 同步完成的平台服务值。

// 引入公开的类型化错误，用于在进入平台 Adapter 前验证应用身份。
use crate::core::{Errc, Error, Result};

/// 操作系统已知的用户目录。
///
/// 本类型是 SpecialDir 的公开权威定义。native 内部文件系统契约
/// `crate::native::capabilities::system::SpecialDir`（私有实现）以本类型为共享语义来源，
/// 并通过 `From` 转换覆盖全部变体，另扩展 Temp/Current/Executable 内部变体；
/// 增删本枚举变体时必须同步 native 侧定义与一致性测试。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialDir {
    /// 当前用户的主目录或配置文件目录。
    Home,
    /// 平台约定的应用配置数据目录。
    AppData,
    /// 平台约定的设备本地应用数据目录。
    LocalAppData,
    /// 当前用户的文档目录。
    Documents,
    /// 当前用户的桌面目录。
    Desktop,
    /// 当前用户的下载目录。
    Downloads,
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
