// 引入当前模块的私有方向映射。
use super::*;

// 八个方向不得互换或退化为无方向值。
#[test]
fn maps_all_resize_edges_to_xdg_shell_edges() {
    // 按公共枚举顺序声明完整期望映射。
    let cases = [
        // 上边映射。
        (WindowResizeEdge::Top, XdgResizeEdge::Top),
        // 下边映射。
        (WindowResizeEdge::Bottom, XdgResizeEdge::Bottom),
        // 左边映射。
        (WindowResizeEdge::Left, XdgResizeEdge::Left),
        // 右边映射。
        (WindowResizeEdge::Right, XdgResizeEdge::Right),
        // 左上角映射。
        (WindowResizeEdge::TopLeft, XdgResizeEdge::TopLeft),
        // 右上角映射。
        (WindowResizeEdge::TopRight, XdgResizeEdge::TopRight),
        // 左下角映射。
        (WindowResizeEdge::BottomLeft, XdgResizeEdge::BottomLeft),
        // 右下角映射。
        (WindowResizeEdge::BottomRight, XdgResizeEdge::BottomRight),
    ];
    // 逐一检查所有方向映射。
    for (edge, expected) in cases {
        // 当前方向必须产生文档化的 xdg-shell 枚举。
        assert_eq!(resize_edge(edge), expected);
    }
}
