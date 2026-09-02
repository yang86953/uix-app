// 框架级错误码枚举

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
/// UIX 框架跨模块传递的稳定错误码。
pub enum Errc {
    /// 表示操作成功，没有错误。
    None = 0,
    /// 表示无法归类的未知错误。
    Unknown = 1,
    /// 表示调用参数无效。
    InvalidArgument = 2,
    /// 表示数值或索引超出允许范围。
    OutOfRange = 3,
    /// 表示目标资源不存在。
    NotFound = 4,
    /// 表示目标资源已经存在。
    AlreadyExists = 5,
    /// 表示调用方缺少所需权限。
    PermissionDenied = 6,
    /// 表示操作在期限内未完成。
    Timeout = 7,
    /// 表示操作已被取消。
    Cancelled = 8,
    /// 表示所请求的功能尚未实现。
    NotImplemented = 9,
    /// 表示当前状态不允许执行该操作。
    InvalidOperation = 10,
    /// 表示系统资源不足以完成操作。
    InsufficientResources = 11,
    /// 表示操作需要等待后再重试。
    WouldBlock = 13,

    /// 表示通用输入输出错误。
    IoError = 100,
    /// 表示目标文件不存在。
    FileNotFound = 101,
    /// 表示文件访问被拒绝。
    AccessDenied = 102,
    /// 表示写入操作失败。
    WriteFailure = 104,

    /// 表示远端拒绝连接。
    ConnectionRefused = 201,
    /// 表示连接被对端重置。
    ConnectionReset = 202,

    /// 表示协议状态无效。
    InvalidState = 301,
    /// 表示数据格式无效。
    FormatError = 302,
    /// 表示数据解析失败。
    ParseError = 303,
    /// 表示数据序列化或反序列化失败。
    SerializationError = 304,

    /// 表示异步任务已被放弃。
    TaskAbandoned = 401,

    /// 表示通用平台错误。
    PlatformError = 500,
    /// 表示原生窗口创建失败。
    WindowCreationFailed = 501,
    /// 表示原生窗口类注册失败。
    ClassRegistrationFailed = 502,
    /// 表示图形呈现表面已失效。
    GraphicsSurfaceLost = 504,
    /// 表示图形设备已失效。
    GraphicsDeviceLost = 505,
    /// 表示图形设备内存不足。
    GraphicsOutOfMemory = 506,
    /// 表示图形输出当前被遮挡。
    GraphicsOccluded = 507,
    /// 表示 Surface 已受控重建，未提交帧应在新代际重试。
    GraphicsSurfaceChanged = 508,

    /// 应用自定义错误码的起始值。
    AppDomainBase = 1000,
}

impl Errc {
    /// 返回错误码所属的稳定类别名称。
    pub fn category(self) -> &'static str {
        let v = self as u32;
        if v < 100 {
            "general"
        } else if v < 200 {
            "io"
        } else if v < 300 {
            "network"
        } else if v < 400 {
            "protocol"
        } else if v < 500 {
            "concurrency"
        } else if v < 1000 {
            "platform"
        } else {
            "application"
        }
    }

    /// 尝试将 Errc 映射为 std::io::ErrorKind。
    pub fn to_io_kind(self) -> Option<std::io::ErrorKind> {
        match self {
            Self::InvalidArgument | Self::OutOfRange => Some(std::io::ErrorKind::InvalidInput),
            Self::NotFound | Self::FileNotFound => Some(std::io::ErrorKind::NotFound),
            Self::PermissionDenied | Self::AccessDenied => {
                Some(std::io::ErrorKind::PermissionDenied)
            }
            Self::Cancelled => Some(std::io::ErrorKind::Interrupted),
            Self::Timeout => Some(std::io::ErrorKind::TimedOut),
            Self::ConnectionRefused => Some(std::io::ErrorKind::ConnectionRefused),
            Self::ConnectionReset => Some(std::io::ErrorKind::ConnectionReset),
            Self::AlreadyExists => Some(std::io::ErrorKind::AlreadyExists),
            Self::FormatError | Self::ParseError | Self::SerializationError => {
                Some(std::io::ErrorKind::InvalidData)
            }
            Self::WriteFailure => Some(std::io::ErrorKind::WriteZero),
            Self::WouldBlock => Some(std::io::ErrorKind::WouldBlock),
            _ => None,
        }
    }
}

impl fmt::Display for Errc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Errc::None => "none",
            Errc::Unknown => "unknown",
            Errc::InvalidArgument => "invalid_argument",
            Errc::OutOfRange => "out_of_range",
            Errc::NotFound => "not_found",
            Errc::AlreadyExists => "already_exists",
            Errc::PermissionDenied => "permission_denied",
            Errc::Timeout => "timeout",
            Errc::Cancelled => "cancelled",
            Errc::NotImplemented => "not_implemented",
            Errc::InvalidOperation => "invalid_operation",
            Errc::InsufficientResources => "insufficient_resources",
            Errc::WouldBlock => "would_block",
            Errc::IoError => "io_error",
            Errc::FileNotFound => "file_not_found",
            Errc::AccessDenied => "access_denied",
            Errc::WriteFailure => "write_failure",
            Errc::ConnectionRefused => "connection_refused",
            Errc::ConnectionReset => "connection_reset",
            Errc::InvalidState => "invalid_state",
            Errc::FormatError => "format_error",
            Errc::ParseError => "parse_error",
            Errc::SerializationError => "serialization_error",
            Errc::TaskAbandoned => "task_abandoned",
            Errc::PlatformError => "platform_error",
            Errc::WindowCreationFailed => "window_creation_failed",
            Errc::ClassRegistrationFailed => "class_registration_failed",
            Errc::GraphicsSurfaceLost => "graphics_surface_lost",
            Errc::GraphicsDeviceLost => "graphics_device_lost",
            Errc::GraphicsOutOfMemory => "graphics_out_of_memory",
            Errc::GraphicsOccluded => "graphics_occluded",
            Errc::GraphicsSurfaceChanged => "graphics_surface_changed",
            Errc::AppDomainBase => "app_domain_base",
        };
        write!(f, "{}", name)
    }
}
