//! `uix-lang-compiler/projection_schema.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use std::collections::BTreeSet;

use super::{RegistrationStatus, UI_PROJECTION_SCHEMA};

#[test]
fn schema_entries_have_unique_stable_identities() {
    let component_ids = UI_PROJECTION_SCHEMA
        .components()
        .iter()
        .map(|entry| entry.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(component_ids.len(), UI_PROJECTION_SCHEMA.components().len());

    let event_ids = UI_PROJECTION_SCHEMA
        .events()
        .iter()
        .map(|entry| entry.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(event_ids.len(), UI_PROJECTION_SCHEMA.events().len());

    let data_ids = UI_PROJECTION_SCHEMA
        .data_constructors()
        .iter()
        .map(|entry| entry.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        data_ids.len(),
        UI_PROJECTION_SCHEMA.data_constructors().len()
    );

    let value_type_ids = UI_PROJECTION_SCHEMA
        .value_types()
        .iter()
        .map(|entry| entry.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        value_type_ids.len(),
        UI_PROJECTION_SCHEMA.value_types().len()
    );

    for entries in [
        UI_PROJECTION_SCHEMA
            .common_attributes()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        UI_PROJECTION_SCHEMA
            .style_properties()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        UI_PROJECTION_SCHEMA
            .theme_tokens()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        UI_PROJECTION_SCHEMA
            .handle_slots()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        UI_PROJECTION_SCHEMA
            .slots()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        UI_PROJECTION_SCHEMA
            .capabilities()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        UI_PROJECTION_SCHEMA
            .attribute_capabilities()
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
    ] {
        assert_eq!(
            entries.iter().copied().collect::<BTreeSet<_>>().len(),
            entries.len()
        );
    }
}

#[test]
fn schema_matches_current_aot_registration_counts() {
    let available = UI_PROJECTION_SCHEMA
        .components()
        .iter()
        .filter(|entry| entry.status == RegistrationStatus::Available)
        .count();
    assert_eq!(available, 112);
    assert_eq!(UI_PROJECTION_SCHEMA.data_constructors().len(), 34);
    assert_eq!(UI_PROJECTION_SCHEMA.events().len(), 12);
    // S4 新增 backgroundSize 后的登记计数。
    assert_eq!(UI_PROJECTION_SCHEMA.style_properties().len(), 68);
    assert_eq!(UI_PROJECTION_SCHEMA.theme_tokens().len(), 90);
    assert_eq!(UI_PROJECTION_SCHEMA.handle_slots().len(), 39);
    assert_eq!(UI_PROJECTION_SCHEMA.slots().len(), 5);
    assert_eq!(UI_PROJECTION_SCHEMA.capabilities().len(), 10);
    assert_eq!(UI_PROJECTION_SCHEMA.attribute_capabilities().len(), 1);
    assert_eq!(UI_PROJECTION_SCHEMA.value_types().len(), 17);
    assert_eq!(
        UI_PROJECTION_SCHEMA.component("App").unwrap().status,
        RegistrationStatus::Available
    );
}

#[test]
fn schema_exposes_style_theme_handle_slot_and_capability_queries() {
    assert!(UI_PROJECTION_SCHEMA.style_property("transform").is_some());
    assert_eq!(
        UI_PROJECTION_SCHEMA
            .theme_token("primaryColor")
            .and_then(|entry| entry.alias_for),
        Some("colorPrimary")
    );
    assert_eq!(
        UI_PROJECTION_SCHEMA
            .handle_slot("Pagination", "pageSize")
            .map(|entry| entry.value_type),
        Some("State<usize>")
    );
    assert!(UI_PROJECTION_SCHEMA.slot("Image", "error").is_some());
    assert_eq!(
        UI_PROJECTION_SCHEMA
            .capability("charts")
            .map(|entry| entry.cargo_feature),
        Some("charts")
    );
}
