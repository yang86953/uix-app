    // 引入当前模块的私有缩放几何。
    use super::*;

    fn drag(edge: WindowResizeEdge) -> WindowsResizeDrag {
        WindowsResizeDrag {
            edge,
            cursor_x: 100,
            cursor_y: 100,
            left: 200,
            top: 200,
            right: 600,
            bottom: 500,
        }
    }

    fn limits() -> ResizeTrackLimits {
        ResizeTrackLimits {
            min_w: 160,
            min_h: 120,
            max_w: 800,
            max_h: 600,
        }
    }

    // 八个方向都必须只移动其声明的窗口边。
    #[test]
    fn resize_edges_move_only_declared_sides() {
        let cases = [
            (WindowResizeEdge::Top, (200, 250, 600, 500)),
            (WindowResizeEdge::Bottom, (200, 200, 600, 550)),
            (WindowResizeEdge::Left, (250, 200, 600, 500)),
            (WindowResizeEdge::Right, (200, 200, 650, 500)),
            (WindowResizeEdge::TopLeft, (250, 250, 600, 500)),
            (WindowResizeEdge::TopRight, (200, 250, 650, 500)),
            (WindowResizeEdge::BottomLeft, (250, 200, 600, 550)),
            (WindowResizeEdge::BottomRight, (200, 200, 650, 550)),
        ];
        for (edge, expected) in cases {
            let rect = resize_drag_rect(drag(edge), 150, 150, limits());
            assert_eq!((rect.left, rect.top, rect.right, rect.bottom), expected);
        }
    }

    // 缩小时必须保留固定边并遵守最小跟踪尺寸。
    #[test]
    fn resize_clamps_extent_without_moving_opposite_edge() {
        let rect = resize_drag_rect(drag(WindowResizeEdge::TopLeft), 900, 900, limits());
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (440, 380, 600, 500)
        );
    }
