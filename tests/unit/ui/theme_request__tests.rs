// 引入父模块入口。
use super::*;
// 引入引用计数。
use std::rc::Rc;

// 验证无请求器时调用保持无副作用。
#[test]
fn set_theme_without_requester_is_side_effect_free() {
    // 直接调用不 panic、不产生任何效果。
    uix_set_theme("dark");
}

// 验证安装请求器后按名称提交，卸载后恢复无副作用。
#[test]
fn requester_receives_names_and_clears_after_uninstall() {
    // 记录提交名称（RefCell 支持非 Copy 值）。
    let received = Rc::new(RefCell::new(String::new()));
    // 复制记录句柄供请求器捕获。
    let received_clone = received.clone();
    // 安装请求器。
    uix_install_theme_requester(Box::new(move |name: &str| {
        // 记录收到的主题名。
        *received_clone.borrow_mut() = name.to_string();
    }));
    // 提交暗色主题。
    uix_set_theme("dark");
    // 请求器必须收到名称。
    assert_eq!(received.borrow().as_str(), "dark");
    // 卸载请求器。
    uix_clear_theme_requester();
    // 卸载后的调用保持无副作用。
    uix_set_theme("light");
    // 记录值保持不变。
    assert_eq!(received.borrow().as_str(), "dark");
}
