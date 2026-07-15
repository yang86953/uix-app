use crate::tests::common::*;
use crate::ui::widgets::display::descriptions::*;

#[test]
fn descriptions_normalizes_zero_columns_before_measurement() {
    let descriptions = Descriptions::new()
        .items(vec![
            DescriptionsItem::new("Name", "Ada"),
            DescriptionsItem::new("Role", "Engineer"),
        ])
        .column(0);

    assert!(matches!(
        descriptions.snapshot_fields(),
        SnapshotFields::Descriptions { column: 1, .. }
    ));
    assert_eq!(
        descriptions.measure(Constraints::loose(Size::new(600.0, 160.0))),
        Size::new(600.0, 72.0)
    );
}

#[test]
fn descriptions_packs_spans_without_overlap_and_measures_every_row() {
    let constraints = Constraints::loose(Size::new(600.0, 300.0));
    let descriptions = Descriptions::new()
        .items(vec![
            DescriptionsItem::new("Name", "Ada").span(2),
            DescriptionsItem::new("Role", "Engineer").span(2),
            DescriptionsItem::new("Team", "Compiler"),
        ])
        .column(3);

    assert_eq!(
        descriptions.measure(constraints),
        Size::new(600.0, 72.0),
        "the second span=2 item must wrap instead of overlapping column three"
    );
}

#[test]
fn descriptions_clamps_invalid_spans_and_label_width() {
    let constraints = Constraints::loose(Size::new(600.0, 300.0));
    let descriptions = Descriptions::new()
        .items(vec![
            DescriptionsItem::new("Wide", "value").span(usize::MAX),
            DescriptionsItem::new("Zero", "value").span(0),
        ])
        .column(2)
        .label_width(f32::NAN);

    assert_eq!(descriptions.measure(constraints), Size::new(600.0, 72.0));
    assert!(matches!(
        descriptions.snapshot_fields(),
        SnapshotFields::Descriptions {
            label_width: 0.0,
            ref items,
            ..
        } if items[1].span == 1
    ));
}
