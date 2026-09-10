use std::path::PathBuf;

use uix_app::core::Result;
use uix_app::platform::graphics::{GpuAdapterInfo, GraphicsBackend};
use uix_app::platform::hardware::{CpuInfo, DisplayInfo, MemoryInfo, OsInfo};
// 引入系统通知身份、能力状态与通知值公开契约。
use uix_app::platform::Platform;
use uix_app::platform::services::{
    AppUserModelId, SpecialDir, SystemNotification, SystemNotificationCapability,
};

fn assert_owned_info<T: Clone + Send + Sync>() {}

#[test]
fn platform_public_surface_is_concrete_typed_and_owned() {
    assert_owned_info::<OsInfo>();
    assert_owned_info::<CpuInfo>();
    assert_owned_info::<MemoryInfo>();
    assert_owned_info::<DisplayInfo>();
    assert_owned_info::<GpuAdapterInfo>();
    // AUMID 值必须可安全跨边界传递和拥有。
    assert_owned_info::<AppUserModelId>();
    // capability 是可复制、可比较的公开状态。
    assert_owned_info::<SystemNotificationCapability>();

    let _: fn() -> Result<Platform> = Platform::new;
    let _: fn(&Platform) -> Result<OsInfo> = Platform::os_info;
    let _: fn(&Platform) -> Result<CpuInfo> = Platform::cpu_info;
    let _: fn(&Platform) -> Result<MemoryInfo> = Platform::memory_info;
    let _: fn(&Platform) -> Result<Box<[DisplayInfo]>> = Platform::displays;
    let _: fn(&Platform, GraphicsBackend) -> Result<Box<[GpuAdapterInfo]>> = Platform::gpu_adapters;
    let _: fn(&Platform, SpecialDir) -> Result<PathBuf> = Platform::special_dir;
    // 配置入口必须归属于可变 Platform owner。
    let _: fn(&mut Platform, AppUserModelId) -> Result<()> =
        Platform::set_notification_app_user_model_id;
    // 能力查询只需不可变 owner 借用并返回类型化状态。
    let _: fn(&Platform) -> Result<SystemNotificationCapability> =
        Platform::system_notification_capability;
    let _: fn(&mut Platform, SystemNotification) -> Result<()> = Platform::show_notification;
}

// AUMID 构造器必须在进入平台 Adapter 前完成边界验证。
#[test]
fn notification_app_user_model_id_enforces_windows_limits() {
    // 合法的公司与产品标识应原样保存。
    let identity = AppUserModelId::new("UIX.NotificationDemo").expect("valid AUMID");
    // 公开借用必须返回完全相同的身份文本。
    assert_eq!(identity.as_str(), "UIX.NotificationDemo");
    // 空格违反 Windows AUMID 约束。
    assert!(AppUserModelId::new("UIX NotificationDemo").is_err());
    // 超过 128 个 UTF-16 单元的标识必须被拒绝。
    assert!(AppUserModelId::new("A".repeat(129)).is_err());
    // 非 BMP 字符按两个 UTF-16 单元计数，不能绕过上限。
    assert!(AppUserModelId::new("😀".repeat(65)).is_err());
}
