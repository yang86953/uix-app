use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::navigation::breadcrumb::*;

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
