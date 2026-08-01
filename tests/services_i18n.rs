//! E-08 全局服务注册 `App::register` 与 i18n key 宏 `t!` 契约。
//!
//! 覆盖：聚合服务一次注册与解析、资源表 key 解析（命中/未命中/替换）、
//! 格式化占位只影响显示不改变业务值。

use std::sync::{Mutex, OnceLock};

use uix::prelude::*;
use uix::ui::{register_translations, set_translations};

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[derive(Clone)]
struct SettingsService {
    path: String,
}

#[derive(Clone)]
struct AppServices {
    settings: SettingsService,
}

#[test]
fn app_register_registers_aggregate_service() {
    let services = AppServices {
        settings: SettingsService {
            path: "/data/settings.toml".into(),
        },
    };
    let mut container = uix::app::Container::new();
    container.register(services);

    let resolved = container.resolve::<AppServices>().expect("聚合服务可解析");
    assert_eq!(resolved.settings.path, "/data/settings.toml");
}

#[test]
fn app_register_builder_entry() {
    // App builder 链式入口与 singleton 共存（编译契约）。
    let mut container = uix::app::Container::new();
    container.singleton(SettingsService {
        path: "/tmp/s.toml".into(),
    });
    container.register(AppServices {
        settings: SettingsService {
            path: "/app/s.toml".into(),
        },
    });
    assert!(container.has::<AppServices>());
    assert!(container.has::<SettingsService>());
}

#[test]
fn t_macro_looks_up_registered_translations() {
    let _guard = serial();
    set_translations(&[("common.save", "保存"), ("common.cancel", "取消")]);

    assert_eq!(t!("common.save"), "保存");
    assert_eq!(t!("common.cancel"), "取消");
    // 未命中返回 key 原文（容错）。
    assert_eq!(t!("missing.key"), "missing.key");
}

#[test]
fn t_macro_registration_merges_and_overrides() {
    let _guard = serial();
    set_translations(&[("common.save", "保存")]);
    register_translations(&[("common.delete", "删除"), ("common.save", "存储")]);

    assert_eq!(t!("common.save"), "存储", "同 key 覆盖");
    assert_eq!(t!("common.delete"), "删除");
}

#[test]
fn t_macro_fmt_replaces_placeholders_without_touching_business_value() {
    let _guard = serial();
    set_translations(&[("common.total", "共 {0} 条")]);

    let count = 42;
    let shown = t!("common.total", count);
    assert_eq!(shown, "共 42 条");
    assert_eq!(count, 42, "格式化只影响显示，不改变业务值");
}
