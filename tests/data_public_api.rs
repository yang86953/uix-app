use uix_app::core::Errc;
use uix_app::data::SettingsService;

#[test]
fn data_settings_round_trip_scalars_in_memory() {
    let settings = SettingsService::new();
    settings.set_typed("retries", 3_u32);
    settings.set("theme", "dark");

    assert_eq!(settings.get_typed::<u32>("retries"), Ok(Some(3)));
    assert_eq!(settings.get("theme").as_deref(), Some("dark"));
    assert!(settings.dirty());
}

#[test]
fn data_settings_failures_keep_typed_diagnostics() {
    let settings = SettingsService::new();
    settings.set("retries", "not-a-number");
    let parse_error = settings
        .require_typed::<u32>("retries")
        .expect_err("invalid scalar must remain observable");
    assert_eq!(parse_error.code(), Errc::ParseError);
    assert!(parse_error.what().contains("retries"));

    let save_error = settings
        .save()
        .expect_err("dirty settings without a configured path must fail");
    assert_eq!(save_error.code(), Errc::InvalidState);
}
