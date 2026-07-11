use super::*;
use crate::core::{Constraints, Size};
use crate::ui::traits::WidgetLayout;

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
