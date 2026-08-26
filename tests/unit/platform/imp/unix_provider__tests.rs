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
