
use super::SpecialDir;

// 断言 platform 公开的 6 个 OS-known 目录逐一映射到 native 契约变体；
// 任一侧增删共享变体都会使本测试失败，提醒同步两处定义。
#[test]
fn platform_variants_are_a_subset_of_native() {
    let pairs = [
        (
            crate::platform::services::SpecialDir::Home,
            SpecialDir::Home,
        ),
        (
            crate::platform::services::SpecialDir::AppData,
            SpecialDir::AppData,
        ),
        (
            crate::platform::services::SpecialDir::LocalAppData,
            SpecialDir::LocalAppData,
        ),
        (
            crate::platform::services::SpecialDir::Documents,
            SpecialDir::Documents,
        ),
        (
            crate::platform::services::SpecialDir::Desktop,
            SpecialDir::Desktop,
        ),
        (
            crate::platform::services::SpecialDir::Downloads,
            SpecialDir::Downloads,
        ),
    ];
    for (platform, native) in pairs {
        assert_eq!(SpecialDir::from(platform), native);
    }
}
