use uix_app::app::{App, Container, WindowConfig};
use uix_app::diagnostics::{Diagnostics, DiagnosticsConfig};
// 引入跨平台确定性字体包公开值。
use uix_app::draw::FontBundle;
use uix_app::ui::label;
// 引入应用默认注册和 builder 覆盖所需的 UI 服务类型。
use uix_app::platform::windowing::{
    DesktopAnchor, DesktopKeyboardInteractivity, DesktopLayer, DesktopLayerConfig,
    WindowSurfaceRole,
};
use uix_app::ui::{Locale, WidgetConfig, en_us};

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

// 冻结 UIX OS 桌面组件所需的公开 layer-surface 配置合同。
#[test]
fn app_exposes_platform_neutral_desktop_layer_surface_roles() {
    // 顶栏占据输出顶部并为普通应用保留 36 个逻辑像素。
    let layer = DesktopLayerConfig::new(DesktopLayer::Top)
        .anchor(DesktopAnchor::Top, true)
        .anchor(DesktopAnchor::Left, true)
        .anchor(DesktopAnchor::Right, true)
        .exclusive_zone(36)
        .keyboard_interactivity(DesktopKeyboardInteractivity::OnDemand)
        .namespace("uixos-top-bar");
    // 公开值在进入 Wayland 适配器前即可完成确定性验证。
    assert_eq!(layer.validate(), Ok(()));
    assert_eq!(layer.layer(), DesktopLayer::Top);
    assert!(layer.is_anchored(DesktopAnchor::Top));
    assert!(!layer.is_anchored(DesktopAnchor::Bottom));
    assert_eq!(layer.exclusive_zone_value(), 36);
    assert_eq!(layer.namespace_value(), "uixos-top-bar");

    // 主窗与次窗共享同一平台中立 surface role，而不暴露 Wayland 原生对象。
    let _app = App::new().surface_role(WindowSurfaceRole::DesktopLayer(layer.clone()));
    let config =
        WindowConfig::new("UIX OS 顶栏", 0, 36, || label("顶栏")).desktop_layer(layer.clone());
    assert_eq!(config.surface_role, WindowSurfaceRole::DesktopLayer(layer));
}

// 非法独占区必须在发送协议请求前稳定拒绝。
#[test]
fn desktop_layer_config_rejects_reserved_negative_exclusive_zones() {
    let config = DesktopLayerConfig::new(DesktopLayer::Overlay).exclusive_zone(-2);
    assert_eq!(config.validate(), Err("exclusive_zone 只允许 -1 或非负值"));
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
    assert!(app.container().resolve_clone::<WidgetConfig>() == Some(WidgetConfig::default()));

    // 构造可辨识的英文语言覆盖。
    let locale = en_us();
    // 构造可辨识的禁用组件配置覆盖。
    let config = WidgetConfig::default().disabled(true);
    // 通过公开 builder 覆盖两个框架默认单例。
    let configured = App::new().locale(locale.clone()).config(config.clone());
    // 显式语言必须替换而不是并存于默认语言。
    assert!(configured.container().resolve_clone::<Locale>() == Some(locale));
    // 显式组件配置必须替换而不是并存于默认配置。
    assert!(configured.container().resolve_clone::<WidgetConfig>() == Some(config));
    // 结束应用默认 DI 服务测试。
}

// 将确定性字体包的 App builder 注入登记为公开契约。
#[test]
fn app_font_bundle_registers_one_consumable_startup_configuration() {
    // 构造无需宿主字体路径的主字体配置。
    let bundle = FontBundle::new("UIX Primary", b"primary font fixture")
        // 声明一个顺序稳定的 CJK 回退占位资产。
        .with_fallback("UIX CJK", b"cjk font fixture");
    // 通过专用 builder 把配置交给 Application 组合根。
    let app = App::new().font_bundle(bundle);
    // 组合根必须按类型保存唯一字体包，而不是拆成字符串或平台路径。
    let configured = app.container().resolve::<FontBundle>();
    // 专用 builder 必须完成配置注入。
    assert!(configured.is_some());
    // 安全取得上方已经确认存在的字体包借用。
    let Some(configured) = configured else {
        // 断言失败时无需继续执行后续字段检查。
        return;
    };
    // 主字体族名称必须原样保留到首次 GUI 启动。
    assert_eq!(configured.primary_family(), "UIX Primary");
    // 回退字体数量必须保留调用方声明的完整顺序长度。
    assert_eq!(configured.fallback_count(), 1);
}

// 将静态字体资产的零复制 builder 冻结为公开编译契约。
#[test]
fn app_static_font_bundle_registers_primary_and_fallback_configuration() {
    // 公开入口只接受进程期静态字节，短生命周期借用无法通过类型检查。
    let bundle = FontBundle::from_static("UIX Static Primary", b"static primary font fixture")
        // 静态回退保持与拥有型入口相同的声明顺序语义。
        .with_static_fallback("UIX Static CJK", b"static cjk font fixture");
    // Application 组合根仍只保存唯一完整字体包配置。
    let app = App::new().font_bundle(bundle);
    let configured = app.container().resolve::<FontBundle>();
    // 静态来源不改变公开配置字段。
    assert!(configured.is_some_and(|bundle| {
        bundle.primary_family() == "UIX Static Primary" && bundle.fallback_count() == 1
    }));
}
