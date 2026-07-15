use crate::tests::common::*;
use crate::ui::view::{column, embed, ViewAdapter};
use crate::ui::{ComponentOverrides, ConfigProvider, Input, SnapshotFields};

#[test]
fn config_provider_inherits_outer_values_and_overrides_nearest_size() {
    let tree = ViewAdapter::build(
        ConfigProvider::new()
            .component_size(ControlSize::Large)
            .disabled(true)
            .child(|| {
                column((
                    embed(Input::new("outer")),
                    ConfigProvider::new()
                        .component_size(ControlSize::Small)
                        .child(|| embed(Input::new("inner"))),
                ))
            }),
    );

    let inputs = tree.find_all_by_type::<Input>();
    assert_eq!(inputs.len(), 2);
    assert!(matches!(
        inputs[0].1.snapshot_fields(),
        SnapshotFields::Input {
            input_size: ControlSize::Large,
            disabled: true,
            ..
        }
    ));
    assert!(matches!(
        inputs[1].1.snapshot_fields(),
        SnapshotFields::Input {
            input_size: ControlSize::Small,
            disabled: true,
            ..
        }
    ));
}

#[test]
fn config_provider_applies_component_constructor_overrides() {
    let mut overrides = ComponentOverrides::default();
    overrides.input.prefix = Some("¥".to_string());
    overrides.input.suffix = Some("CNY".to_string());

    let tree = ViewAdapter::build(
        ConfigProvider::new()
            .overrides(overrides)
            .child(|| embed(Input::new("amount"))),
    );
    let input = tree
        .find_all_by_type::<Input>()
        .into_iter()
        .next()
        .map(|(_, input)| input)
        .expect("configured input");

    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::Input {
            prefix,
            suffix,
            ..
        } if prefix == "¥" && suffix == "CNY"
    ));
}
