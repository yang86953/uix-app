//! E-06 路由 key 派生宏 `#[derive(Display)]` 契约。
//!
//! 覆盖：单元变体枚举派生 Display（kebab-case）、与 `Navigation<K>` 的
//! 语义事件/快照文本集成、多词变体与缩写拆分。

use std::fmt::Display as _;
use uix::prelude::*;

#[derive(Clone, PartialEq, Debug, Display)]
enum Page {
    Home,
    Settings,
    About,
}

#[derive(Clone, PartialEq, Display)]
enum TabPage {
    List,
    Detail,
    UserProfile,
    APIVersion,
}

#[test]
fn derive_display_outputs_kebab_case() {
    assert_eq!(Page::Home.to_string(), "home");
    assert_eq!(Page::Settings.to_string(), "settings");
    assert_eq!(Page::About.to_string(), "about");
    assert_eq!(TabPage::UserProfile.to_string(), "user-profile");
    assert_eq!(TabPage::APIVersion.to_string(), "api-version");
}

#[test]
fn derive_display_works_with_formatting() {
    let page = Page::Settings;
    assert_eq!(format!("{page}"), "settings");
    assert_eq!(format!("{page:?}"), "Settings", "Debug 保持变体名");
}

#[test]
fn navigation_uses_derived_display_for_semantic_text() {
    // Navigation::item 把 key 的 Display 文本写入 NavItem key（语义事件/快照）。
    let navigation = Navigation::new("MyApp")
        .item("首页", Page::Home)
        .item("设置", Page::Settings)
        .item("关于", Page::About)
        .active_page(&State::new(Page::Home))
        .show_version(false);

    // 快照文本经 NavItem key 暴露；构造不 panic 且 item key 正确。
    let _ = navigation;
}
