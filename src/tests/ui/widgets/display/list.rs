use crate::tests::common::*;
use crate::ui::widgets::List;

#[test]
fn list_size_applies_to_header_items_and_footer_measurement() {
    let constraints = Constraints::loose(Size::new(600.0, 600.0));
    let items = vec!["Ada", "Alan"];

    let small = List::new()
        .header("People")
        .items(items.clone())
        .footer("2 people")
        .size(ControlSize::Small);
    assert_eq!(small.measure(constraints), Size::new(400.0, 128.0));

    let large = List::new()
        .header("People")
        .items(items)
        .footer("2 people")
        .size(ControlSize::Large);
    assert_eq!(large.measure(constraints), Size::new(400.0, 192.0));
}

#[test]
fn load_more_keeps_its_independent_action_slot_height() {
    let constraints = Constraints::loose(Size::new(600.0, 600.0));
    let list = List::new()
        .header("People")
        .items(vec!["Ada", "Alan"])
        .footer("2 people")
        .load_more("Load more")
        .size(ControlSize::Large);

    assert_eq!(list.measure(constraints), Size::new(400.0, 232.0));
}
