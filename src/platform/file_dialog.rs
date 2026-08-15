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

// 编码 Linux 与 macOS Provider 共用的可移植过滤器描述。
pub(super) fn portable_filters(filters: &[FileDialogFilter]) -> String {
    // 将每个公开值转换为 `名称 (*.ext *.ext)` 片段。
    filters
        // 保持调用方过滤器顺序。
        .iter()
        // 每个片段只使用构造时验证过的字符。
        .map(|filter| {
            // 为每个扩展名补回原生对话框需要的通配符前缀。
            let patterns = filter
                // 读取不可变规范扩展名。
                .extensions()
                // 按声明顺序编码模式。
                .iter()
                // 生成可移植星号点号语法。
                .map(|extension| format!("*.{extension}"))
                // 收集后以空格分隔同组模式。
                .collect::<Vec<_>>()
                // Linux 与 AppKit 解析器都接受空格分隔。
                .join(" ");
            // 名称与模式组成一个原生过滤器描述。
            format!("{} ({patterns})", filter.name())
        })
        // 收集各命名过滤器片段。
        .collect::<Vec<_>>()
        // 分号是现有 Linux Provider 的组分隔符。
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
    use super::{FileDialogFilter, portable_filters, windows_filters};

    // 三平台编码必须保留规范顺序并去除重复扩展名。
    #[test]
    fn file_dialog_filter_encodings_are_stable() {
        // 混合前缀与大小写构造同一过滤器。
        let images = FileDialogFilter::new("Images", ["PNG", "*.jpg", ".png"])
            // 测试夹具必须满足公开构造契约。
            .expect("image filter should be valid");
        // 可移植格式为 Linux 与 AppKit 提供命名模式组。
        assert_eq!(portable_filters(&[images.clone()]), "Images (*.png *.jpg)");
        // Win32 格式必须包含描述、模式和最终双 NUL。
        assert_eq!(windows_filters(&[images]), "Images\0*.png;*.jpg\0\0");
        // 不受限 Win32 对话框仍得到有效双 NUL 列表。
        assert_eq!(windows_filters(&[]), "All Files\0*.*\0\0");
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
