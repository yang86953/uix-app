//! Windows 系统通知的真实进程主线程验收入口。

// 非 Windows 目标保留可执行测试入口，但不伪造平台能力。
#[cfg(not(windows))]
fn main() {}

// Windows 目标验证公开 Platform 的配置、探测与发送行为。
#[cfg(windows)]
fn main() {
    // 引入类型化错误码用于精确断言失败语义。
    use uix::core::Errc;
    // 引入通知身份、能力状态与通知值。
    use uix::platform::services::{
        AppUserModelId, SystemNotification, SystemNotificationCapability,
    };
    // 引入唯一的平台 composition root。
    use uix::platform::Platform;

    // harness=false 保证本函数运行在进程初始主线程。
    let mut platform =
        Platform::new().expect("Platform must initialize on the process main thread");
    // 未配置身份的开发环境必须返回可解释状态。
    assert_eq!(
        // 查询不应创建或猜测应用身份。
        platform
            .system_notification_capability()
            .expect("unconfigured capability query must succeed"),
        // Windows 明确要求应用先配置 AUMID。
        SystemNotificationCapability::IdentityRequired,
    );
    // 未配置身份时发送必须失败。
    let unconfigured = platform
        // 使用有效通知内容触发身份门禁。
        .show_notification(SystemNotification::new("UIX", "未配置身份验收"))
        // 成功将违反产品契约。
        .expect_err("unconfigured notification must not fake success");
    // 失败类型必须保持为产品决策要求的 NotImplemented。
    assert_eq!(unconfigured.code(), Errc::NotImplemented);

    // 可选测试夹具模拟安装器创建并登记唯一开始菜单快捷方式。
    let fixture = std::env::var_os("UIX_NOTIFICATION_TEST_SETUP")
        // 仅明确设置环境变量时执行可恢复的系统测试写入。
        .map(|_| TemporaryNotificationShortcut::install());
    // 外部身份优先，其次使用测试夹具，最后使用未登记探针。
    let configured = std::env::var("UIX_NOTIFICATION_TEST_AUMID")
        // 测试夹具提供其刚登记的唯一 AUMID。
        .or_else(|_| {
            fixture
                // 借用夹具，不提前触发清理。
                .as_ref()
                // 克隆短生命周期的测试身份。
                .map(|fixture| fixture.app_user_model_id.clone())
                // 没有夹具时保留环境变量缺失错误。
                .ok_or(std::env::VarError::NotPresent)
        })
        // 没有安装身份时使用保证唯一的未登记探针。
        .unwrap_or_else(|_| "UIX.Notification.UnregisteredProbe.20260814".to_owned());
    // 验证并拥有外部提供的 AUMID。
    let identity = AppUserModelId::new(configured.clone())
        // 保留原文本供测试结束时精确清理同一通知历史。
        .expect("test AUMID must satisfy Windows limits");
    // Platform 只保存配置，不写系统登记。
    platform
        .set_notification_app_user_model_id(identity)
        .expect("Platform must accept a validated identity");
    // 读取真实开始菜单登记探测结果。
    let capability = platform
        .system_notification_capability()
        .expect("registered identity probe must complete");

    // 外部提供身份或启用夹具时进入真实通知提交路径。
    if std::env::var_os("UIX_NOTIFICATION_TEST_AUMID").is_some() || fixture.is_some() {
        // 已登记的安装身份必须被只读探测为 Available。
        assert_eq!(capability, SystemNotificationCapability::Available);
        // 使用唯一可见内容提交 Windows Toast。
        platform
            .show_notification(SystemNotification::new(
                // 标题标识 UIX 真通知验收。
                "UIX 系统通知验收",
                // 正文标识版本与日期，便于视觉核对。
                "0.0.1 · Windows AUMID 真发送 · 2026-08-14",
            ))
            // Windows 拒绝提交必须让测试失败。
            .expect("registered Windows notification must be accepted");
        // 可选短暂停留窗口仅供已授权的视觉捕获工具使用。
        if let Some(hold_millis) = std::env::var("UIX_NOTIFICATION_TEST_VISUAL_HOLD_MS")
            // 非整数配置不启用等待。
            .ok()
            // 把文本解析为毫秒。
            .and_then(|value| value.parse::<u64>().ok())
        {
            // 上限避免测试夹具长时间阻塞清理。
            let hold_millis = hold_millis.min(30_000);
            // 在已登记身份和快捷方式仍存在时保留通知。
            std::thread::sleep(std::time::Duration::from_millis(hold_millis));
        }
        // 取得 Windows 通知历史管理器。
        let history = windows::UI::Notifications::ToastNotificationManager::History()
            // 测试必须能够清理自己创建的通知记录。
            .expect("Windows notification history must be available");
        // 只清理本测试唯一 AUMID 下的通知。
        history
            // 使用与发送完全相同的应用身份。
            .ClearWithId(&windows::core::HSTRING::from(configured))
            // 清理失败会保留快捷方式 guard 作为异常恢复。
            .expect("temporary notification history must be cleared");
        // 显式析构 Platform，确保 COM 对象先于测试快捷方式清理。
        drop(platform);
        // 正常路径精确删除本次测试创建的唯一快捷方式。
        if let Some(fixture) = fixture {
            // 清理失败会让测试失败并保留确切路径供恢复。
            fixture.cleanup();
        }
        // 真实发送路径完成后结束测试进程。
        return;
    }

    // 默认开发环境的随机身份必须被探测为未登记。
    assert_eq!(
        // 使用上方真实只读扫描结果。
        capability,
        // 禁止把仅配置字符串当成系统登记成功。
        SystemNotificationCapability::IdentityUnregistered,
    );
    // 未登记身份时发送必须在进入 WinRT 前失败。
    let unregistered = platform
        // 使用有效内容触发部署登记门禁。
        .show_notification(SystemNotification::new("UIX", "未登记身份验收"))
        // 成功将构成伪成功。
        .expect_err("unregistered notification must not fake success");
    // 缺少安装登记使用稳定的 InvalidState。
    assert_eq!(unregistered.code(), Errc::InvalidState);
}

