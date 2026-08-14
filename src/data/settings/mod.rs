pub use settings::SettingsService;
// 组合根把配置路径解析入口经 settings 门面消费。
pub(crate) use settings::resolve_configured_settings_path;

#[allow(clippy::module_inception)]
pub(crate) mod settings;
