use uix::app::{App, Container, WindowConfig};
use uix::diagnostics::{Diagnostics, DiagnosticsConfig};
use uix::ui::label;
// 引入应用默认注册和 builder 覆盖所需的 UI 服务类型。
use uix::ui::{ComponentConfig, Locale, en_us};

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

// 将 Application 组合根的默认 DI 服务注册约束登记为公开测试。
#[test]
// 验证默认服务存在，且显式 builder 配置继续覆盖同一单例。
fn app_registers_default_ui_services_and_preserves_builder_overrides() {
    // 创建未显式配置语言和组件默认值的应用。
    let app = App::new();
    // 默认语言必须在任何窗口创建前已经注册。
    assert!(app.container().resolve_clone::<Locale>() == Some(Locale::default()));
    // 默认组件配置必须在任何窗口创建前已经注册。
    assert!(app.container().resolve_clone::<ComponentConfig>() == Some(ComponentConfig::default()));

    // 构造可辨识的英文语言覆盖。
    let locale = en_us();
    // 构造可辨识的禁用组件配置覆盖。
    let config = ComponentConfig::default().disabled(true);
    // 通过公开 builder 覆盖两个框架默认单例。
    let configured = App::new().locale(locale.clone()).config(config.clone());
    // 显式语言必须替换而不是并存于默认语言。
    assert!(configured.container().resolve_clone::<Locale>() == Some(locale));
    // 显式组件配置必须替换而不是并存于默认配置。
    assert!(configured.container().resolve_clone::<ComponentConfig>() == Some(config));
// 结束应用默认 DI 服务测试。
}
