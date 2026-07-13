use crate::tests::common::*;
use crate::ui::widgets::navigation::breadcrumb::*;

#[test]
fn breadcrumb_is_picture_eligible() {
    let breadcrumb = Breadcrumb::new()
        .item(BreadcrumbItem::new("Home"))
        .item(BreadcrumbItem::new("Docs").active());

    assert_eq!(breadcrumb.picture_policy(), PicturePolicy::Eligible);
}
