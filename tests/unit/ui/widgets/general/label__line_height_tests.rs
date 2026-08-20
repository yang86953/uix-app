    // 引入待验证的 Label 私有测量入口。
    use super::Label;
    // 引入公开行高与样式构造器。
    use crate::ui::theme::style::{LineHeight, Style};

    // 验证固定像素行高成为单行 Label 的固有高度。
    #[test]
    fn line_height_changes_label_intrinsic_height() {
        // 创建二十四像素显式行高样式。
        let style = Style::default().with_line_height(
            // 正像素值必须构造成功。
            LineHeight::pixels(24.0).expect("正像素行高必须有效"),
        );
        // 把统一样式应用到真实 Label 组件。
        let label = Label::new("line height").style(style);
        // 固有高度必须使用显式行盒而非默认视觉字高。
        assert_eq!(label.intrinsic_size().h, 24.0);
    }
