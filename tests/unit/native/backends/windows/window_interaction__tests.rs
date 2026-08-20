    // 引入当前模块的私有方向映射与常量。
    use super::*;

    // 八个方向不得互换或退化为标题栏移动。
    #[test]
    fn maps_all_resize_edges_to_win32_hit_tests() {
        // 按公共枚举顺序声明完整期望映射。
        let cases = [
            // 上边映射。
            (WindowResizeEdge::Top, HTTOP as usize),
            // 下边映射。
            (WindowResizeEdge::Bottom, HTBOTTOM as usize),
            // 左边映射。
            (WindowResizeEdge::Left, HTLEFT as usize),
            // 右边映射。
            (WindowResizeEdge::Right, HTRIGHT as usize),
            // 左上角映射。
            (WindowResizeEdge::TopLeft, HTTOPLEFT as usize),
            // 右上角映射。
            (WindowResizeEdge::TopRight, HTTOPRIGHT as usize),
            // 左下角映射。
            (WindowResizeEdge::BottomLeft, HTBOTTOMLEFT as usize),
            // 右下角映射。
            (WindowResizeEdge::BottomRight, HTBOTTOMRIGHT as usize),
        ];
        // 逐一检查所有方向映射。
        for (edge, expected) in cases {
            // 当前方向必须产生文档化的 Win32 hit-test 常量。
            assert_eq!(resize_hit_test(edge), expected);
        }
    }
