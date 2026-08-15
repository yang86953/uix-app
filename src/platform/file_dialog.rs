// 引入平台公开过滤器值与 typed error。
use super::services::FileDialogFilter;
// 引入跨平台核心错误契约。
use crate::core::{Errc, Error, Result};

// 在进入原生 Provider 前验证同步对话框标题。
pub(super) fn validate_title(operation: &str, title: &str) -> Result<()> {
    // 标题需要可见文本且不能被 C ABI 截断。
    if title.trim().is_empty() || title.contains('\0') || title.chars().any(char::is_control) {
        // 返回稳定的调用方参数错误。
        return Err(Error::new(
            // 非法标题属于调用方输入错误。
            Errc::InvalidArgument,
            // 保留具体公开入口便于诊断。
            format!(
                "{operation}: title must be non-empty and contain no control or NUL characters"
            ),
        ));
    }
    // 标题满足三平台同步对话框前置条件。
    Ok(())
}

// 把单个过滤器的规范扩展名编码为外部 Provider 共用的模式列表。
fn filter_patterns(filter: &FileDialogFilter) -> String {
    // 为每个扩展名补回原生对话框需要的通配符前缀。
    filter
        // 读取不可变规范扩展名。
        .extensions()
        // 按声明顺序编码模式。
        .iter()
        // 生成星号点号语法。
        .map(|extension| format!("*.{extension}"))
        // 收集后以空格分隔同组模式。
        .collect::<Vec<_>>()
        // 两个 Linux Provider 都接受空格分隔的同组模式。
        .join(" ")
}

// 编码 Zenity 每个 `--file-filter` 参数所需的命名组。
pub(super) fn zenity_filters(filters: &[FileDialogFilter]) -> String {
    // 将每个公开值转换为 `名称 | *.ext *.ext` 片段。
    filters
        // 保持调用方过滤器顺序。
        .iter()
        // 每个片段只使用构造时验证过的字符。
        .map(|filter| {
            // 生成当前命名组的模式列表。
            let patterns = filter_patterns(filter);
            // Zenity 使用竖线分隔显示名称与模式。
            format!("{} | {patterns}", filter.name())
        })
        // 收集各命名过滤器片段。
        .collect::<Vec<_>>()
        // 分号仅供 Linux Adapter 拆成多个独立命令参数。
        .join(";")
}

// 编码 KDialog/Qt name filter 所需的命名组列表。
pub(super) fn kdialog_filters(filters: &[FileDialogFilter]) -> String {
    // 将每个公开值转换为 `名称 (*.ext *.ext)` 片段。
    filters
        // 保持调用方过滤器顺序。
        .iter()
        // 每组单独保留显示名称与模式。
        .map(|filter| {
            // 生成当前命名组的模式列表。
            let patterns = filter_patterns(filter);
            // KDialog 把组间竖线转换为 Qt name filter 换行。
            format!("{} ({patterns})", filter.name())
        })
        // 收集各命名过滤器片段。
        .collect::<Vec<_>>()
        // 当前 KDialog 源码使用竖线分隔多个 name filters。
        .join("|")
}

// 编码 AppKit allowedFileTypes 解析组件所需的纯扩展名模式。
pub(super) fn macos_filters(filters: &[FileDialogFilter]) -> String {
    // AppKit 不展示命名过滤器组，只需要规范扩展名事实。
    filters
        // 保持调用方过滤器组顺序。
        .iter()
        // 展开每组已经稳定去重的扩展名。
        .flat_map(|filter| filter.extensions().iter())
        // 补回 AppKit 解析器识别的星号点号前缀。
        .map(|extension| format!("*.{extension}"))
        // 收集所有纯模式。
        .collect::<Vec<_>>()
        // 分号只分隔扩展名，不携带可能被误识别的显示名称。
        .join(";")
}

