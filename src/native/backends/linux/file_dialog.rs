// ============================================================================
// platform/linux/file_dialog.rs — Linux 原生文件对话框 adapter
// ============================================================================
//
// Uses `zenity` (GNOME) or `kdialog` (KDE) for native file dialogs.
// Detection: prefer KDE_DESKTOP_SESSION / DESKTOP_SESSION env vars.
// Falls back to zenity if neither is detected or available.
// ============================================================================

// 引入 platform capabilities Module 统一拥有的外部对话框退出分类。
use crate::native::capabilities::services::file_dialog_process::{
    ExternalDialogOutcome, classify_external_dialog_exit, parse_confirmed_dialog_path,
    parse_confirmed_dialog_paths,
};
use crate::native::{Errc, Error, Result};

// 通过 Platform System 窄 Adapter 打开多选文件面板。
pub(crate) fn choose_files(
    // 标题已经由公开 Platform 门面验证。
    title: &str,
    // Zenity 编码由 Platform 私有过滤器组件生成。
    zenity_filters: &str,
    // KDialog 编码由 Platform 私有过滤器组件生成。
    kdialog_filters: &str,
) -> Result<Option<Vec<String>>> {
    // 运行时桌面探测只选择对应 Provider 的精确协议。
    if use_kde() {
        // KDE 桌面只接收 Qt name filter 编码。
        kde_open_file(title, kdialog_filters)
    } else {
        // GNOME 与通用 GTK 桌面只接收 Zenity 编码。
        zenity_open_file(title, zenity_filters)
    }
}

// 通过 Platform System 窄 Adapter 打开保存面板。
pub(crate) fn choose_save_file(
    // 标题已经由公开 Platform 门面验证。
    title: &str,
    // Zenity 编码由 Platform 私有过滤器组件生成。
    zenity_filters: &str,
    // KDialog 编码由 Platform 私有过滤器组件生成。
    kdialog_filters: &str,
) -> Result<Option<String>> {
    // 保存入口与打开入口使用相同的桌面 Provider 选择规则。
    if use_kde() {
        // KDE 桌面只接收 Qt name filter 编码。
        kde_save_file(title, kdialog_filters)
    } else {
        // GNOME 与通用 GTK 桌面只接收 Zenity 编码。
        zenity_save_file(title, zenity_filters)
    }
}

