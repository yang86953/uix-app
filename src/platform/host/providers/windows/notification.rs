//! Windows 桌面系统通知 Adapter。

// 引入 Windows 路径的 UTF-16 编码能力。
use std::os::windows::ffi::OsStrExt;
// 引入开始菜单扫描所需的路径类型。
use std::path::{Path, PathBuf};

// 引入 UIX 类型化错误与结果。
use crate::core::{Errc, Error, Result};
// 引入通知身份、能力状态和值对象。
use crate::platform::services::{AppUserModelId, SystemNotification, SystemNotificationCapability};
// 引入 XML DOM 以构造 ToastGeneric 载荷。
use windows::Data::Xml::Dom::XmlDocument;
// 引入 Property System 键类型。
use windows::Win32::Foundation::PROPERTYKEY;
// 引入 COM 对象创建、内存释放和快捷方式持久化接口。
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, IPersistFile, STGM_READ,
};
// 引入 PROPVARIANT 字符串转换；值自身负责生命周期清理。
use windows::Win32::System::Com::StructuredStorage::PropVariantToString;
// 引入开始菜单 known folder 与 ShellLink 接口。
use windows::Win32::UI::Shell::{
    FOLDERID_CommonPrograms, FOLDERID_Programs, IShellLinkW, KNOWN_FOLDER_FLAG,
    SHGetKnownFolderPath, ShellLink,
};
// 引入快捷方式属性存储接口。
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
// 引入 Windows Toast 的同步提交入口。
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
// 引入 Windows 字符串、GUID、COM cast 与 UTF-16 指针类型。
use windows::core::{GUID, HSTRING, Interface, PCWSTR, PWSTR};

// System.AppUserModel.ID 的 Windows Property System 键。
const PKEY_APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
    // 该 fmtid 由 Windows Shell 的 AppUserModel 属性集定义。
    fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    // pid 5 对应 System.AppUserModel.ID。
    pid: 5,
};

// 查询 Windows 通知身份是否已经由部署层登记。
pub(crate) fn system_notification_capability(
    // 身份由 Platform 拥有，此处只在同步探测期间借用。
    app_user_model_id: Option<&AppUserModelId>,
) -> Result<SystemNotificationCapability> {
    // 没有显式身份时立即报告配置要求，不扫描系统状态。
    let Some(app_user_model_id) = app_user_model_id else {
        // 未配置身份是正常、可解释的能力状态。
        return Ok(SystemNotificationCapability::IdentityRequired);
    };
    // 只读核验当前用户和所有用户的开始菜单快捷方式。
    if notification_identity_is_registered(app_user_model_id.as_str())? {
        // 只有存在完全匹配的快捷方式登记才宣告可用。
        Ok(SystemNotificationCapability::Available)
    } else {
        // 配置存在但系统登记缺失时禁止伪成功。
        Ok(SystemNotificationCapability::IdentityUnregistered)
    }
}

// 使用已核验的 AUMID 同步提交一条 Windows Toast。
pub(crate) fn show_notification(
    // Windows Available 状态保证该身份存在。
    app_user_model_id: Option<&AppUserModelId>,
    // 通知文本由公开门面预先完成基础校验。
    notification: &SystemNotification,
) -> Result<()> {
    // 防御 Provider 被绕过时的缺失身份。
    let app_user_model_id = app_user_model_id.ok_or_else(|| {
        // Provider 级错误仍保持产品约定的类型。
        Error::new(
            // 未配置身份时无法调用 Windows 通知 API。
            Errc::NotImplemented,
            // 诊断指向公开配置入口。
            "Platform::show_notification: configure an AppUserModelId before sending",
        )
    })?;
    // 构造安全转义的 ToastGeneric XML 文档。
    let xml = format!(
        // 两个 text 节点分别承载标题和正文。
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        // 转义标题，禁止调用方文本改变 XML 结构。
        escape_notification_xml(notification.title()),
        // 转义正文，保留原始可见字符。
        escape_notification_xml(notification.message()),
    );
    // 激活 Windows XML DOM 文档。
    let document = XmlDocument::new()
        // 把 WinRT 激活失败映射为稳定的 UIX 错误。
        .map_err(|source| {
            notification_windows_error("show_notification", "XmlDocument::new", source)
        })?;
    // 载入已经转义的通知载荷。
    document
        // Windows XML DOM 接受 HSTRING 载荷。
        .LoadXml(&HSTRING::from(xml))
        // XML 载入失败说明平台拒绝了载荷。
        .map_err(|source| {
            notification_windows_error("show_notification", "XmlDocument::LoadXml", source)
        })?;
    // 从 XML DOM 创建系统 Toast 值。
    let toast = ToastNotification::CreateToastNotification(&document)
        // 保留创建阶段名称，便于定位 WinRT 故障。
        .map_err(|source| {
            notification_windows_error(
                "show_notification",
                "ToastNotification::CreateToastNotification",
                source,
            )
        })?;
    // 使用部署层登记的明确 AUMID 创建 notifier。
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
        // 直接复用验证后的身份文本。
        app_user_model_id.as_str(),
    ))
    // notifier 创建失败通常表示身份或系统通知服务异常。
    .map_err(|source| {
        notification_windows_error(
            "show_notification",
            "ToastNotificationManager::CreateToastNotifierWithId",
            source,
        )
    })?;
    // 同步把通知提交给 Windows 通知平台。
    notifier
        // Show 返回后只保证系统接受提交，不伪造后续用户交互。
        .Show(&toast)
        // 把系统提交失败映射为类型化平台错误。
        .map_err(|source| {
            notification_windows_error("show_notification", "ToastNotifier::Show", source)
        })
}

