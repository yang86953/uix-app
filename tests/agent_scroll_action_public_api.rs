// 声明本文件只在启用 test-harness 时编译 Agent 滚动动作契约测试。
#![cfg(feature = "test-harness")]

//! Agent 语义滚动动作契约：目标已声明滚动能力时，边界无位移是合法动作结果。
//!
//! 回归背景：滚动视口在内容边界（或范围未就绪）时把滚轮夹取成无位移，
//! 曾沿 NotHandled 一路上报成 internal command failure，误导调用方以为
//! 框架内部故障；实际位移应由调用方经语义快照核对。

use uix_app::core::Point;
use uix_app::prelude::*;
use uix_app::ui::test_harness::TestApp;

fn scroll_list() -> ViewNode {
    let rows: Vec<ViewNode> = (0..40)
        .map(|index| label(format!("第 {index} 行")).automation_id(format!("row.{index}")))
        .collect();
    scroll(column_fit(rows))
        .vertical()
        .automation_id("list.scroll")
        .build()
}

#[test]
fn scroll_moves_content_and_boundary_noop_is_ok() {
    let mut app = TestApp::new((320.0, 200.0), scroll_list);
    let first_row_y = |app: &TestApp| {
        app.snapshot()
            .find("row.0")
            .expect("首行应在语义树中")
            .frame
            .y
    };

    // 基准：向下滚动（TestApp 约定负值向下）产生真实位移。
    let top = first_row_y(&app);
    app.scroll("list.scroll", Point::new(0.0, -0.5))
        .expect("向下滚动应成功");
    let scrolled = first_row_y(&app);
    assert!(
        scrolled < top,
        "向下滚动后首行应上移：{top} -> {scrolled}"
    );

    // 滚回顶部：动作成功且位移回到原位。
    app.scroll("list.scroll", Point::new(0.0, 5.0))
        .expect("滚回顶部应成功");
    assert_eq!(first_row_y(&app), top, "滚回顶部后首行应回到原位");

    // 已在顶部继续向上：夹取成无位移，动作本身仍应成功（回归点）。
    app.scroll("list.scroll", Point::new(0.0, 5.0))
        .expect("边界无位移的滚动不应报动作失败");
    assert_eq!(
        first_row_y(&app),
        top,
        "边界滚动不得产生位移"
    );
}