// 通过 Platform System 窄 Adapter 打开目录面板。
pub(crate) fn choose_folder(title: &str) -> Result<Option<String>> {
    // 复用与打开/保存入口相同的桌面 Provider 选择规则。
    if use_kde() {
        kde_open_folder(title)
    } else {
        zenity_open_folder(title)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Desktop environment detection
// ════════════════════════════════════════════════════════════════════════════

fn use_kde() -> bool {
    std::env::var("KDE_DESKTOP_SESSION").is_ok()
        || std::env::var("KDE_SESSION_VERSION").is_ok()
        || std::env::var("DESKTOP_SESSION")
            .map(|s| s.to_lowercase().contains("kde"))
            .unwrap_or(false)
}

// ════════════════════════════════════════════════════════════════════════════
// Zenity (GNOME / generic GTK)
// ════════════════════════════════════════════════════════════════════════════

fn zenity_open_file(title: &str, filters: &str) -> Result<Option<Vec<String>>> {
    let mut cmd = std::process::Command::new("zenity");
    cmd.args([
        "--file-selection",
        "--title",
        title,
        "--multiple",
        "--separator",
        "\n",
    ]);

    if !filters.is_empty() {
        for part in filters.split(';') {
            let part = part.trim();
            if !part.is_empty() {
                cmd.arg("--file-filter");
                cmd.arg(part);
            }
        }
    }

    let output = spawn_dialog("zenity", &mut cmd)?;
    // 只把明确取消码归一化为成功空值。
    if !dialog_was_confirmed("zenity", &output)? {
        return Ok(None);
    }
    // 确认结果必须包含一条或多条可无损表示的路径。
    Ok(Some(parse_confirmed_dialog_paths(
        "zenity",
        &output.stdout,
    )?))
}

fn zenity_save_file(title: &str, filters: &str) -> Result<Option<String>> {
    let mut cmd = std::process::Command::new("zenity");
    cmd.args([
        "--file-selection",
        "--title",
        title,
        "--save",
        "--confirm-overwrite",
    ]);

    if !filters.is_empty() {
        for part in filters.split(';') {
            let part = part.trim();
            if !part.is_empty() {
                cmd.arg("--file-filter");
                cmd.arg(part);
            }
        }
    }

    let output = spawn_dialog("zenity", &mut cmd)?;
    // 只把明确取消码归一化为成功空值。
    if !dialog_was_confirmed("zenity", &output)? {
        return Ok(None);
    }
    // 确认结果必须包含一条可无损表示的路径。
    Ok(Some(parse_confirmed_dialog_path("zenity", &output.stdout)?))
}

fn zenity_open_folder(title: &str) -> Result<Option<String>> {
    let mut cmd = std::process::Command::new("zenity");
    cmd.args(["--file-selection", "--title", title, "--directory"]);
    let output = spawn_dialog("zenity", &mut cmd)?;
    // 只把明确取消码归一化为成功空值。
    if !dialog_was_confirmed("zenity", &output)? {
        return Ok(None);
    }
    // 确认结果必须包含一条可无损表示的路径。
    Ok(Some(parse_confirmed_dialog_path("zenity", &output.stdout)?))
}

// ════════════════════════════════════════════════════════════════════════════
// KDialog (KDE)
// ════════════════════════════════════════════════════════════════════════════

fn kde_open_file(title: &str, filters: &str) -> Result<Option<Vec<String>>> {
    let mut cmd = std::process::Command::new("kdialog");
    // KDialog 必须显式开启多选并用逐行输出消除空格分隔歧义。
    cmd.args([
        "--title",
        title,
        "--multiple",
        "--separate-output",
        "--getopenfilename",
        ".",
        filters,
    ]);
    let output = spawn_dialog("kdialog", &mut cmd)?;
    // 只把明确取消码归一化为成功空值。
    if !dialog_was_confirmed("kdialog", &output)? {
        return Ok(None);
    }
    // 确认结果必须包含一条或多条可无损表示的路径。
    Ok(Some(parse_confirmed_dialog_paths(
        "kdialog",
        &output.stdout,
    )?))
}

fn kde_save_file(title: &str, filters: &str) -> Result<Option<String>> {
    let mut cmd = std::process::Command::new("kdialog");
    cmd.args(["--title", title, "--getsavefilename", ".", filters]);
    let output = spawn_dialog("kdialog", &mut cmd)?;
    // 只把明确取消码归一化为成功空值。
    if !dialog_was_confirmed("kdialog", &output)? {
        return Ok(None);
    }
    // 确认结果必须包含一条可无损表示的路径。
    Ok(Some(parse_confirmed_dialog_path(
        "kdialog",
        &output.stdout,
    )?))
}

fn kde_open_folder(title: &str) -> Result<Option<String>> {
    let mut cmd = std::process::Command::new("kdialog");
    cmd.args(["--title", title, "--getexistingdirectory", "."]);
    let output = spawn_dialog("kdialog", &mut cmd)?;
    // 只把明确取消码归一化为成功空值。
    if !dialog_was_confirmed("kdialog", &output)? {
        return Ok(None);
    }
    // 确认结果必须包含一条可无损表示的路径。
    Ok(Some(parse_confirmed_dialog_path(
        "kdialog",
        &output.stdout,
    )?))
}

// 把子进程输出收窄为确认布尔值或 typed failure。
fn dialog_was_confirmed(program: &str, output: &std::process::Output) -> Result<bool> {
    // 由 capabilities Module 统一解释退出码与有界标准错误。
    let outcome = classify_external_dialog_exit(program, output.status.code(), &output.stderr)?;
    // 只有确认结果允许调用方继续解析标准输出。
    Ok(matches!(outcome, ExternalDialogOutcome::Confirmed))
}

/// 启动对话框进程;进程无法启动(程序缺失等)视为对话框本身失败。
fn spawn_dialog(program: &str, cmd: &mut std::process::Command) -> Result<std::process::Output> {
    cmd.output().map_err(|err| {
        // 权限拒绝与普通启动 I/O 故障需要可区分分类。
        let code = if err.kind() == std::io::ErrorKind::PermissionDenied {
            // 可执行文件存在但当前进程无启动权限。
            Errc::PermissionDenied
        } else {
            // 缺失程序与其他启动故障保留现有 I/O 分类。
            Errc::IoError
        };
        Error::new(
            // 传播已经按 OS 错误种类选择的 typed code。
            code,
            format!("LinuxFileDialog: failed to launch {program}: {err}"),
        )
    })
}