// 只读搜索开始菜单中的 AUMID 快捷方式登记。
fn notification_identity_is_registered(app_user_model_id: &str) -> Result<bool> {
    // 当前用户和所有用户的 Programs 目录共同构成桌面应用登记范围。
    let folders = [
        // 当前用户开始菜单 Programs 目录。
        (&FOLDERID_Programs, "FOLDERID_Programs"),
        // 全局开始菜单 Programs 目录。
        (&FOLDERID_CommonPrograms, "FOLDERID_CommonPrograms"),
    ];
    // 记录至少一个可读取根目录，避免把探测失败误报为未登记。
    let mut readable_root = false;
    // 保存最后一个根目录错误，在两个目录都不可读时返回。
    let mut last_error = None;
    // 依次探测当前用户和所有用户目录。
    for (folder, name) in folders {
        // 解析 known-folder 路径，不创建或修改目录。
        let root = match notification_programs_path(folder, name) {
            // 成功解析时继续扫描。
            Ok(root) => root,
            // 单个 known folder 不可用时仍尝试另一个范围。
            Err(error) => {
                // 保存精确失败，供完全不可探测时返回。
                last_error = Some(error);
                // 继续下一个登记范围。
                continue;
            }
        };
        // 扫描该根目录下所有 .lnk 文件。
        match shortcut_tree_contains_aumid(&root, app_user_model_id) {
            // 找到完全匹配的登记即可提前成功。
            Ok(true) => return Ok(true),
            // 根目录可读但没有匹配项，继续另一个范围。
            Ok(false) => readable_root = true,
            // 单个根目录不可读时保留错误并继续。
            Err(error) => last_error = Some(error),
        }
    }
    // 至少扫描过一个范围时，未命中就是明确的未登记。
    if readable_root {
        // 返回 false 让公开 capability 给出 IdentityUnregistered。
        return Ok(false);
    }
    // 两个范围都不可探测时返回最后一个平台错误。
    Err(last_error.unwrap_or_else(|| {
        // 理论上的空目录表仍返回稳定错误。
        Error::new(
            // 系统登记范围不可读取属于平台故障。
            Errc::PlatformError,
            // 诊断说明失败发生在只读登记探测阶段。
            "Platform::system_notification_capability: no readable Start Menu registration roots",
        )
    }))
}

// 解析通知登记使用的 Programs known folder。
fn notification_programs_path(folder: &GUID, name: &str) -> Result<PathBuf> {
    // SAFETY: folder 指向静态 GUID；返回的 Shell 内存由 guard 唯一释放。
    let pointer = unsafe {
        // 零 flags 与空 token 只查询当前上下文中的现存目录。
        SHGetKnownFolderPath(folder, KNOWN_FOLDER_FLAG(0), None)
    }
    // known-folder API 失败映射为 capability 探测错误。
    .map_err(|source| notification_windows_error("system_notification_capability", name, source))?;
    // 无论 UTF-16 转换成功与否都释放 Shell 分配内存。
    let pointer = CoTaskMemWide(pointer);
    // SAFETY: SHGetKnownFolderPath 返回有效的 NUL 终止 UTF-16 字符串。
    let path = unsafe { pointer.0.to_string() }
        // 非法 UTF-16 说明系统路径违反契约。
        .map_err(|source| {
            // 返回稳定平台错误并保留 known-folder 名称。
            Error::new(
                // 系统返回非法路径属于平台错误。
                Errc::PlatformError,
                // 诊断包含 UTF-16 转换失败。
                format!("Platform::system_notification_capability: {name} returned invalid UTF-16: {source}"),
            )
        })?;
    // 返回 owned 路径供只读目录扫描。
    Ok(PathBuf::from(path))
}

