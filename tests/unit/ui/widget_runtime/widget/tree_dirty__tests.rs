// 引入被测树级失效实现与私有辅助函数。
use super::*;

// 分数滚动无法由整数纹理搬移精确表达时必须回退为重绘。
#[test]
fn fractional_scroll_delta_is_not_rounded_into_texture_copy() {
    // 空树足以验证合成事务不会登记错误的 retained move。
    let mut tree = WidgetTree::new();
    let viewport = Rect::new(0.0, 0.0, 120.0, 80.0);
    // 十二点五像素若被取整，旧文字像素会与当前布局逐帧漂移。
    assert!(!tree.push_scroll_composite(viewport, 0.0, 12.5));
    assert_eq!(tree.scroll_region_moves(), None);
}

// 整数视口和整数位移仍应保留局部纹理搬移快路径。
#[test]
fn integral_scroll_delta_keeps_exact_texture_copy() {
    let mut tree = WidgetTree::new();
    let viewport = Rect::new(4.0, 6.0, 120.0, 80.0);
    assert!(tree.push_scroll_composite(viewport, 0.0, 12.0));
    assert_eq!(
        tree.scroll_region_moves(),
        Some(vec![ScrollCopy::new(viewport, 0.0, 12.0)])
    );
}

// 内容视口外的滚动条沟槽必须形成互不重叠的独立重绘区域。
#[test]
fn scroll_chrome_regions_preserve_right_and_bottom_gutters() {
    let frame = Rect::new(10.0, 20.0, 100.0, 80.0);
    let viewport = Rect::new(10.0, 20.0, 92.0, 72.0);
    assert_eq!(
        scroll_chrome_regions(frame, viewport),
        vec![
            Rect::new(10.0, 92.0, 100.0, 8.0),
            Rect::new(102.0, 20.0, 8.0, 72.0),
        ]
    );
}
