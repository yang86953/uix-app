use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::layout::engine::BoxModel;
use crate::ui::widgets::Container;

#[test]
fn measure_clamps_container_intrinsic_size() {
    let measured = Container::new()
        .size(80.0, 30.0)
        .measure(Constraints::loose(Size::new(50.0, 40.0)));

    assert_eq!(measured, Size::new(50.0, 30.0));
}

#[test]
fn measure_excludes_margin_from_intrinsic_size() {
    let measured = Container::new()
        .size(80.0, 24.0)
        .margin(EdgeInsets::new(1.0, 2.0, 3.0, 4.0))
        .measure(Constraints::unconstrained());

    assert_eq!(measured, Size::new(80.0, 24.0));
}

#[test]
fn content_rect_does_not_subtract_margin_again() {
    // 父级已把 margin 算进子项 frame 起点；content_rect 只扣 border+padding。
    let box_model = BoxModel {
        margin: EdgeInsets::new(12.0, 0.0, 12.0, 0.0),
        border_width: EdgeInsets::zero(),
        padding: EdgeInsets::new(8.0, 4.0, 8.0, 4.0),
    };
    let frame = Rect::new(12.0, 10.0, 100.0, 32.0);
    assert_eq!(
        box_model.content_rect(frame),
        Rect::new(20.0, 14.0, 84.0, 24.0)
    );
    assert_eq!(box_model.visual_rect(frame), frame);
}

#[test]
fn measure_uses_cached_content_height_without_explicit_height() {
    let container = Container::new().w(120.0);
    container.cached_content_size.set(Size::new(100.0, 48.0));

    let measured = container.measure(Constraints::unconstrained());

    assert_eq!(measured, Size::new(120.0, 48.0));
}

#[test]
fn measure_children_filters_hidden_and_preserves_layout_metadata() {
    let mut tree = WidgetTree::new();
    let host = tree.set_root(Box::new(Container::new()));
    let visible = tree.add_child(
        host,
        Box::new(
            Container::new()
                .style(
                    Style::container()
                        .with_grid_cell(2)
                        .with_grid_column_span(3)
                        .with_grid_row_span(4),
                )
                .size(20.0, 10.0)
                .flex_grow(2.0)
                .flex_shrink(0.25),
        ),
    );
    let hidden = tree.add_child(host, Box::new(Container::new().size(30.0, 12.0)));
    tree.get_mut(hidden).unwrap().set_visible(false);

    let measured = Container::new().size(100.0, 60.0).measure_children(
        Rect::new(0.0, 0.0, 100.0, 60.0),
        &[visible, hidden],
        &tree,
    );

    assert_eq!(measured.len(), 1);
    assert_eq!(measured[0].id, visible);
    assert_eq!(measured[0].measured_size, Size::new(20.0, 10.0));
    assert_eq!(measured[0].flex_grow, 2.0);
    assert_eq!(measured[0].flex_shrink, 0.25);
    assert_eq!(measured[0].grid_cell, Some(2));
    assert_eq!(measured[0].grid_column_span, 3);
    assert_eq!(measured[0].grid_row_span, 4);
}

#[test]
fn zero_height_row_bootstraps_from_children() {
    use crate::ui::view::adapter::ViewAdapter;
    use crate::ui::view::{label, row};

    // 未设高的 Row 在 frame.h=0 时仍须布局子项并缓存 intrinsic 高度
    let mut tree = ViewAdapter::build(row([label("A"), label("B")]));
    let root_id = tree.root_id().expect("root");
    if let Some(root) = tree.get_mut(root_id) {
        root.set_frame(Rect::new(0.0, 0.0, 200.0, 0.0));
    }
    let children = tree.get(root_id).expect("root").children().to_vec();
    let positions = tree.get(root_id).expect("root").layout_children(
        Rect::new(0.0, 0.0, 200.0, 0.0),
        &children,
        &tree,
    );
    assert_eq!(positions.len(), 2, "zero-height row must place children");
    assert!(
        positions.iter().all(|(_, r)| r.h > 0.0),
        "children should get intrinsic height, got {positions:?}"
    );
    let container = tree
        .get(root_id)
        .expect("root")
        .component()
        .as_any()
        .downcast_ref::<Container>()
        .expect("Container");
    assert!(
        container.cached_content_size.get().h > 0.0,
        "cached_content_size must update after zero-height bootstrap"
    );
}

/// 复现首页快捷导航：不定高 column_fit 放在 row 里时，标题/描述不得重叠。
#[test]
fn column_fit_nav_tile_in_row_does_not_overlap_labels() {
    use crate::ui::view::adapter::ViewAdapter;
    use crate::ui::view::{column_fit, embed, label, row, space};
    use crate::ui::widgets::{Icon, Label};

    let tile = |title: &str, desc: &str| {
        column_fit([
            row([
                embed(Icon::new("cpu").size(20.0)),
                label(title).font_size(14.0),
            ])
            .align(AlignItems::Center)
            .gap(10.0),
            space(8.0),
            label(desc).font_size(12.0),
        ])
        .width(200.0)
        .padding(EdgeInsets::uniform(16.0))
    };

    let mut tree = ViewAdapter::build(row([
        tile("应用能力", "State · Timer · Theme · 多窗"),
        tile("通用组件", "Button · Tag · Icon"),
    ]));
    let root_id = tree.root_id().expect("root");
    // 模拟页面行：先给一个偏矮的 frame，依赖 expand 撑开
    tree.get_mut(root_id)
        .expect("root")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 40.0));
    tree.push_layout_invalidation(root_id);
    tree.layout();

    let mut frames = Vec::new();
    for &id in tree.traverse().iter() {
        let node = tree.get(id).expect("node");
        if let Some(l) = node.component().as_any().downcast_ref::<Label>() {
            frames.push((l.text().to_string(), node.frame()));
        }
    }
    let title = frames.iter().find(|(t, _)| t == "应用能力").expect("title");
    let desc = frames
        .iter()
        .find(|(t, _)| t.contains("State"))
        .expect("desc");
    assert!(
        desc.1.y + 0.5 >= title.1.y + title.1.h,
        "desc must sit below title (no overlap): title={:?} desc={:?} all={frames:?}",
        title.1,
        desc.1
    );
    assert!(
        desc.1.y + 0.5 >= title.1.y + title.1.h + 6.0,
        "expected ~8px spacer between title and desc: title.bottom={} desc.y={} all={frames:?}",
        title.1.y + title.1.h,
        desc.1.y
    );
}