// 编码 Win32 OPENFILENAMEW 要求的双 NUL 过滤器列表。
pub(super) fn windows_filters(filters: &[FileDialogFilter]) -> String {
    // 无过滤器时显式提供不受限组，避免传入不完整的单 NUL 列表。
    if filters.is_empty() {
        // Win32 要求描述、模式与列表末尾分别以 NUL 结束。
        return "All Files\0*.*\0\0".to_owned();
    }
    // 预留一个可增长字符串保存内嵌 NUL。
    let mut encoded = String::new();
    // 按调用方顺序写入每组描述与模式。
    for filter in filters {
        // 写入用户可见名称。
        encoded.push_str(filter.name());
        // 结束描述字段。
        encoded.push('\0');
        // 逐项写入同组分号分隔的 Win32 通配符。
        for (index, extension) in filter.extensions().iter().enumerate() {
            // 第二项起先写入模式分隔符。
            if index > 0 {
                // Win32 同组模式使用分号分隔。
                encoded.push(';');
            }
            // 写入星号点号前缀。
            encoded.push_str("*.");
            // 写入已验证的规范扩展名。
            encoded.push_str(extension);
        }
        // 结束模式字段。
        encoded.push('\0');
    }
    // 再追加一个 NUL 形成列表终止的双 NUL。
    encoded.push('\0');
    // 返回供 UTF-16 编码器逐字符保留的完整列表。
    encoded
}

// 文件对话框契约测试只验证纯值与编码，不弹出真实窗口。
#[cfg(test)]
mod tests {
    // 引入私有编码器与公开过滤器构造器。
    use super::{
        FileDialogFilter, kdialog_filters, macos_filters, windows_filters, zenity_filters,
    };

    // 三平台编码必须保留规范顺序并去除重复扩展名。
    #[test]
    fn file_dialog_filter_encodings_are_stable() {
        // 混合前缀与大小写构造同一过滤器。
        let images = FileDialogFilter::new("Images", ["PNG", "*.jpg", ".png"])
            // 测试夹具必须满足公开构造契约。
            .expect("image filter should be valid");
        // 第二组验证 Linux Provider 的多组分隔与名称空格。
        let source = FileDialogFilter::new("Source code", ["rs"])
            // 测试夹具必须满足公开构造契约。
            .expect("source filter should be valid");
        // Zenity 每个组使用名称与模式竖线，并由 Adapter 拆分分号。
        assert_eq!(
            zenity_filters(&[images.clone(), source.clone()]),
            "Images | *.png *.jpg;Source code | *.rs",
        );
        // KDialog 使用括号内模式与组间竖线。
        assert_eq!(
            kdialog_filters(&[images.clone(), source]),
            "Images (*.png *.jpg)|Source code (*.rs)",
        );
        // 空切片不得生成伪造的 Provider 过滤器。
        assert_eq!(zenity_filters(&[]), "");
        // KDialog 空切片同样保留不限制类型语义。
        assert_eq!(kdialog_filters(&[]), "");
        // Win32 格式必须包含描述、模式和最终双 NUL。
        assert_eq!(windows_filters(&[images]), "Images\0*.png;*.jpg\0\0");
        // 不受限 Win32 对话框仍得到有效双 NUL 列表。
        assert_eq!(windows_filters(&[]), "All Files\0*.*\0\0");
    }

    // AppKit 编码不能把小写显示名称降格成可选文件类型。
    #[test]
    fn macos_filter_encoding_excludes_display_names() {
        // 小写名称曾会被 AppKit 解析器误识别为裸扩展名。
        let images = FileDialogFilter::new("images", ["png", "jpg"])
            // 测试夹具必须满足公开构造契约。
            .expect("lowercase display name should be valid");
        // 含空格名称同样只能承担应用侧描述职责。
        let source = FileDialogFilter::new("source code", ["rs"])
            // 测试夹具必须满足公开构造契约。
            .expect("spaced display name should be valid");
        // 编码结果只包含规范扩展名模式，不包含任一显示名称词。
        assert_eq!(macos_filters(&[images, source]), "*.png;*.jpg;*.rs");
    }

    // 模糊通配符、路径和空扩展名必须在 Provider 前被拒绝。
    #[test]
    fn file_dialog_filter_rejects_ambiguous_extensions() {
        // 全通配符不能伪装成确定文件类型。
        assert!(FileDialogFilter::new("All", ["*.*"]).is_err());
        // 路径模式不能跨平台稳定解释。
        assert!(FileDialogFilter::new("Path", ["folder/png"]).is_err());
        // 无扩展名过滤器应改为向对话框传空切片。
        assert!(FileDialogFilter::new("Empty", std::iter::empty::<String>()).is_err());
    }
}
