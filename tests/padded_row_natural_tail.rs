//! 有水平内边距的 flex 行中，grow 兄弟之后的自然宽度尾随子不得塌缩成内边距宽度。
#![cfg(feature = "test-harness")]
use uix::prelude::*;
use uix::ui::test_harness::TestApp;

#[test]
fn padded_row_keeps_natural_width_tail_after_grow_sibling() {
    let mut app = TestApp::new(
        (1536.0, 54.0),
        move || uix::uix!(
            r#"<Container direction="row" height="54" gap="8" align="center" style="paddingLeft: 14px; paddingRight: 14px;">
        <Container flexGrow={1} height="54" automationId="grower" />
        <Container direction="row" gap="2" automationId="tail">
            <Container width="46" height="40" />
            <Container width="46" height="40" />
        </Container>
    </Container>"#
        ),
    );
    app.settle().expect("布局未收敛");
    let snapshot = app.snapshot();
    let tail = snapshot.find("tail").unwrap().frame;
    assert!(
        (tail.w - 94.0).abs() < 0.01,
        "尾随自然宽度行塌缩: {tail:?}"
    );
}
