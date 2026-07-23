use uix::app::{App, Container, WindowConfig};
use uix::diagnostics::{Diagnostics, DiagnosticsConfig};
use uix::ui::view::label;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Service(&'static str);

#[test]
fn app_container_exposes_shared_service_registration() {
    let mut container = Container::new();
    container.singleton(Service("ready"));

    assert!(container.has::<Service>());
    assert_eq!(container.resolve::<Service>(), Some(&Service("ready")));
    assert_eq!(container.resolve_clone::<Service>(), Some(Service("ready")));

    container.remove::<Service>();
    assert!(!container.has::<Service>());
}

#[test]
fn app_window_config_keeps_public_window_contract() {
    let config =
        WindowConfig::new("Diagnostics", 800, 600, || label("ready")).custom_title_bar(true);

    assert_eq!(config.title, "Diagnostics");
    assert_eq!((config.width, config.height), (800, 600));
    assert!(config.custom_title_bar);
    let _root = (config.root)();
}

#[test]
fn app_exposes_one_runtime_diagnostics_handle_to_user_callbacks() {
    let _app = App::new()
        .diagnostics(DiagnosticsConfig::default().report_capacity(4))
        .on_start(|handle| {
            let _: Diagnostics = handle.diagnostics();
        });
}
