//! Platform System 的中立文件系统目录值。

/// 操作系统或当前进程拥有的特殊目录。
///
/// 本枚举服务公开 Platform facade；所有变体都表示可直接访问的目录位置，
/// 不承载目录创建或生命周期策略。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialDir {
    /// 当前用户的主目录或配置文件目录。
    Home,
    /// 操作系统提供的临时文件目录。
    Temp,
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
    /// 当前进程的工作目录。
    Current,
    /// 当前可执行文件所在目录。
    Executable,
}