// 仅测试代码模拟部署层登记，并保证唯一文件可恢复。
#[cfg(windows)]
struct TemporaryNotificationShortcut {
    // 与快捷方式 System.AppUserModel.ID 完全一致的测试身份。
    app_user_model_id: String,
    // 本次进程唯一拥有的开始菜单快捷方式路径。
    path: std::path::PathBuf,
    // 标记正常清理是否已完成。
    active: bool,
}

// 测试夹具负责创建与精确清理，不进入 UIX 生产实现。
#[cfg(windows)]
impl TemporaryNotificationShortcut {
    // 在当前用户开始菜单创建唯一测试快捷方式。
    fn install() -> Self {
        // 引入 Property System 键类型。
        use windows::Win32::Foundation::PROPERTYKEY;
        // 引入 COM 创建、内存释放和快捷方式持久化接口。
        use windows::Win32::System::Com::{
            CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, IPersistFile,
        };
        // 引入 PROPVARIANT 字符串值构造。
        use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
        // 引入当前用户 Programs known folder 与 ShellLink。
        use windows::Win32::UI::Shell::{
            FOLDERID_Programs, IShellLinkW, KNOWN_FOLDER_FLAG, SHGetKnownFolderPath, ShellLink,
        };
        // 引入快捷方式属性存储接口。
        use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
        // 引入 GUID、COM cast 与 UTF-16 指针类型。
        use windows::core::{GUID, Interface, PCWSTR};

        // 测试身份保持在 Windows 128 单元上限内，并与生产名称隔离。
        let app_user_model_id = format!("UIX.Notification.VisualTest.{}", std::process::id());
        // SAFETY: Platform 已在当前主线程初始化 COM；known-folder GUID 为静态值。
        let programs = unsafe {
            // 零 flags 与空 token 只查询现存的当前用户 Programs 目录。
            SHGetKnownFolderPath(&FOLDERID_Programs, KNOWN_FOLDER_FLAG(0), None)
        }
        // 测试环境必须能够解析开始菜单路径。
        .expect("current-user Start Menu Programs folder must be available");
        // SAFETY: Shell 返回有效的 NUL 终止 UTF-16 路径。
        let programs_path = unsafe { programs.to_string() }
            // 非法 UTF-16 必须终止测试。
            .expect("Start Menu Programs path must be valid UTF-16");
        // SAFETY: 指针来自 SHGetKnownFolderPath，且只在转换后释放一次。
        unsafe {
            // 释放 Shell 分配的 known-folder 路径。
            CoTaskMemFree(Some(programs.as_ptr().cast()));
        }
        // 使用进程 ID 生成本次运行独占的快捷方式路径。
        let path = std::path::PathBuf::from(programs_path)
            // 文件名明确标识为 UIX 临时验收夹具。
            .join(format!(
                "UIX Notification Visual Test {}.lnk",
                std::process::id()
            ));
        // 绝不覆盖用户或前次运行留下的同名文件。
        assert!(
            !path.exists(),
            "temporary notification shortcut already exists"
        );

        // System.AppUserModel.ID 的 Windows Property System 键。
        const PKEY_APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
            // AppUserModel 属性集的固定 fmtid。
            fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
            // pid 5 对应 System.AppUserModel.ID。
            pid: 5,
        };
        // SAFETY: 当前主线程已经初始化 COM apartment。
        let shell_link: IShellLinkW = unsafe {
            // 创建进程内 ShellLink 对象作为测试安装器夹具。
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
        }
        // COM 创建失败必须终止真实通知测试。
        .expect("ShellLink COM object must be available");
        // 取得当前测试可执行文件作为快捷方式目标。
        let executable = std::env::current_exe().expect("test executable path must be available");
        // 编码目标路径为 NUL 终止 UTF-16。
        let executable_wide = windows_path(&executable);
        // SAFETY: 缓冲有效且以 NUL 结尾；SetPath 只修改内存中的新 ShellLink。
        unsafe {
            // 设置快捷方式目标，不启动任何进程。
            shell_link
                .SetPath(PCWSTR(executable_wide.as_ptr()))
                .expect("temporary shortcut target must be accepted");
        }
        // 查询新 ShellLink 的属性存储接口。
        let store: IPropertyStore = shell_link
            // QueryInterface 不触碰现有系统文件。
            .cast()
            // Windows ShellLink 必须提供属性存储。
            .expect("ShellLink must expose IPropertyStore");
        // 把测试 AUMID 构造为 owned PROPVARIANT。
        let value = PROPVARIANT::from(app_user_model_id.as_str());
        // SAFETY: 属性键和值在调用期间有效，目标是尚未保存的新快捷方式。
        unsafe {
            // 写入 System.AppUserModel.ID。
            store
                .SetValue(&PKEY_APP_USER_MODEL_ID, &value)
                .expect("temporary shortcut AUMID must be writable");
            // 提交内存中的属性变更。
            store
                .Commit()
                .expect("temporary shortcut properties must commit");
        }
        // 查询持久化文件接口以保存唯一测试快捷方式。
        let persist: IPersistFile = shell_link
            // QueryInterface 只获取同一内存对象的接口。
            .cast()
            // ShellLink 必须支持 IPersistFile。
            .expect("ShellLink must expose IPersistFile");
        // 编码唯一快捷方式路径为 NUL 终止 UTF-16。
        let shortcut_wide = windows_path(&path);
        // SAFETY: 路径缓冲有效；上方断言保证不会覆盖现有文件。
        unsafe {
            // 将新快捷方式保存到当前用户开始菜单。
            persist
                .Save(PCWSTR(shortcut_wide.as_ptr()), true)
                .expect("temporary notification shortcut must save");
        }
        // 保存后必须能够由生产只读探测看到该文件。
        assert!(
            path.is_file(),
            "temporary notification shortcut was not created"
        );
        // 返回 active guard，异常退出时也会尝试清理。
        Self {
            // 交给 Platform 配置的同一身份。
            app_user_model_id,
            // 只记录本次创建的精确路径。
            path,
            // 标记尚未执行正常清理。
            active: true,
        }
    }

    // 正常结束时删除精确的测试快捷方式。
    fn cleanup(mut self) {
        // 删除仅由本 guard 创建且已核验存在的文件。
        std::fs::remove_file(&self.path).expect("temporary notification shortcut must be removed");
        // 禁止 Drop 再次删除。
        self.active = false;
        // 删除后确认系统测试写入已恢复。
        assert!(
            !self.path.exists(),
            "temporary notification shortcut still exists"
        );
    }
}

// 异常退出时尽力恢复测试创建的唯一快捷方式。
#[cfg(windows)]
impl Drop for TemporaryNotificationShortcut {
    // 仅清理本 guard 仍拥有的测试文件。
    fn drop(&mut self) {
        // 正常 cleanup 已处理时不重复删除。
        if self.active {
            // 忽略 panic 路径的清理错误，保留原始测试失败。
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

// 把 Windows 路径编码为 NUL 终止 UTF-16。
#[cfg(windows)]
fn windows_path(path: &std::path::Path) -> Vec<u16> {
    // 引入 Windows 原生路径编码 trait。
    use std::os::windows::ffi::OsStrExt;
    // 编码原生路径并追加 Win32 终止符。
    path.as_os_str()
        // 转换为 UTF-16 单元。
        .encode_wide()
        // 追加单个 NUL。
        .chain(std::iter::once(0))
        // 收集为调用期间稳定的 owned 缓冲。
        .collect()
}
