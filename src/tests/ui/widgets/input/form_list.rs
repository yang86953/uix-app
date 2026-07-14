use crate::ui::{Form, FormListError};

fn item_list() -> crate::ui::FormListModel {
    Form::new()
        .list("items", |items| {
            items
                .field("name", "Name")
                .required("name required")
                .field("qty", "Quantity")
                .default(1i32)
                .validate_range(1..=100, "quantity out of range")
        })
        .build()
}

#[test]
fn form_list_add_remove_preserves_stable_item_ids() {
    let mut list = item_list();
    let first = list.add_item().expect("first item");
    let second = list.add_item().expect("second item");
    let third = list.add_item().expect("third item");

    assert_eq!(list.name(), "items");
    assert_eq!(list.item_ids(), vec![first, second, third]);
    assert_eq!(list.remove_item(1), Some(second));
    assert_eq!(list.item_ids(), vec![first, third]);
    assert_eq!(list.remove_item(9), None);

    let fourth = list.add_item().expect("fourth item");
    assert!(fourth.get() > third.get());
    assert_eq!(list.item_ids(), vec![first, third, fourth]);
}

#[test]
fn form_list_rows_keep_independent_values_and_ordered_errors() {
    let mut list = item_list();
    let first = list.add_item().expect("first item");
    let second = list.add_item().expect("second item");
    assert!(list.set_value(first, "name", "Ada"));
    assert!(list.set_value(first, "qty", 3i32));
    assert!(list.set_value(second, "qty", 0i32));

    let errors = list.validate().expect_err("second row should fail");
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].item_id(), second);
    assert_eq!(errors[0].item_index(), 1);
    assert_eq!(errors[0].field(), "name");
    assert_eq!(errors[1].field(), "qty");

    assert!(list.set_value(second, "name", "Grace"));
    assert!(list.set_value(second, "qty", 2i32));
    let values = list.validate().expect("both rows should pass");
    assert_eq!(values.len(), 2);
    assert_eq!(
        values
            .get(first)
            .and_then(|item| item.get::<String>("name")),
        Some(&"Ada".to_string())
    );
    assert_eq!(
        values.item(1).map(|(id, _)| id),
        Some(second),
        "validation preserves current row order"
    );
}

#[test]
fn form_list_clear_is_idempotent_and_missing_targets_are_noops() {
    let mut list = item_list();
    let first = list.add_item().expect("first item");
    assert_eq!(list.remove_item(0), Some(first));
    assert!(!list.set_value(first, "name", "missing"));
    list.clear();
    list.clear();
    assert!(list.is_empty());
    assert!(list.item_ids().is_empty());
    assert_eq!(
        FormListError::ItemIdExhausted.to_string(),
        "form list item id exhausted"
    );
}
