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
