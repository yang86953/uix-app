use super::*;
use crate::draw::compositor::PicturePolicy;
use crate::ui::traits::WidgetComponent;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_breadcrumb_size() {
    let measured = Breadcrumb::new()
        .item(BreadcrumbItem::new("Home"))
        .item(BreadcrumbItem::new("Docs").active())
        .measure(Constraints::loose(Size::new(60.0, 16.0)));

    assert_eq!(measured, Size::new(60.0, 16.0));
}

#[test]
fn breadcrumb_is_picture_eligible() {
    let breadcrumb = Breadcrumb::new()
        .item(BreadcrumbItem::new("Home"))
        .item(BreadcrumbItem::new("Docs").active());

    assert_eq!(breadcrumb.picture_policy(), PicturePolicy::Eligible);
}
