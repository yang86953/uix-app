// 引入非 UTF-8 Unix 路径到内核字节序列的转换能力。
use std::os::unix::ffi::OsStrExt;
// 引入 Provider 候选路径值。
use std::path::Path;

// 判断与 Command::new 同名查找契约对应的 Unix Provider 是否可执行。
pub(super) fn command_is_available(command: &str) -> bool {
    // 未配置 PATH 时不能证明无路径命令可用。
    let Some(path) = std::env::var_os("PATH") else {
        // 返回不可用能力，避免伪装 Provider 已就绪。
        return false;
    };
    // 把环境输入交给可独立验证的路径探测组件。
    command_is_available_in_path(command, &path)
}

// 在调用方给定的 PATH 列表中查找 Unix 可执行 Provider。
fn command_is_available_in_path(command: &str, path: &std::ffi::OsStr) -> bool {
    // 逐项检查 PATH，并保持空项表示当前目录的系统语义。
    std::env::split_paths(path).any(|directory| {
        // 拼接与 Command::new 查找相同的无路径命令名。
        let candidate = directory.join(command);
        // 只接受当前进程身份可执行的普通文件。
        is_executable_file(&candidate)
    })
}

// 把文件元数据收窄为 Unix 可执行 Provider 判定。
fn is_executable_file(path: &Path) -> bool {
    // 不可读取或不是普通文件的候选均不可作为 Provider 启动。
    let Ok(metadata) = path.metadata() else {
        // 保持 capability 查询为无副作用的布尔探测。
        return false;
    };
    // 目录即使可搜索也不能充当通知 Provider。
    if !metadata.is_file() {
        // 明确拒绝目录、设备与其他非普通文件。
        return false;
    }
    // 含 NUL 的路径无法传给 Unix access 系统调用。
    let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        // 把不可表示路径稳定分类为不可启动。
        return false;
    };
    // SAFETY: path 是存活的 NUL 结尾 C 字符串，access 不保留指针。
    unsafe {
        // 让内核按当前进程身份判断真实执行权限，而不是只观察任一执行位。
        libc::access(path.as_ptr(), libc::X_OK) == 0
    }
}

// Unix 目标聚焦验证 Provider 路径探测，不启动任何外部进程。
#[cfg(test)]
mod tests {
    // 引入被测的私有路径探测组件。
    use super::command_is_available_in_path;

    // 当前测试程序本身应能被执行位探测发现。
    #[test]
    fn finds_executable_provider_in_supplied_path() {
        // 取得当前测试程序的绝对路径。
        let executable = std::env::current_exe().expect("current test executable must be known");
        // 可执行文件必须具有父目录。
        let directory = executable
            .parent()
            .expect("test executable must have a parent");
        // 可执行文件必须具有可传给命令查找的文件名。
        let command = executable
            // 只读取末段名称，不向测试泄漏绝对路径匹配。
            .file_name()
            // 当前测试程序路径必须以文件名结尾。
            .and_then(std::ffi::OsStr::to_str)
            // Rust 测试二进制名称应可用 UTF-8 表示。
            .expect("test executable name must be UTF-8");
        // 单项 PATH 只包含当前测试程序目录。
        let path = std::env::join_paths([directory]).expect("test PATH must be representable");
        // 可执行普通文件必须形成可用 Provider。
        assert!(command_is_available_in_path(command, &path));
    }

    // 缺失 Provider 不得被伪装为可用。
    #[test]
    fn rejects_missing_provider_in_supplied_path() {
        // 取得当前测试程序所在的现有目录。
        let executable = std::env::current_exe().expect("current test executable must be known");
        // 可执行文件必须具有父目录。
        let directory = executable
            .parent()
            .expect("test executable must have a parent");
        // 单项 PATH 只包含该目录，避免读取宿主环境。
        let path = std::env::join_paths([directory]).expect("test PATH must be representable");
        // 确定不存在的命令名必须形成不可用 Provider。
        assert!(!command_is_available_in_path(
            // 使用带 UIX 前缀的稳定测试命令名避免碰撞。
            "uix-provider-that-does-not-exist",
            // 传入测试独占的 PATH 值。
            &path,
        ));
    }
}
