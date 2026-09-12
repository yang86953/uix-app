//! A pure layout solver, without state, fonts, rendering or native windows.
use uix_app::core::{Rect, Size, WidgetId};
use uix_app::ui::{AlignItems, FlexLayout, LayoutChild, LayoutEngine};

fn main() {
    let children = [
        LayoutChild::new(WidgetId::new(1), Size::new(30.0, 20.0)),
        LayoutChild::new(WidgetId::new(2), Size::new(40.0, 25.0)),
    ];
    let output = FlexLayout::row()
        .with_gap(10.0)
        .with_align(AlignItems::Start)
        .layout(Rect::new(5.0, 7.0, 100.0, 50.0), &children);
    assert_eq!(
        output.positions,
        [
            Rect::new(5.0, 7.0, 30.0, 20.0),
            Rect::new(45.0, 7.0, 40.0, 25.0),
        ]
    );
    println!("standalone layout passed");
}