// 递归扫描单个开始菜单目录中的快捷方式。
fn shortcut_tree_contains_aumid(root: &Path, app_user_model_id: &str) -> Result<bool> {
    // 使用显式栈避免递归调用深度依赖目录结构。
    let mut pending = vec![root.to_path_buf()];
    // 逐目录消费待扫描列表。
    while let Some(directory) = pending.pop() {
        // 根目录读取失败必须报告，子目录瞬时失败则跳过。
        let entries = match std::fs::read_dir(&directory) {
            // 保留成功的惰性目录迭代器。
            Ok(entries) => entries,
            // 根目录不可读意味着此登记范围不可探测。
            Err(source) if directory == root => {
                // 映射权限与一般 IO 错误。
                return Err(notification_io_error("read Start Menu root", source));
            }
            // 某个子目录不可读不应遮蔽其余快捷方式。
            Err(_) => continue,
        };
        // 检查当前目录的每个可读取条目。
        for entry in entries.flatten() {
            // 读取类型失败时忽略该单项。
            let Ok(file_type) = entry.file_type() else {
                // 继续扫描其他登记项。
                continue;
            };
            // 非符号链接目录加入待扫描栈。
            if file_type.is_dir() {
                // 保存 owned 路径供后续读取。
                pending.push(entry.path());
                // 当前条目已经处理完成。
                continue;
            }
            // 仅解析 .lnk 文件，其他开始菜单资源与登记无关。
            let path = entry.path();
            // 扩展名匹配使用 Windows 大小写不敏感语义。
            let is_shortcut = path
                // 取得可选扩展名。
                .extension()
                // 转换为可比较文本。
                .and_then(|extension| extension.to_str())
                // 比较标准快捷方式扩展名。
                .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"));
            // 跳过非快捷方式文件。
            if !is_shortcut {
                // 继续扫描同目录其余条目。
                continue;
            }
            // 读取快捷方式属性并精确比较 AUMID。
            if shortcut_has_aumid(&path, app_user_model_id)? {
                // 找到部署登记后立即停止扫描。
                return Ok(true);
            }
        }
    }
    // 所有可读快捷方式都未匹配。
    Ok(false)
}

// 读取单个 .lnk 的 System.AppUserModel.ID 属性。
fn shortcut_has_aumid(path: &Path, app_user_model_id: &str) -> Result<bool> {
    // SAFETY: COM apartment 由 Platform::State 在同一 owner thread 初始化。
    let shell_link: IShellLinkW = unsafe {
        // 创建进程内 ShellLink COM 对象，仅用于读取现有文件。
        CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
    }
    // COM 创建失败属于 capability 平台错误。
    .map_err(|source| {
        notification_windows_error(
            "system_notification_capability",
            "CoCreateInstance(ShellLink)",
            source,
        )
    })?;
    // 将 ShellLink 查询为持久化文件接口。
    let persist: IPersistFile = shell_link
        // COM QueryInterface 不改变快捷方式内容。
        .cast()
        // cast 失败表示当前系统 ShellLink 契约异常。
        .map_err(|source| {
            notification_windows_error(
                "system_notification_capability",
                "ShellLink::cast(IPersistFile)",
                source,
            )
        })?;
    // 把文件路径编码为 NUL 终止 UTF-16。
    let wide_path: Vec<u16> = path
        // 取得原生 Windows 路径文本。
        .as_os_str()
        // 编码为 UTF-16 单元。
        .encode_wide()
        // 追加 Win32 所需终止符。
        .chain(std::iter::once(0))
        // 收集为调用期间稳定的缓冲。
        .collect();
    // SAFETY: wide_path 有效且以 NUL 结尾；STGM_READ 禁止写入。
    if unsafe { persist.Load(PCWSTR(wide_path.as_ptr()), STGM_READ) }.is_err() {
        // 损坏或不可解析的单个快捷方式不算匹配。
        return Ok(false);
    }
    // 将已载入的 ShellLink 查询为属性存储。
    let store: IPropertyStore = match shell_link.cast() {
        // 成功取得只读属性接口。
        Ok(store) => store,
        // 不提供属性存储的快捷方式不含可核验登记。
        Err(_) => return Ok(false),
    };
    // SAFETY: 属性键是静态有效值，store 生命周期覆盖返回值。
    let value = match unsafe { store.GetValue(&PKEY_APP_USER_MODEL_ID) } {
        // windows crate 的 PROPVARIANT 自带 Drop，可直接拥有返回值。
        Ok(value) => value,
        // 缺失属性或不可读属性均不算匹配。
        Err(_) => return Ok(false),
    };
    // 为 128 单元 AUMID 加 NUL 预留一个单元。
    let mut text = [0u16; 129];
    // SAFETY: value 与输出缓冲有效，缓冲覆盖 AUMID 上限。
    if unsafe { PropVariantToString(&value, &mut text) }.is_err() {
        // 非字符串属性不构成有效登记。
        return Ok(false);
    }
    // 定位输出中的 NUL 终止符。
    let length = text
        // 遍历固定缓冲中的 UTF-16 单元。
        .iter()
        // 找到首个终止单元。
        .position(|unit| *unit == 0)
        // API 若填满缓冲则使用完整长度。
        .unwrap_or(text.len());
    // 使用 UTF-16 转换后做区分大小写的身份精确比较。
    Ok(String::from_utf16_lossy(&text[..length]) == app_user_model_id)
}

// 转义 Toast XML 文本节点中的特殊字符。
fn escape_notification_xml(value: &str) -> String {
    // 预留原文本长度，常见路径只分配一次。
    let mut escaped = String::with_capacity(value.len());
    // 逐 Unicode 标量替换 XML 元字符。
    for character in value.chars() {
        // 按 XML 文本节点规则写入安全实体。
        match character {
            // 与号使用标准实体。
            '&' => escaped.push_str("&amp;"),
            // 小于号使用标准实体。
            '<' => escaped.push_str("&lt;"),
            // 大于号使用标准实体。
            '>' => escaped.push_str("&gt;"),
            // 双引号使用标准实体。
            '"' => escaped.push_str("&quot;"),
            // 单引号使用标准实体。
            '\'' => escaped.push_str("&apos;"),
            // 其他字符原样保留。
            _ => escaped.push(character),
        }
    }
    // 返回 owned 安全文本。
    escaped
}

// 把 Windows API 错误映射为 UIX 平台错误。
fn notification_windows_error(
    // 公开 Platform 操作名。
    context: &str,
    // 具体 Windows API 阶段。
    operation: &str,
    // 原始 Windows 错误。
    source: windows::core::Error,
) -> Error {
    // 访问被拒绝保持公开的权限错误类型。
    let code = if source.code().0 == 0x8007_0005_u32 as i32 {
        // E_ACCESSDENIED 对应权限不足。
        Errc::PermissionDenied
    } else {
        // 其余 WinRT/COM 错误统一归入平台错误。
        Errc::PlatformError
    };
    // 保留阶段名和系统错误文本。
    Error::new(
        // 使用上方稳定映射结果。
        code,
        // 诊断包含公开操作和具体 Windows 阶段。
        format!("Platform::{context}: {operation} failed: {source}"),
    )
}

// 把开始菜单读取错误映射为 UIX 类型。
fn notification_io_error(operation: &str, source: std::io::Error) -> Error {
    // 区分权限不足与一般平台 IO 故障。
    let code = match source.kind() {
        // 保留权限错误供调用方处理。
        std::io::ErrorKind::PermissionDenied => Errc::PermissionDenied,
        // 其他错误来自 Windows 环境或文件系统。
        _ => Errc::PlatformError,
    };
    // 返回带操作上下文的 owned 错误。
    Error::new(
        // 使用稳定错误码。
        code,
        // 诊断说明发生在 capability 探测。
        format!("Platform::system_notification_capability: {operation} failed: {source}"),
    )
}

// 管理 SHGetKnownFolderPath 返回的内存。
struct CoTaskMemWide(PWSTR);

impl Drop for CoTaskMemWide {
    // 释放 Shell 分配的 UTF-16 路径。
    fn drop(&mut self) {
        // SAFETY: guard 唯一拥有 SHGetKnownFolderPath 返回的指针。
        unsafe {
            // CoTaskMemFree 接受空指针，但系统成功路径应为非空。
            CoTaskMemFree(Some(self.0.as_ptr().cast()));
        }
    }
}
